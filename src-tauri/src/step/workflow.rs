use crate::step::basestep::{BaseStep, StepManifestProvider};
use crate::step::disinputstep::DisInputStep;
use crate::step::disoutputstep::DisOutputStep;
use crate::step::model::WorkflowNode;
use crate::step::model::{MsgType, StepManifest, StepMsg, WorkflowDefinition, WorkflowEdge};
#[cfg(not(target_os = "android"))]
use crate::step::serialportstep::SerialPortStep;
use crate::step::tcpclientstep::TcpClientStep;
use crate::step::tcpserverstep::TcpServerStep;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, OnceLock, RwLock};
use tauri::{async_runtime, AppHandle};
use tokio::sync::{broadcast, mpsc};

/// 工作流实例集合：
/// key 为工作流 id，value 为工作流实例的强引用。
/// 工作流启动后由该集合持有，停止时再从集合中移除并释放。
type WorkflowRegistry = RwLock<HashMap<String, Arc<Workflow>>>;

/// 全局单例工作流注册表。
/// 第一次访问时初始化，后续整个进程复用同一个集合。
static WORKFLOW_INSTANCES: OnceLock<WorkflowRegistry> = OnceLock::new();

fn workflow_instances() -> &'static WorkflowRegistry {
    WORKFLOW_INSTANCES.get_or_init(|| RwLock::new(HashMap::new()))
}

pub struct Workflow {
    /// 工作流定义数据，直接保存前端传入的工作流 JSON 结构。
    definition: Arc<WorkflowDefinition>,
    edges: Arc<[WorkflowEdge]>,
    /// 工作流内部消息广播通道发送端。
    /// 每个工作流实例拥有自己的广播通道，步骤之间可以围绕该工作流收发消息。
    tx: broadcast::Sender<StepMsg<Value>>,
    /// 当前运行中的步骤实例集合。
    /// key 为 node.id，value 为具体步骤实例。
    steps: RwLock<HashMap<String, Arc<dyn BaseStep>>>,
    /// Tauri 应用句柄。
    /// 只有需要把数据推送给前端的步骤会使用它；测试或纯后端构造时可以为空。
    app: Option<AppHandle>,
}

impl Workflow {
    /// 返回当前后端支持的所有步骤类型定义。
    pub fn available_steps() -> Vec<StepManifest> {
        let mut steps = vec![
            DisInputStep::manifest(),
            DisOutputStep::manifest(),
            TcpClientStep::manifest(),
            TcpServerStep::manifest(),
        ];
        #[cfg(not(target_os = "android"))]
        steps.push(SerialPortStep::manifest());
        steps
    }

    /// 使用工作流定义创建实例，并自动注册到全局实例集合中。
    pub fn new(definition: WorkflowDefinition) -> Arc<Self> {
        // 创建当前工作流实例专属的广播通道。
        // 这里统一使用 StepMsg<Value>，这样消息体既保留结构化 JSON，
        // 又能兼容不同步骤发送的不同类型数据。
        let (tx, _) = broadcast::channel::<StepMsg<Value>>(64);
        let edges = Arc::<[WorkflowEdge]>::from(definition.edges.clone());
        Arc::new(Self {
            definition: Arc::new(definition),
            edges,
            tx,
            steps: RwLock::new(HashMap::new()),
            app: None,
        })
    }

    /// 创建带 Tauri 应用句柄的工作流实例。
    /// 接收窗口步骤会用这个句柄把收到的数据 emit 给前端。
    pub fn new_with_app(definition: WorkflowDefinition, app: AppHandle) -> Arc<Self> {
        let (tx, _) = broadcast::channel::<StepMsg<Value>>(64);
        let edges = Arc::<[WorkflowEdge]>::from(definition.edges.clone());
        Arc::new(Self {
            definition: Arc::new(definition),
            edges,
            tx,
            steps: RwLock::new(HashMap::new()),
            app: Some(app),
        })
    }

    /// 获取当前工作流 id。
    pub fn id(&self) -> &str {
        &self.definition.id
    }

    /// 获取当前工作流完整定义。
    pub fn definition(&self) -> &WorkflowDefinition {
        &self.definition
    }

    /// 运行工作流。
    /// 会按照 edges 的上下游顺序实例化节点，并按节点类型创建对应步骤对象。
    pub fn run(self: &Arc<Self>) -> Result<(), String> {
        let sorted_nodes = self.sort_node_indices();
        let mut steps = HashMap::<String, Arc<dyn BaseStep>>::new();

        for node_index in sorted_nodes {
            let node = &self.definition.nodes[node_index];
            let step = self.instantiate_step(node)?;
            steps.insert(step.id().to_string(), step);
        }

        if let Ok(mut current_steps) = self.steps.write() {
            current_steps.clear();
            current_steps.extend(steps);
        }

        Ok(())
    }

    /// Explicitly drop all step instances so their background tasks and sockets are aborted.
    pub fn shutdown(&self) {
        if let Ok(mut current_steps) = self.steps.write() {
            current_steps.clear();
        }
    }

    /// 订阅当前工作流的广播消息。
    /// 每次调用都会返回一个新的接收端，用于监听后续发布的消息。
    pub fn subscribe(&self) -> broadcast::Receiver<StepMsg<Value>> {
        self.tx.subscribe()
    }

    /// 订阅某个步骤相关的消息，并返回过滤后的 rx。
    /// 1. 只接收 StepMsg<Value> 类型消息。
    /// 2. 只接收 action 与订阅参数一致的消息。
    /// 3. 再根据当前工作流 edges 判断消息来源步骤是否满足上下游关系。
    /// 4. 满足条件的消息会被转发到返回的 rx 中。
    /// 外部可自行读取、取消或销毁该订阅。
    pub fn subscribe_step(
        &self,
        step_id: impl Into<String>,
        action: MsgType,
    ) -> WorkflowSubscription {
        let current_step_id = step_id.into();
        let mut rx = self.subscribe();
        let edges = Arc::clone(&self.edges);
        let (filtered_tx, filtered_rx) = mpsc::unbounded_channel::<StepMsg<Value>>();

        let task = async_runtime::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(step_msg) => {
                        if step_msg.action != action {
                            continue;
                        }

                        if Self::match_step_relation(
                            &edges,
                            &current_step_id,
                            &step_msg.step_id,
                            &action,
                        ) {
                            if filtered_tx.send(step_msg).is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });

        WorkflowSubscription {
            rx: filtered_rx,
            task: Some(task),
        }
    }

    /// Subscribe to any message published by a step directly connected to `step_id`.
    ///
    /// This is used by display/output steps: communication steps publish received data as
    /// `Up`, but users may place the display node on either side of the communication node
    /// in the designer. For display purposes the important relation is adjacency, not flow
    /// direction.
    pub fn subscribe_step_related(&self, step_id: impl Into<String>) -> WorkflowSubscription {
        let current_step_id = step_id.into();
        let mut rx = self.subscribe();
        let edges = Arc::clone(&self.edges);
        let (filtered_tx, filtered_rx) = mpsc::unbounded_channel::<StepMsg<Value>>();

        let task = async_runtime::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(step_msg) => {
                        if Self::match_step_adjacency(&edges, &current_step_id, &step_msg.step_id)
                            && filtered_tx.send(step_msg).is_err()
                        {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });

        WorkflowSubscription {
            rx: filtered_rx,
            task: Some(task),
        }
    }

    /// 发布步骤消息。
    /// 调用方只需要传入步骤 id、动作类型以及任意可序列化的消息体，
    /// 方法内部会自动将消息体转成 serde_json::Value 后广播出去。
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

        self.tx.send(step_msg).map_err(|err| err.to_string())
    }

    /// 将当前工作流重新序列化为 JSON 字符串。
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self.definition.as_ref())
    }

    /// 按 id 从全局实例集合中查找工作流。
    /// 如果集合中不存在该工作流，这里会返回 None。
    pub fn get(id: &str) -> Option<Arc<Self>> {
        let registry = workflow_instances().read().ok()?;
        registry.get(id).cloned()
    }

    /// 获取当前全局实例集合中的全部工作流 id。
    pub fn list_ids() -> Vec<String> {
        workflow_instances()
            .read()
            .map(|registry| registry.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// 按 id 从全局实例集合中移除工作流。
    /// 该方法既可手动调用，也会在 Drop 时自动执行。
    pub fn remove(id: &str) -> bool {
        // 注意不要在持有注册表写锁时 drop Workflow。
        // Workflow::drop 里也会尝试注销自身，如果 remove 的 Arc 在锁内被释放，
        // 就可能产生同一线程重复申请写锁的问题。
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

    /// 将新创建的工作流注册到全局实例集合中。
    pub fn register_running(workflow: &Arc<Self>) {
        if let Ok(mut registry) = workflow_instances().write() {
            registry.insert(workflow.id().to_string(), Arc::clone(workflow));
        }
    }

    /// 按节点类型实例化对应的步骤对象。
    fn instantiate_step(
        self: &Arc<Self>,
        node: &WorkflowNode,
    ) -> Result<Arc<dyn BaseStep>, String> {
        match node.r#type.to_lowercase().as_str() {
            "disinputstep" => {
                let step: Arc<dyn BaseStep> = DisInputStep::new(node, Arc::clone(self))?;
                Ok(step)
            }
            "disoutputstep" => {
                let step: Arc<dyn BaseStep> =
                    DisOutputStep::new(node, Arc::clone(self), self.app.clone())?;
                Ok(step)
            }
            "serialportstep" => {
                #[cfg(not(target_os = "android"))]
                {
                    let step: Arc<dyn BaseStep> = SerialPortStep::new(node, Arc::clone(self))?;
                    Ok(step)
                }
                #[cfg(target_os = "android")]
                {
                    Err("serialportstep is not available on Android".to_string())
                }
            }
            "tcpclientstep" => {
                let step: Arc<dyn BaseStep> = TcpClientStep::new(node, Arc::clone(self))?;
                Ok(step)
            }
            "tcpserverstep" => {
                let step: Arc<dyn BaseStep> = TcpServerStep::new(node, Arc::clone(self))?;
                Ok(step)
            }
            _ => Err(format!("unsupported step type: {}", node.r#type)),
        }
    }

    /// 按 edges 拓扑顺序排列节点。
    /// 上游节点会优先于下游节点被实例化。
    fn sort_node_indices(&self) -> Vec<usize> {
        let mut nodes_by_id = self
            .definition
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

        for edge in self.edges.iter() {
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

        // 如果图中存在环或孤立异常节点，最后按原始节点顺序补齐，避免节点丢失。
        for node in &self.definition.nodes {
            if let Some(remain) = nodes_by_id.remove(&node.id) {
                sorted.push(remain);
            }
        }

        sorted
    }

    /// 根据当前工作流的连线方向，判断消息来源步骤是否满足订阅条件。
    /// Up：
    /// 当前步骤订阅来自“下级步骤”的消息，即 source = 当前步骤、target = 消息来源步骤。
    /// Down：
    /// 当前步骤订阅来自“上级步骤”的消息，即 source = 消息来源步骤、target = 当前步骤。
    fn match_step_relation(
        edges: &[WorkflowEdge],
        current_step_id: &str,
        message_step_id: &str,
        action: &MsgType,
    ) -> bool {
        edges.iter().any(|edge| match action {
            MsgType::Up => edge.source == current_step_id && edge.target == message_step_id,
            MsgType::Down => edge.source == message_step_id && edge.target == current_step_id,
        })
    }

    fn match_step_adjacency(
        edges: &[WorkflowEdge],
        current_step_id: &str,
        message_step_id: &str,
    ) -> bool {
        edges.iter().any(|edge| {
            (edge.source == current_step_id && edge.target == message_step_id)
                || (edge.source == message_step_id && edge.target == current_step_id)
        })
    }
}

/// 工作流实例销毁时，自动从全局集合中注销。
/// 这样外部只要不再持有 Arc，实例就会自然释放。
impl Drop for Workflow {
    fn drop(&mut self) {
        Self::remove(self.id());
    }
}

/// 工作流步骤订阅对象。
/// 对外暴露过滤后的 rx，外部可按需读取消息。
/// 如需停止订阅，可主动 close/cancel，或直接丢弃该对象。
pub struct WorkflowSubscription {
    /// 过滤后的消息接收端。
    pub rx: mpsc::UnboundedReceiver<StepMsg<Value>>,
    /// 后台筛选任务句柄，用于主动终止订阅。
    task: Option<async_runtime::JoinHandle<()>>,
}

impl WorkflowSubscription {
    /// 关闭接收端并停止后台任务。
    pub fn close(&mut self) {
        self.rx.close();
        self.cancel();
    }

    /// 仅取消后台筛选任务。
    pub fn cancel(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl Drop for WorkflowSubscription {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(source: &str, target: &str) -> WorkflowEdge {
        WorkflowEdge {
            id: None,
            source: source.to_string(),
            target: target.to_string(),
            extra: HashMap::new(),
        }
    }

    fn input_node(id: &str) -> WorkflowNode {
        WorkflowNode {
            id: id.to_string(),
            r#type: "DisInputStep".to_string(),
            position: crate::step::model::WorkflowNodePosition { x: 0.0, y: 0.0 },
            data: crate::step::model::WorkflowNodeData {
                name: "input".to_string(),
                description: String::new(),
                columns: Vec::new(),
                extra: HashMap::new(),
            },
            selected: false,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn adjacency_matches_either_edge_direction() {
        let edges = vec![edge("tcp", "output"), edge("reverse-output", "serial")];

        assert!(Workflow::match_step_adjacency(&edges, "output", "tcp"));
        assert!(Workflow::match_step_adjacency(
            &edges,
            "reverse-output",
            "serial"
        ));
        assert!(!Workflow::match_step_adjacency(&edges, "output", "input"));
    }

    #[test]
    fn down_relation_does_not_match_up_receive_topology() {
        let edges = vec![edge("tcp", "output")];

        assert!(!Workflow::match_step_relation(
            &edges,
            "output",
            "tcp",
            &MsgType::Up
        ));
        assert!(Workflow::match_step_adjacency(&edges, "output", "tcp"));
    }

    #[test]
    fn run_keeps_workflow_definition_json_unchanged() {
        let definition = WorkflowDefinition {
            id: "readonly-definition".to_string(),
            name: "readonly definition".to_string(),
            description: Some("definition remains a startup snapshot".to_string()),
            nodes: vec![input_node("input")],
            edges: Vec::new(),
        };
        let expected = serde_json::to_value(&definition).unwrap();
        let workflow = Workflow::new(definition);

        workflow.run().unwrap();

        let actual: Value = serde_json::from_str(&workflow.to_json().unwrap()).unwrap();
        assert_eq!(actual, expected);
    }
}
