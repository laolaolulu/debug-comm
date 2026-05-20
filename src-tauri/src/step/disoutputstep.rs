use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{StepManifest, WorkflowNode};
use crate::step::workflow::Workflow;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::async_runtime::{self, JoinHandle};
use tauri::{AppHandle, Emitter};

/// 接收数据窗口步骤节点 data 结构。
/// 当前只保留最基础的显示字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisOutputStepData {
    /// 节点显示名称。
    pub name: String,
    /// 节点说明。
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
struct WorkflowStepMessagePayload {
    #[serde(rename = "taskId")]
    task_id: String,
    #[serde(rename = "stepId")]
    step_id: String,
    #[serde(rename = "stepBy")]
    step_by: String,
    msg: Value,
    time: u64,
}

/// 接收数据窗口步骤。
/// 该步骤负责监听与自身相邻的通信节点消息，并直接推送给前端。
pub struct DisOutputStep {
    /// 当前步骤的基础上下文，包括节点配置和所属工作流。
    context: BaseStepContext,
    /// 步骤运行状态，用于控制后台接收任务退出。
    running: Arc<AtomicBool>,
    /// 后台接收任务句柄，便于步骤销毁时停止任务。
    receive_task: Mutex<Option<JoinHandle<()>>>,
}

impl DisOutputStep {
    /// 创建并启动接收数据窗口步骤。
    pub fn new(
        node: WorkflowNode,
        workflow: Arc<Workflow>,
        app: Option<AppHandle>,
    ) -> Result<Arc<Self>, String> {
        // 基于节点和工作流创建基础上下文。
        let context: BaseStepContext = BaseStepContext::new(node, Arc::clone(&workflow));

        // 仍然解析 data，是为了尽早发现接收窗口节点配置结构不合法。
        let _data = context
            .node
            .data
            .parse::<DisOutputStepData>()
            .map_err(|err| format!("disoutputstep[{}] invalid data: {err}", context.id()))?;

        // 创建步骤实例。
        let step = Arc::new(Self {
            context,
            running: Arc::new(AtomicBool::new(true)),
            receive_task: Mutex::new(None),
        });

        if let Some(app) = app {
            let workflow_id = workflow.id().to_string();
            let output_step_id = step.id().to_string();
            let mut subscription = workflow.subscribe_step_related(output_step_id.clone());
            let running = Arc::clone(&step.running);

            let receive_task = async_runtime::spawn(async move {
                while running.load(Ordering::Relaxed) {
                    let Some(step_msg) = subscription.rx.recv().await else {
                        break;
                    };

                    let payload = WorkflowStepMessagePayload {
                        task_id: workflow_id.clone(),
                        step_id: output_step_id.clone(),
                        step_by: step_msg.step_id,
                        msg: step_msg.msg,
                        time: current_time_millis(),
                    };
                    let _ = app.emit("workflow-step-message", payload);
                }
            });

            if let Ok(mut task) = step.receive_task.lock() {
                *task = Some(receive_task);
            }
        }

        Ok(step)
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
            description: "接收相邻通信节点消息并推送给前端显示".to_string(),
            default_data: serde_json::json!([]),
        }
    }
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
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
