use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{value_to_bytes, StepManifest, WorkflowNode};
use crate::step::workflow::Workflow;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, EventId, Listener};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisInputStepData {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct DisInputStepPayload {
    #[serde(rename = "stepId")]
    step_id: String,
    msg: Value,
}

pub struct DisInputStep {
    app: Option<AppHandle>,
    event_id: Option<EventId>,
}

impl DisInputStep {
    /// 创建发送窗口步骤并监听前端发送事件。
    pub fn new(
        node: &WorkflowNode,
        workflow: Arc<Workflow>,
        app: Option<AppHandle>,
    ) -> Result<Arc<Self>, String> {
        node.data
            .parse::<DisInputStepData>()
            .map_err(|err| format!("disinputstep[{}] invalid data: {err}", node.id))?;

        let context = BaseStepContext::new(&node.id, workflow);
        let event_id = app.as_ref().map(|app| {
            let context = context.clone();
            let step_id = node.id.clone();
            app.listen("workflow-step-input-message", move |event| {
                let Ok(payload) = serde_json::from_str::<DisInputStepPayload>(event.payload())
                else {
                    return;
                };
                if payload.step_id != step_id {
                    return;
                }
                let Ok(bytes) = value_to_bytes(&payload.msg) else {
                    return;
                };
                let _ = context.write_down(bytes);
            })
        });

        Ok(Arc::new(Self { app, event_id }))
    }
}

impl BaseStep for DisInputStep {}

impl Drop for DisInputStep {
    /// 释放前端事件监听。
    fn drop(&mut self) {
        if let (Some(app), Some(event_id)) = (&self.app, self.event_id.take()) {
            app.unlisten(event_id);
        }
    }
}

impl StepManifestProvider for DisInputStep {
    /// 返回发送数据窗口步骤元数据。
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "DisInputStep".to_string(),
            name: "发送数据窗口".to_string(),
            description: "接收前端发送事件，按 step id 向下发布消息".to_string(),
            default_data: serde_json::json!([]),
        }
    }
}
