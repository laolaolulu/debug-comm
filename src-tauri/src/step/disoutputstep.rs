use crate::step::basestep::{BaseStep, StepManifestProvider};
use crate::step::model::{StepManifest, StepManifestData, StepMsg, WorkflowNode};
use crate::step::workflow::Workflow;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

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

pub struct DisOutputStep {
    id: String,
    workflow_id: String,
    app: Option<AppHandle>,
}

impl DisOutputStep {
    /// 创建接收数据窗口步骤。
    pub fn new(
        node: &WorkflowNode,
        workflow: Arc<Workflow>,
        app: Option<AppHandle>,
    ) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            id: node.id.clone(),
            workflow_id: workflow.workflow.id.clone(),
            app,
        }))
    }
}

impl BaseStep for DisOutputStep {
    /// 接收下级上行消息并推送到前端。
    fn read_down(&self, step_msg: StepMsg<Value>) {
        let Some(app) = &self.app else {
            return;
        };
        let payload = WorkflowStepMessagePayload {
            task_id: self.workflow_id.clone(),
            step_id: self.id.clone(),
            step_by: step_msg.step_id,
            msg: step_msg.msg,
            time: current_time_millis(),
        };
        let _ = app.emit("workflow-step-message", payload);
    }
}

impl StepManifestProvider for DisOutputStep {
    /// 返回接收数据窗口步骤元数据。
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "DisOutputStep".into(),
            data: StepManifestData {
                name: "接收数据窗口".into(),
                description: "读取下级消息并推送给前端显示".into(),
                columns: vec![],
            },
        }
    }
}

/// 返回当前毫秒时间戳。
fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}
