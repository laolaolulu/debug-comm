use crate::step::basestep::{BaseStep, StepManifestProvider};
use crate::step::model::{StepManifest, WorkflowNode};
use crate::step::workflow::Workflow;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 发送数据窗口步骤节点 data。
///
/// 该步骤本身不持有外部连接，只提供工作流节点占位和 manifest。
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
/// 发送动作由外部通过 step id 完成。
pub struct DisInputStep;

impl DisInputStep {
    /// 创建发送数据窗口步骤。
    pub fn new(node: &WorkflowNode, _workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        // 这里解析一次 data，主要用于尽早发现前端传入结构不符合约定的问题。
        node.data
            .parse::<DisInputStepData>()
            .map_err(|err| format!("disinputstep[{}] invalid data: {err}", node.id))?;

        Ok(Arc::new(Self))
    }
}

impl BaseStep for DisInputStep {}

impl StepManifestProvider for DisInputStep {
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "DisInputStep".to_string(),
            name: "发送数据窗口".to_string(),
            description: "发送数据窗口占位，消息由外部按 step id 发布".to_string(),
            default_data: serde_json::json!([]),
        }
    }
}
