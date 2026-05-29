use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{StepManifest, WorkflowNode};
use crate::step::workflow::Workflow;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::AppHandle;

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

/// 接收数据窗口步骤。
/// 该步骤只提供工作流节点占位和 manifest，消息监听由外部按 step id 完成。
pub struct DisOutputStep {
    /// 当前步骤的基础上下文，包括节点配置和所属工作流。
    context: BaseStepContext,
}

impl DisOutputStep {
    /// 创建接收数据窗口步骤。
    pub fn new(
        node: &WorkflowNode,
        workflow: Arc<Workflow>,
        _app: Option<AppHandle>,
    ) -> Result<Arc<Self>, String> {
        // 基于节点和工作流创建基础上下文。
        let context: BaseStepContext =
            BaseStepContext::new(&node.id, &node.r#type, Arc::clone(&workflow));

        // 仍然解析 data，是为了尽早发现接收窗口节点配置结构不合法。
        let _data = node
            .data
            .parse::<DisOutputStepData>()
            .map_err(|err| format!("disoutputstep[{}] invalid data: {err}", context.id()))?;

        Ok(Arc::new(Self { context }))
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
            description: "接收数据窗口占位，消息由外部按 step id 监听".to_string(),
            default_data: serde_json::json!([]),
        }
    }
}
