use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{
    find_bytes, parse_hex_end_flag, value_to_bytes, MsgType, StepManifest, WorkflowNode,
};
use crate::step::workflow::Workflow;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::TcpListener as StdTcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::async_runtime::{self, JoinHandle};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// TCP 服务端步骤节点 data。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpServerStepData {
    /// 节点显示名称。
    pub name: String,
    /// 节点说明。
    #[serde(default)]
    pub description: String,
    /// 本地监听地址。
    pub bind_addr: String,
    /// 本地监听端口。
    pub port: u16,
    /// 可选的 16 进制结束符，例如 0A0D。为空时读到多少就发布多少。
    #[serde(default)]
    pub end_flag: Option<String>,
    /// 单次读取最大字节数。
    #[serde(default = "default_max_read_bytes")]
    pub max_read_bytes: usize,
}

fn default_max_read_bytes() -> usize {
    1024
}

type ClientWriters = Arc<Mutex<HashMap<usize, mpsc::UnboundedSender<Vec<u8>>>>>;
type ClientTasks = Arc<Mutex<Vec<JoinHandle<()>>>>;

/// TCP 服务端步骤。
///
/// 运行模型：
/// - 监听本地端口并接收客户端连接。
/// - 每个客户端有独立读任务，读到数据后发布 Up 消息。
/// - 一个工作流写任务订阅 Down 消息，并广播写给所有已连接客户端。
pub struct TcpServerStep {
    context: BaseStepContext,
    running: Arc<AtomicBool>,
    accept_task: Mutex<Option<JoinHandle<()>>>,
    write_task: Mutex<Option<JoinHandle<()>>>,
    client_tasks: ClientTasks,
}

impl TcpServerStep {
    /// 创建并启动 TCP 服务端步骤。
    pub fn new(node: WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        let context = BaseStepContext::new(node, Arc::clone(&workflow));
        let data = context
            .node
            .data
            .parse::<TcpServerStepData>()
            .map_err(|err| format!("tcpserverstep[{}] invalid data: {err}", context.id()))?;
        let end_flag = parse_hex_end_flag(data.end_flag.as_deref())
            .map_err(|err| format!("tcpserverstep[{}] invalid end_flag: {err}", context.id()))?;

        // 使用 std listener 先做同步 bind，可以让 new 直接把端口占用等错误返回给调用方。
        let address = format!("{}:{}", data.bind_addr, data.port);
        let std_listener = StdTcpListener::bind(&address)
            .map_err(|err| format!("bind {address} failed: {err}"))?;
        std_listener
            .set_nonblocking(true)
            .map_err(|err| format!("set {address} nonblocking failed: {err}"))?;
        let listener = TcpListener::from_std(std_listener)
            .map_err(|err| format!("create tokio listener {address} failed: {err}"))?;

        let step = Arc::new(Self {
            context,
            running: Arc::new(AtomicBool::new(true)),
            accept_task: Mutex::new(None),
            write_task: Mutex::new(None),
            client_tasks: Arc::new(Mutex::new(Vec::new())),
        });

        let clients: ClientWriters = Arc::new(Mutex::new(HashMap::new()));
        let client_tasks = Arc::clone(&step.client_tasks);
        let step_id = step.id().to_string();
        let running_for_accept = Arc::clone(&step.running);
        let clients_for_accept = Arc::clone(&clients);
        let workflow_for_accept = Arc::downgrade(&workflow);
        let max_read_bytes = data.max_read_bytes.max(1);
        let accept_task = async_runtime::spawn(async move {
            let mut next_client_id = 1_usize;

            while running_for_accept.load(Ordering::Relaxed) {
                let Ok((stream, _addr)) = listener.accept().await else {
                    break;
                };

                let client_id = next_client_id;
                next_client_id += 1;

                let (mut reader, mut writer) = stream.into_split();
                let (client_tx, mut client_rx) = mpsc::unbounded_channel::<Vec<u8>>();
                if let Ok(mut clients) = clients_for_accept.lock() {
                    clients.insert(client_id, client_tx);
                }

                let clients_for_reader = Arc::clone(&clients_for_accept);
                let workflow_for_reader = workflow_for_accept.clone();
                let step_id_for_reader = step_id.clone();
                let end_flag_for_reader = end_flag.clone();

                // 每个客户端独立读。客户端断开或读取失败时，移除它的写入通道。
                let reader_task = async_runtime::spawn(async move {
                    let mut read_buffer = vec![0_u8; max_read_bytes];
                    let mut packet_buffer = Vec::<u8>::new();

                    loop {
                        match reader.read(&mut read_buffer).await {
                            Ok(0) => break,
                            Ok(size) => {
                                if let Some(workflow) = workflow_for_reader.upgrade() {
                                    Self::publish_received(
                                        &workflow,
                                        &step_id_for_reader,
                                        &mut packet_buffer,
                                        end_flag_for_reader.as_deref(),
                                        &read_buffer[..size],
                                    );
                                } else {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }

                    if let Ok(mut clients) = clients_for_reader.lock() {
                        clients.remove(&client_id);
                    }
                });

                // 每个客户端独立写，工作流写任务会把 payload 投递到 client_rx。
                let writer_task = async_runtime::spawn(async move {
                    while let Some(payload) = client_rx.recv().await {
                        if writer.write_all(&payload).await.is_err() {
                            break;
                        }
                        let _ = writer.flush().await;
                    }
                });

                if let Ok(mut tasks) = client_tasks.lock() {
                    tasks.push(reader_task);
                    tasks.push(writer_task);
                }
            }
        });

        let running_for_write = Arc::clone(&step.running);
        let clients_for_write = Arc::clone(&clients);
        let mut subscription = workflow.subscribe_step(step.id().to_string(), MsgType::Down);
        let write_task = async_runtime::spawn(async move {
            while running_for_write.load(Ordering::Relaxed) {
                let Some(step_msg) = subscription.rx.recv().await else {
                    break;
                };
                let payload = match value_to_bytes(&step_msg.msg) {
                    Ok(payload) => payload,
                    Err(err) => {
                        eprintln!("tcpserverstep ignored invalid message: {err}");
                        continue;
                    }
                };
                if payload.is_empty() {
                    continue;
                }

                // 当前实现按文档默认广播给所有客户端。
                // 后续如果增加 write_mode，可以在这里改为最新客户端或指定客户端。
                if let Ok(clients) = clients_for_write.lock() {
                    for client_tx in clients.values() {
                        let _ = client_tx.send(payload.clone());
                    }
                }
            }
        });

        if let Ok(mut task) = step.accept_task.lock() {
            *task = Some(accept_task);
        }
        if let Ok(mut task) = step.write_task.lock() {
            *task = Some(write_task);
        }

        Ok(step)
    }

    /// 根据是否配置结束符，发布原始读取数据或完整数据包。
    fn publish_received(
        workflow: &Workflow,
        step_id: &str,
        packet_buffer: &mut Vec<u8>,
        end_flag: Option<&[u8]>,
        received: &[u8],
    ) {
        if let Some(flag) = end_flag {
            packet_buffer.extend_from_slice(received);
            while let Some(index) = find_bytes(packet_buffer, flag) {
                let packet_end = index + flag.len();
                let payload = packet_buffer[..packet_end].to_vec();
                packet_buffer.drain(..packet_end);
                let _ = workflow.publish(step_id.to_string(), MsgType::Up, payload);
            }
        } else {
            let _ = workflow.publish(step_id.to_string(), MsgType::Up, received.to_vec());
        }
    }
}

impl BaseStep for TcpServerStep {
    fn context(&self) -> &BaseStepContext {
        &self.context
    }
}

impl StepManifestProvider for TcpServerStep {
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "TcpServerStep".to_string(),
            name: "TCP 服务端".to_string(),
            description:
                "监听本地 TCP 端口，接收客户端数据并发布上行消息，订阅下行消息后广播写回客户端"
                    .to_string(),
            default_data: serde_json::json!([
                   {
                    "title": "结束符(HEX)",
                    "dataIndex": "end_flag",
                    "valueType": "text",
                    "initialValue": null
                },
                {
                    "title": "监听IP地址",
                    "dataIndex": "bind_addr",
                    "valueType": "text",
                    "initialValue": "0.0.0.0"
                },
                {
                    "title": "监听端口",
                    "dataIndex": "port",
                    "valueType": "digit",
                    "initialValue": 502
                },
            ]),
        }
    }
}

impl Drop for TcpServerStep {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);

        if let Ok(mut task) = self.accept_task.lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
        if let Ok(mut task) = self.write_task.lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
        if let Ok(mut tasks) = self.client_tasks.lock() {
            for task in tasks.drain(..) {
                task.abort();
            }
        }
    }
}
