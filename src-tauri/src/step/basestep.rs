use crate::step::model::{StepManifest, WorkflowNode};
use crate::step::workflow::Workflow;
use std::sync::{Arc, Weak};

/// 步骤基础上下文。
/// 所有具体步骤都会持有当前节点定义和所属工作流实例。
#[derive(Debug, Clone)]
pub struct BaseStepContext {
    /// 当前步骤对应的工作流节点。
    pub node: WorkflowNode,
    /// 当前节点所属工作流实例。
    pub workflow: Weak<Workflow>,
}

impl BaseStepContext {
    /// 创建步骤基础上下文。
    pub fn new(node: WorkflowNode, workflow: Arc<Workflow>) -> Self {
        Self {
            node,
            workflow: Arc::downgrade(&workflow),
        }
    }

    /// 获取当前步骤 id。
    pub fn id(&self) -> &str {
        &self.node.id
    }

    /// 获取当前步骤类型。
    pub fn node_type(&self) -> &str {
        &self.node.r#type
    }

    /// 获取当前所属工作流实例。
    pub fn workflow(&self) -> Option<Arc<Workflow>> {
        self.workflow.upgrade()
    }
}

/// 所有步骤的公共能力。
/// 具体步骤通过组合 BaseStepContext 来复用基础字段。
pub trait BaseStep: Send + Sync {
    /// 获取步骤基础上下文。
    fn context(&self) -> &BaseStepContext;

    /// 获取当前步骤 id。
    fn id(&self) -> &str {
        self.context().id()
    }

    /// 获取当前步骤类型。
    fn node_type(&self) -> &str {
        self.context().node_type()
    }
}

/// 步骤元数据提供者。
/// 用于向前端导出可创建的步骤列表和默认节点 data。
pub trait StepManifestProvider {
    fn manifest() -> StepManifest
    where
        Self: Sized;
}

/// 无行为的基础步骤。
/// 对于暂未实现的节点类型，先实例化成该类型，保证工作流能够完成节点装配。
#[derive(Debug)]
pub struct BasicStep {
    context: BaseStepContext,
}

impl BasicStep {
    /// 创建一个无行为的基础步骤实例。
    pub fn new(node: WorkflowNode, workflow: Arc<Workflow>) -> Arc<Self> {
        Arc::new(Self {
            context: BaseStepContext::new(node, workflow),
        })
    }
}

impl BaseStep for BasicStep {
    fn context(&self) -> &BaseStepContext {
        &self.context
    }
}
