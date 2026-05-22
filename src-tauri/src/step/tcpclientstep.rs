use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{
    find_bytes, parse_hex_end_flag, value_to_bytes, MsgType, StepManifest, WorkflowNode,
};
use crate::step::workflow::Workflow;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::async_runtime::{self, JoinHandle};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// TCP 客户端步骤节点 data。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpClientStepData {
    /// 节点显示名称。
    pub name: String,
    /// 节点说明。
    #[serde(default)]
    pub description: String,
    /// 可选的 16 进制结束符，例如 0A0D。为空时读到多少就发布多少。
    #[serde(default)]
    pub end_flag: Option<String>,
    /// 远端主机地址。
    pub host: String,
    /// 远端端口。
    pub port: u16,
}

/// TCP 客户端步骤。
///
/// 运行模型：
/// - 建立一个到远端的 TCP 连接。
/// - 订阅上游 Down 消息并写入 socket。
/// - 从 socket 读取数据并发布 Up 消息。
pub struct TcpClientStep {
    context: BaseStepContext,
    running: Arc<AtomicBool>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl TcpClientStep {
    /// 创建并启动 TCP 客户端步骤。
    pub fn new(node: WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        let context = BaseStepContext::new(node, Arc::clone(&workflow));
        let data = context
            .node
            .data
            .parse::<TcpClientStepData>()
            .map_err(|err| format!("tcpclientstep[{}] invalid data: {err}", context.id()))?;
        let end_flag = parse_hex_end_flag(data.end_flag.as_deref())
            .map_err(|err| format!("tcpclientstep[{}] invalid end_flag: {err}", context.id()))?;

        let step = Arc::new(Self {
            context,
            running: Arc::new(AtomicBool::new(true)),
            task: Mutex::new(None),
        });

        let address = format!("{}:{}", data.host, data.port);
        let step_id = step.id().to_string();
        let running = Arc::clone(&step.running);
        let workflow_for_task = Arc::downgrade(&workflow);
        let mut subscription = workflow.subscribe_step(step.id().to_string(), MsgType::Down);

        let task = async_runtime::spawn(async move {
            let Ok(mut stream) = TcpStream::connect(&address).await else {
                return;
            };

            let mut read_buffer = vec![0_u8; 1024];
            let mut packet_buffer = Vec::<u8>::new();

            while running.load(Ordering::Relaxed) {
                tokio::select! {
                    inbound = subscription.rx.recv() => {
                        let Some(step_msg) = inbound else {
                            break;
                        };
                        let payload = match value_to_bytes(&step_msg.msg) {
                            Ok(payload) => payload,
                            Err(err) => {
                                eprintln!("tcpclientstep ignored invalid message: {err}");
                                continue;
                            }
                        };
                        if payload.is_empty() {
                            continue;
                        }
                        if stream.write_all(&payload).await.is_err() {
                            break;
                        }
                        let _ = stream.flush().await;
                    }
                    read_result = stream.read(&mut read_buffer) => {
                        match read_result {
                            Ok(0) => break,
                            Ok(size) => {
                                let received = &read_buffer[..size];
                                if let Some(workflow) = workflow_for_task.upgrade() {
                                    Self::publish_received(
                                        &workflow,
                                        &step_id,
                                        &mut packet_buffer,
                                        end_flag.as_deref(),
                                        received,
                                    );
                                } else {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        if let Ok(mut current_task) = step.task.lock() {
            *current_task = Some(task);
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

impl BaseStep for TcpClientStep {
    fn context(&self) -> &BaseStepContext {
        &self.context
    }
}

impl StepManifestProvider for TcpClientStep {
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "TcpClientStep".to_string(),
            name: "TCP 客户端".to_string(),
            description: "主动连接远端 TCP 服务，订阅上级消息并写入连接，读取返回数据后向上级发布"
                .to_string(),
            default_data: serde_json::json!([
                {
                    "title": "结束符(HEX)",
                    "dataIndex": "end_flag",
                    "valueType": "text",
                    "initialValue": null
                },
                {
                    "title": "服务IP地址",
                    "dataIndex": "host",
                    "valueType": "text",
                    "initialValue": "127.0.0.1"
                },
                {
                    "title": "服务端口",
                    "dataIndex": "port",
                    "valueType": "digit",
                    "initialValue": 502
                }
            ]),
        }
    }
}

impl Drop for TcpClientStep {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);

        if let Ok(mut task) = self.task.lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
    }
}
