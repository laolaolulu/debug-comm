use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{
    find_bytes, parse_hex_end_flag, value_to_bytes, StepManifest, StepManifestData, StepMsg,
    WorkflowNode,
};
use crate::step::workflow::Workflow;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::model::{WorkflowDefinition, WorkflowNodeData};
    use std::collections::HashMap;
    use std::net::TcpListener;

    #[test]
    fn new_returns_error_when_connection_fails() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("free port should bind");
        let port = listener
            .local_addr()
            .expect("local address should exist")
            .port();
        drop(listener);

        let workflow = Workflow::new_for_test(WorkflowDefinition {
            id: "tcp-client-connect-fail".to_string(),
            name: "TCP client connect fail".to_string(),
            description: None,
            nodes: Vec::new(),
            edges: Vec::new(),
        });
        let node = WorkflowNode {
            id: "tcp-client".to_string(),
            r#type: "TcpClientStep".to_string(),
            data: WorkflowNodeData {
                name: "TCP Client".to_string(),
                description: String::new(),
                columns: Vec::new(),
                params: HashMap::from([
                    ("end_flag".to_string(), Value::String(String::new())),
                    ("host".to_string(), Value::String("127.0.0.1".to_string())),
                    ("port".to_string(), Value::from(port)),
                ]),
            },
        };

        let error = match TcpClientStep::new(&node, workflow) {
            Ok(_) => panic!("connection should fail"),
            Err(error) => error,
        };

        assert!(error.contains("connect 127.0.0.1"));
    }
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
        let (init_tx, init_rx) = mpsc::channel::<Result<(), String>>();
        let task = async_runtime::spawn(async move {
            let stream = match TcpStream::connect(&address).await {
                Ok(stream) => stream,
                Err(err) => {
                    let _ = init_tx.send(Err(format!("connect {address} failed: {err}")));
                    return;
                }
            };

            let (mut reader, writer) = stream.into_split();
            *writer_for_task.lock().await = Some(writer);
            if init_tx.send(Ok(())).is_err() {
                return;
            }

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

        match init_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                task.abort();
                return Err(err);
            }
            Err(RecvTimeoutError::Timeout) => {
                task.abort();
                return Err(format!("connect {host}:{port} failed: timed out"));
            }
            Err(RecvTimeoutError::Disconnected) => {
                task.abort();
                return Err(format!("connect {host}:{port} failed: task stopped"));
            }
        }

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
