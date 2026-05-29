use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{
    find_bytes, parse_hex_end_flag, value_to_bytes, StepManifest, StepMsg, WorkflowNode,
};
use crate::step::workflow::Workflow;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::async_runtime::{self, JoinHandle};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::Mutex as AsyncMutex;

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
/// - 上级消息到达时写入 socket。
/// - 从 socket 读取数据并向上级发布消息。
pub struct TcpClientStep {
    context: BaseStepContext,
    running: Arc<AtomicBool>,
    writer: Arc<AsyncMutex<Option<OwnedWriteHalf>>>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl TcpClientStep {
    /// 创建并启动 TCP 客户端步骤。
    pub fn new(node: &WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        let context = BaseStepContext::new(&node.id, Arc::clone(&workflow));
        let data = node
            .data
            .parse::<TcpClientStepData>()
            .map_err(|err| format!("tcpclientstep[{}] invalid data: {err}", context.id()))?;
        let end_flag = parse_hex_end_flag(data.end_flag.as_deref())
            .map_err(|err| format!("tcpclientstep[{}] invalid end_flag: {err}", context.id()))?;

        let step = Arc::new(Self {
            context,
            running: Arc::new(AtomicBool::new(true)),
            writer: Arc::new(AsyncMutex::new(None)),
            task: Mutex::new(None),
        });

        let address = format!("{}:{}", data.host, data.port);
        let running = Arc::clone(&step.running);
        let context_for_task = step.context.clone();
        let writer_for_task = Arc::clone(&step.writer);
        let task = async_runtime::spawn(async move {
            let Ok(stream) = TcpStream::connect(&address).await else {
                return;
            };
            let (mut reader, writer) = stream.into_split();
            *writer_for_task.lock().await = Some(writer);

            let mut read_buffer = vec![0_u8; 1024];
            let mut packet_buffer = Vec::<u8>::new();

            while running.load(Ordering::Relaxed) {
                match reader.read(&mut read_buffer).await {
                    Ok(0) => break,
                    Ok(size) => {
                        if Self::publish_received(
                            &context_for_task,
                            &mut packet_buffer,
                            end_flag.as_deref(),
                            &read_buffer[..size],
                        )
                        .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            *writer_for_task.lock().await = None;
        });

        if let Ok(mut current_task) = step.task.lock() {
            *current_task = Some(task);
        }

        Ok(step)
    }

    /// 根据是否配置结束符，发布原始读取数据或完整数据包。
    fn publish_received(
        context: &BaseStepContext,
        packet_buffer: &mut Vec<u8>,
        end_flag: Option<&[u8]>,
        received: &[u8],
    ) -> Result<(), String> {
        if let Some(flag) = end_flag {
            packet_buffer.extend_from_slice(received);
            while let Some(index) = find_bytes(packet_buffer, flag) {
                let packet_end = index + flag.len();
                let payload = packet_buffer[..packet_end].to_vec();
                packet_buffer.drain(..packet_end);
                context.write_up(payload)?;
            }
        } else {
            context.write_up(received.to_vec())?;
        }
        Ok(())
    }
}

impl BaseStep for TcpClientStep {
    /// 接收上级下行消息并写入 TCP 连接。
    fn read_up(&self, step_msg: StepMsg<Value>) {
        let payload = match value_to_bytes(&step_msg.msg) {
            Ok(payload) => payload,
            Err(err) => {
                eprintln!("tcpclientstep ignored invalid message: {err}");
                return;
            }
        };
        if payload.is_empty() {
            return;
        }

        let writer = Arc::clone(&self.writer);
        async_runtime::spawn(async move {
            let mut current_writer = writer.lock().await;
            let Some(writer) = current_writer.as_mut() else {
                return;
            };
            if writer.write_all(&payload).await.is_ok() {
                let _ = writer.flush().await;
            }
        });
    }
}

impl StepManifestProvider for TcpClientStep {
    /// 返回 TCP 客户端步骤元数据。
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "TcpClientStep".to_string(),
            name: "TCP 客户端".to_string(),
            description: "主动连接远端 TCP 服务，读取上级消息并写入连接，读取返回数据后向上级发布"
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
    /// 停止 TCP 客户端后台任务。
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);

        if let Ok(mut task) = self.task.lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
    }
}
