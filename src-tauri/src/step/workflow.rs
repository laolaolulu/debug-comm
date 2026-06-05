use crate::step::basestep::{BaseStep, StepManifestProvider};
use crate::step::disinputstep::DisInputStep;
use crate::step::disoutputstep::DisOutputStep;
use crate::step::javascriptstep::JavaScriptStep;
use crate::step::model::{MsgType, StepManifest, StepMsg, WorkflowDefinition, WorkflowNode};
use crate::step::serialportstep::SerialPortStep;
use crate::step::tcpclientstep::TcpClientStep;
use crate::step::tcpserverstep::TcpServerStep;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, OnceLock, RwLock};
use tauri::AppHandle;

type WorkflowRegistry = RwLock<HashMap<String, Arc<Workflow>>>;

static WORKFLOW_INSTANCES: OnceLock<WorkflowRegistry> = OnceLock::new();

/// 获取全局运行中工作流注册表。
fn workflow_instances() -> &'static WorkflowRegistry {
    WORKFLOW_INSTANCES.get_or_init(|| RwLock::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::model::WorkflowNodeData;

    fn unsupported_node(id: &str, name: &str, step_type: &str) -> WorkflowNode {
        WorkflowNode {
            id: id.to_string(),
            r#type: step_type.to_string(),
            data: WorkflowNodeData {
                name: name.to_string(),
                description: String::new(),
                columns: Vec::new(),
                params: HashMap::new(),
            },
        }
    }

    #[test]
    fn run_collects_all_node_start_errors() {
        let workflow = Workflow::new_for_test(WorkflowDefinition {
            id: "collect-start-errors".to_string(),
            name: "Collect start errors".to_string(),
            description: None,
            nodes: vec![
                unsupported_node("node-a", "Node A", "MissingStepA"),
                unsupported_node("node-b", "Node B", "MissingStepB"),
            ],
            edges: Vec::new(),
        });

        let result = workflow
            .run()
            .expect("workflow run should return a start result");

        assert!(!result.started);
        assert_eq!(result.errors.len(), 2);
        assert_eq!(result.errors[0].step_id, "node-a");
        assert_eq!(result.errors[0].step_name, "Node A");
        assert_eq!(result.errors[1].step_id, "node-b");
        assert_eq!(result.errors[1].step_name, "Node B");
        assert!(!Workflow::list_ids().contains(&"collect-start-errors".to_string()));
    }
}

pub struct Workflow {
    pub workflow: WorkflowDefinition,
    steps: RwLock<HashMap<String, Arc<dyn BaseStep>>>,
    app: Option<AppHandle>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowStartError {
    #[serde(rename = "stepId")]
    pub step_id: String,
    #[serde(rename = "stepName")]
    pub step_name: String,
    #[serde(rename = "stepType")]
    pub step_type: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct WorkflowStartResult {
    pub started: bool,
    pub errors: Vec<WorkflowStartError>,
}

impl Workflow {
    /// 返回当前后端支持的所有步骤类型定义。
    pub fn available_steps() -> Vec<StepManifest> {
        vec![
            DisInputStep::manifest(),
            DisOutputStep::manifest(),
            TcpClientStep::manifest(),
            TcpServerStep::manifest(),
            SerialPortStep::manifest(),
            JavaScriptStep::manifest(),
        ]
    }

    /// 创建带 Tauri 应用句柄的工作流实例。
    pub fn new_with_app(workflow: WorkflowDefinition, app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            workflow,
            steps: RwLock::new(HashMap::new()),
            app: Some(app),
        })
    }

    #[cfg(test)]
    pub fn new_for_test(workflow: WorkflowDefinition) -> Arc<Self> {
        Arc::new(Self {
            workflow,
            steps: RwLock::new(HashMap::new()),
            app: None,
        })
    }

    /// 按拓扑顺序实例化并启动工作流中的所有步骤。
    pub fn run(self: &Arc<Self>) -> Result<WorkflowStartResult, String> {
        let sorted_nodes = self.sort_node_indices();
        let mut steps = HashMap::<String, Arc<dyn BaseStep>>::new();
        let mut errors = Vec::<WorkflowStartError>::new();

        for node_index in sorted_nodes {
            let node = &self.workflow.nodes[node_index];
            match self.instantiate_step(node) {
                Ok(step) => {
                    steps.insert(node.id.clone(), step);
                }
                Err(message) => {
                    errors.push(WorkflowStartError {
                        step_id: node.id.clone(),
                        step_name: node.data.name.clone(),
                        step_type: node.r#type.clone(),
                        message,
                    });
                }
            }
        }

        let started = !steps.is_empty();
        if let Ok(mut current_steps) = self.steps.write() {
            current_steps.clear();
            current_steps.extend(steps);
        }
        if started {
            self.register_running();
        }

        Ok(WorkflowStartResult { started, errors })
    }

    /// 清空当前步骤实例，让后台任务和连接随步骤释放。
    pub fn shutdown(&self) {
        if let Ok(mut current_steps) = self.steps.write() {
            current_steps.clear();
        }
    }

    /// 将步骤消息按方向转发给相邻步骤。
    pub fn publish<T>(
        &self,
        step_id: impl Into<String>,
        action: MsgType,
        msg: T,
    ) -> Result<usize, String>
    where
        T: Serialize,
    {
        let step_msg = StepMsg {
            step_id: step_id.into(),
            action,
            msg: serde_json::to_value(msg).map_err(|err| err.to_string())?,
        };

        let targets = self.message_targets(&step_msg);
        let count = targets.len();

        for target in targets {
            match step_msg.action {
                MsgType::Down => target.read_up(step_msg.clone()),
                MsgType::Up => target.read_down(step_msg.clone()),
            }
        }

        Ok(count)
    }

    /// 获取当前全局注册表中的全部工作流 id。
    pub fn list_ids() -> Vec<String> {
        workflow_instances()
            .read()
            .map(|registry| registry.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// 按 id 从全局注册表移除工作流并关闭其步骤。
    pub fn remove(id: &str) -> bool {
        let removed = workflow_instances()
            .write()
            .ok()
            .and_then(|mut registry| registry.remove(id));
        let existed = removed.is_some();
        if let Some(workflow) = removed.as_ref() {
            workflow.shutdown();
        }
        drop(removed);
        existed
    }

    /// 将运行中的工作流注册到全局集合。
    fn register_running(self: &Arc<Self>) {
        if let Ok(mut registry) = workflow_instances().write() {
            registry.insert(self.workflow.id.clone(), Arc::clone(self));
        }
    }

    /// 按节点类型创建具体步骤实例。
    fn instantiate_step(
        self: &Arc<Self>,
        node: &WorkflowNode,
    ) -> Result<Arc<dyn BaseStep>, String> {
        match node.r#type.to_lowercase().as_str() {
            "disinputstep" => {
                let step: Arc<dyn BaseStep> =
                    DisInputStep::new(node, Arc::clone(self), self.app.clone())?;
                Ok(step)
            }
            "disoutputstep" => {
                let step: Arc<dyn BaseStep> =
                    DisOutputStep::new(node, Arc::clone(self), self.app.clone())?;
                Ok(step)
            }
            "serialportstep" => {
                let step: Arc<dyn BaseStep> = SerialPortStep::new(node, Arc::clone(self))?;
                Ok(step)
            }
            "tcpclientstep" => {
                let step: Arc<dyn BaseStep> = TcpClientStep::new(node, Arc::clone(self))?;
                Ok(step)
            }
            "tcpserverstep" => {
                let step: Arc<dyn BaseStep> = TcpServerStep::new(node, Arc::clone(self))?;
                Ok(step)
            }
            "javascriptstep" => {
                let step: Arc<dyn BaseStep> = JavaScriptStep::new(node, Arc::clone(self))?;
                Ok(step)
            }
            _ => Err(format!("unsupported step type: {}", node.r#type)),
        }
    }

    /// 按 edges 拓扑顺序排列节点，异常残留节点按原顺序补齐。
    fn sort_node_indices(&self) -> Vec<usize> {
        let mut nodes_by_id = self
            .workflow
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.clone(), index))
            .collect::<HashMap<_, _>>();
        let mut indegree = nodes_by_id
            .keys()
            .cloned()
            .map(|id| (id, 0_usize))
            .collect::<HashMap<_, _>>();
        let mut graph = HashMap::<String, Vec<String>>::new();

        for edge in self.workflow.edges.iter() {
            if nodes_by_id.contains_key(&edge.source) && nodes_by_id.contains_key(&edge.target) {
                graph
                    .entry(edge.source.clone())
                    .or_default()
                    .push(edge.target.clone());
                if let Some(value) = indegree.get_mut(&edge.target) {
                    *value += 1;
                }
            }
        }

        let mut zero_indegree = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        zero_indegree.sort();

        let mut queue = VecDeque::from(zero_indegree);
        let mut sorted = Vec::with_capacity(nodes_by_id.len());

        while let Some(node_id) = queue.pop_front() {
            if let Some(node_index) = nodes_by_id.remove(&node_id) {
                sorted.push(node_index);
            }

            if let Some(targets) = graph.get(&node_id) {
                for target in targets {
                    if let Some(value) = indegree.get_mut(target) {
                        *value -= 1;
                        if *value == 0 {
                            queue.push_back(target.clone());
                        }
                    }
                }
            }
        }

        for node in &self.workflow.nodes {
            if let Some(remain) = nodes_by_id.remove(&node.id) {
                sorted.push(remain);
            }
        }

        sorted
    }

    /// 根据消息方向找到需要触发的相邻步骤。
    fn message_targets(&self, step_msg: &StepMsg<Value>) -> Vec<Arc<dyn BaseStep>> {
        let target_ids = self
            .workflow
            .edges
            .iter()
            .filter_map(|edge| match step_msg.action {
                MsgType::Down if edge.source == step_msg.step_id => Some(edge.target.as_str()),
                MsgType::Up if edge.target == step_msg.step_id => Some(edge.source.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        let Ok(steps) = self.steps.read() else {
            return Vec::new();
        };

        target_ids
            .into_iter()
            .filter_map(|step_id| steps.get(step_id).cloned())
            .collect()
    }
}

impl Drop for Workflow {
    /// 工作流释放时从全局注册表注销自己。
    fn drop(&mut self) {
        Self::remove(&self.workflow.id);
    }
}
