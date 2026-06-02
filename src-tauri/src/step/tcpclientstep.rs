use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{
    find_bytes, parse_hex_end_flag, value_to_bytes, StepManifest, StepManifestData, StepMsg,
    WorkflowNode,
};
use crate::step::workflow::Workflow;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::async_runtime::{self, JoinHandle};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::Mutex as AsyncMutex;

pub struct TcpClientStep {
    context: BaseStepContext,
    running: Arc<AtomicBool>,
    writer: Arc<AsyncMutex<Option<OwnedWriteHalf>>>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl TcpClientStep {
    /// 创建 TCP 客户端步骤并启动连接读取任务。
    pub fn new(node: &WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        let context = BaseStepContext::new(node, Arc::clone(&workflow));
        let end_flag =
            parse_hex_end_flag(context.get_optional_data::<String>("end_flag")?.as_deref())
                .map_err(|err| {
                    format!("tcpclientstep[{}] invalid end_flag: {err}", context.id())
                })?;
        let host = context.get_data::<String>("host")?;
        let port = context.get_data::<u16>("port")?;

        let step = Arc::new(Self {
            context,
            running: Arc::new(AtomicBool::new(true)),
            writer: Arc::new(AsyncMutex::new(None)),
            task: Mutex::new(None),
        });

        let address = format!("{host}:{port}");
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

    /// 按结束符拆包并向上级步骤发布接收到的数据。
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
    /// 接收上级下发消息并写入 TCP 连接。
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
            r#type: "TcpClientStep".into(),
            data: StepManifestData {
                name: "TCP 客户端".into(),
                description:
                    "主动连接远端 TCP 服务，读取上级消息并写入连接，读到返回数据后向上级发布".into(),
                columns: vec![
                    serde_json::json!({ "title": "结束符(HEX)", "dataIndex": "end_flag", "valueType": "text", "initialValue": "" }),
                    serde_json::json!({ "title": "服务IP地址", "dataIndex": "host", "valueType": "text", "initialValue": "127.0.0.1" }),
                    serde_json::json!({ "title": "服务端口", "dataIndex": "port", "valueType": "digit", "initialValue": 502 }),
                ],
            },
        }
    }
}
impl Drop for TcpClientStep {
    /// 释放 TCP 客户端后台任务。
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);

        if let Ok(mut task) = self.task.lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
    }
}
