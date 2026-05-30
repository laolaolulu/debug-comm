use crate::step::model::{MsgType, StepManifest, StepMsg, WorkflowNode};
use crate::step::workflow::Workflow;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::sync::{Arc, Weak};

/// 步骤基础上下文。
#[derive(Debug, Clone)]
pub struct BaseStepContext {
    pub node: WorkflowNode,
    workflow: Weak<Workflow>,
}

impl BaseStepContext {
    /// 创建步骤上下文并保存节点参数。
    pub fn new(node: &WorkflowNode, workflow: Arc<Workflow>) -> Self {
        Self {
            node: node.clone(),
            workflow: Arc::downgrade(&workflow),
        }
    }

    /// 返回当前步骤 id。
    pub fn id(&self) -> &str {
        &self.node.id
    }

    /// 读取步骤参数。
    pub fn get_data<T>(&self, key: &str) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let value = self
            .node
            .data
            .value(key)
            .ok_or_else(|| format!("step {} missing parameter: {key}", self.id()))?;
        serde_json::from_value(value.clone())
            .map_err(|_| format!("step {} invalid parameter: {key}", self.id()))
    }

    /// 读取可选步骤参数。
    pub fn get_optional_data<T>(&self, key: &str) -> Result<Option<T>, String>
    where
        T: DeserializeOwned,
    {
        self.node
            .data
            .value(key)
            .map(|value| {
                serde_json::from_value(value.clone())
                    .map_err(|_| format!("step {} invalid parameter: {key}", self.id()))
            })
            .transpose()
    }

    /// 获取所属工作流实例。
    fn workflow(&self) -> Result<Arc<Workflow>, String> {
        self.workflow
            .upgrade()
            .ok_or_else(|| format!("workflow dropped for step {}", self.id()))
    }

    /// 向下级步骤发布消息。
    pub fn write_down<T>(&self, msg: T) -> Result<usize, String>
    where
        T: Serialize,
    {
        self.workflow()?
            .publish(self.id().to_string(), MsgType::Down, msg)
    }

    /// 向上级步骤发布消息。
    pub fn write_up<T>(&self, msg: T) -> Result<usize, String>
    where
        T: Serialize,
    {
        self.workflow()?
            .publish(self.id().to_string(), MsgType::Up, msg)
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
pub trait StepManifestProvider {
    /// 返回步骤在前端可创建列表中的元数据。
    fn manifest() -> StepManifest
    where
        Self: Sized;
}
