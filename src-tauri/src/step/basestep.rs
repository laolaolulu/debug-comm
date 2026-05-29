use crate::step::model::{MsgType, StepManifest, StepMsg};
use crate::step::workflow::Workflow;
use serde::Serialize;
use serde_json::Value;
use std::sync::{Arc, Weak};

/// 步骤基础上下文。
#[derive(Debug, Clone)]
pub struct BaseStepContext {
    id: String,
    workflow: Weak<Workflow>,
}

impl BaseStepContext {
    /// 创建步骤上下文并弱引用所属工作流。
    pub fn new(id: impl Into<String>, workflow: Arc<Workflow>) -> Self {
        Self {
            id: id.into(),
            workflow: Arc::downgrade(&workflow),
        }
    }

    /// 返回当前步骤 id。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 获取所属工作流实例。
    fn workflow(&self) -> Result<Arc<Workflow>, String> {
        self.workflow
            .upgrade()
            .ok_or_else(|| format!("workflow dropped for step {}", self.id))
    }

    /// 向下级步骤发布消息。
    pub fn write_down<T>(&self, msg: T) -> Result<usize, String>
    where
        T: Serialize,
    {
        self.workflow()?
            .publish(self.id.to_string(), MsgType::Down, msg)
    }

    /// 向上级步骤发布消息。
    pub fn write_up<T>(&self, msg: T) -> Result<usize, String>
    where
        T: Serialize,
    {
        self.workflow()?
            .publish(self.id.to_string(), MsgType::Up, msg)
    }
}

/// 所有步骤的公共消息能力。
pub trait BaseStep: Send + Sync {
    /// 上级消息下发到当前步骤时触发。
    fn read_up(&self, _step_msg: StepMsg<Value>) {}

    /// 下级消息上行到当前步骤时触发。
    fn read_down(&self, _step_msg: StepMsg<Value>) {}
}

/// 步骤元数据提供者。
/// 用于向前端导出可创建的步骤列表和默认节点 data。
pub trait StepManifestProvider {
    /// 返回步骤在前端可创建列表中的元数据。
    fn manifest() -> StepManifest
    where
        Self: Sized;
}
