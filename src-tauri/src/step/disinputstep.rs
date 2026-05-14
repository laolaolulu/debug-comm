use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{MsgType, StepManifest, WorkflowNode};
use crate::step::workflow::Workflow;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// 发送数据窗口步骤节点 data。
///
/// 该步骤本身不持有外部连接，它更像一个工作流入口：
/// 前端或其他 Tauri command 找到 workflow 后，可通过该节点 id 发布 Down 消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisInputStepData {
    /// 节点显示名称。
    pub name: String,
    /// 节点说明。
    #[serde(default)]
    pub description: String,
}

/// 发送数据窗口步骤。
///
/// 目前步骤只负责占位和提供 manifest。真正的“发送”动作由外部调用
/// `Workflow::publish(step_id, MsgType::Down, payload)` 完成，这样前端按钮、
/// 快捷键或脚本都可以复用同一条发布路径。
pub struct DisInputStep {
    context: BaseStepContext,
}

impl DisInputStep {
    /// 创建发送数据窗口步骤。
    pub fn new(node: WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        let context = BaseStepContext::new(node, workflow);

        // 这里解析一次 data，主要用于尽早发现前端传入结构不符合约定的问题。
        context
            .node
            .data
            .parse::<DisInputStepData>()
            .map_err(|err| format!("disinputstep[{}] invalid data: {err}", context.id()))?;

        Ok(Arc::new(Self { context }))
    }

    /// 通过当前输入步骤向下游发布消息。
    ///
    /// 这个方法便于后续 Tauri command 或测试代码拿到步骤实例后复用。
    /// 当前工作流步骤集合保存的是 trait object，外部通常会直接调用 Workflow::publish。
    pub fn publish_down(&self, payload: Value) -> Result<usize, String> {
        let workflow = self
            .context
            .workflow()
            .ok_or_else(|| format!("workflow dropped for step {}", self.id()))?;

        workflow.publish(self.id().to_string(), MsgType::Down, payload)
    }
}

impl BaseStep for DisInputStep {
    fn context(&self) -> &BaseStepContext {
        &self.context
    }
}

impl StepManifestProvider for DisInputStep {
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "DisInputStep".to_string(),
            name: "发送数据窗口".to_string(),
            description: "作为人工输入入口，将前端输入发布为下行消息".to_string(),
            default_data: serde_json::json!([]),
        }
    }
}
