use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{MsgType, StepManifest, StepMsg, WorkflowNode};
use crate::step::workflow::Workflow;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

/// 接收数据窗口步骤节点 data 结构。
/// 当前只保留最基础的显示字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisOutputStepData {
    /// 节点显示名称。
    pub name: String,
    /// 节点说明。
    #[serde(default)]
    pub description: String,
    /// 接收窗口最多缓存多少条消息。
    #[serde(default = "default_cache_size")]
    pub cache_size: usize,
}

fn default_cache_size() -> usize {
    200
}

/// 接收数据窗口步骤。
/// 该步骤只负责接收并缓存消息，不做其他业务。
pub struct DisOutputStep {
    /// 当前步骤的基础上下文，包括节点配置和所属工作流。
    context: BaseStepContext,
    /// 步骤运行状态，用于控制后台接收任务退出。
    running: Arc<AtomicBool>,
    /// 最近接收到的消息缓存。
    /// 当前还没有对外查询接口，先把缓存放在步骤内部，后续可接 Tauri command 或 event。
    messages: Arc<Mutex<VecDeque<StepMsg<Value>>>>,
    /// 后台接收任务句柄，便于步骤销毁时停止任务。
    receive_task: Mutex<Option<JoinHandle<()>>>,
}

impl DisOutputStep {
    /// 创建并启动接收数据窗口步骤。
    pub fn new(node: WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        // 基于节点和工作流创建基础上下文。
        let context: BaseStepContext = BaseStepContext::new(node, Arc::clone(&workflow));

        // 将节点 data 解析成当前步骤自己的 data 结构。
        let data = context
            .node
            .data
            .parse::<DisOutputStepData>()
            .map_err(|err| format!("disoutputstep[{}] invalid data: {err}", context.id()))?;
        let cache_size = data.cache_size.max(1);

        // 创建步骤实例。
        let step = Arc::new(Self {
            context,
            running: Arc::new(AtomicBool::new(true)),
            messages: Arc::new(Mutex::new(VecDeque::with_capacity(cache_size))),
            receive_task: Mutex::new(None),
        });

        // 接收窗口作为链路末端，默认订阅上游发来的 Down 消息。
        // 当前只做内存缓存；如果后续需要实时 UI，可在这里增加 app_handle.emit。
        let mut subscription = workflow.subscribe_step(step.id().to_string(), MsgType::Down);
        let messages = Arc::clone(&step.messages);
        let running = Arc::clone(&step.running);
        let receive_task = tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                let Some(step_msg) = subscription.rx.recv().await else {
                    break;
                };

                if let Ok(mut messages) = messages.lock() {
                    if messages.len() >= cache_size {
                        messages.pop_front();
                    }
                    messages.push_back(step_msg);
                }
            }
        });

        if let Ok(mut task) = step.receive_task.lock() {
            *task = Some(receive_task);
        }

        Ok(step)
    }

    /// 返回当前缓存消息快照。
    /// 该方法目前供后续命令或测试使用，不会暴露内部 VecDeque 的可变引用。
    pub fn cached_messages(&self) -> Vec<StepMsg<Value>> {
        self.messages
            .lock()
            .map(|messages| messages.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl BaseStep for DisOutputStep {
    fn context(&self) -> &BaseStepContext {
        &self.context
    }
}

impl StepManifestProvider for DisOutputStep {
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "DisOutputStep".to_string(),
            name: "接收数据窗口".to_string(),
            description: "接收并缓存来自上级步骤的消息，不做其他业务处理".to_string(),
            default_data: serde_json::json!([]),
        }
    }
}

impl Drop for DisOutputStep {
    fn drop(&mut self) {
        // 通知后台任务退出循环。
        self.running.store(false, Ordering::Relaxed);

        // 如果任务句柄存在，则主动中止后台任务。
        if let Ok(mut task) = self.receive_task.lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
    }
}
