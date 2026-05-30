use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{
    find_bytes, parse_hex_end_flag, value_to_bytes, StepManifest, StepManifestData, StepMsg,
    WorkflowNode,
};
use crate::step::workflow::Workflow;
use serde_json::Value;
use std::collections::HashMap;
use std::net::TcpListener as StdTcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::async_runtime::{self, JoinHandle};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

fn default_max_read_bytes() -> usize {
    1024
}

type ClientWriters = Arc<Mutex<HashMap<usize, mpsc::UnboundedSender<Vec<u8>>>>>;
type ClientTasks = Arc<Mutex<Vec<JoinHandle<()>>>>;

pub struct TcpServerStep {
    context: BaseStepContext,
    running: Arc<AtomicBool>,
    clients: ClientWriters,
    accept_task: Mutex<Option<JoinHandle<()>>>,
    client_tasks: ClientTasks,
}

impl TcpServerStep {
    pub fn new(node: &WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        let context = BaseStepContext::new(node, Arc::clone(&workflow));
        let end_flag =
            parse_hex_end_flag(context.get_optional_data::<String>("end_flag")?.as_deref())
                .map_err(|err| {
                    format!("tcpserverstep[{}] invalid end_flag: {err}", context.id())
                })?;
        let bind_addr = context.get_data::<String>("bind_addr")?;
        let port = context.get_data::<u16>("port")?;
        let max_read_bytes = context
            .get_data::<usize>("max_read_bytes")
            .unwrap_or_else(|_| default_max_read_bytes());

        let address = format!("{bind_addr}:{port}");
        let std_listener = StdTcpListener::bind(&address)
            .map_err(|err| format!("bind {address} failed: {err}"))?;
        std_listener
            .set_nonblocking(true)
            .map_err(|err| format!("set {address} nonblocking failed: {err}"))?;
        let listener = TcpListener::from_std(std_listener)
            .map_err(|err| format!("create tokio listener {address} failed: {err}"))?;

        let clients: ClientWriters = Arc::new(Mutex::new(HashMap::new()));
        let step = Arc::new(Self {
            context,
            running: Arc::new(AtomicBool::new(true)),
            clients: Arc::clone(&clients),
            accept_task: Mutex::new(None),
            client_tasks: Arc::new(Mutex::new(Vec::new())),
        });

        let client_tasks = Arc::clone(&step.client_tasks);
        let context_for_accept = step.context.clone();
        let running_for_accept = Arc::clone(&step.running);
        let clients_for_accept = Arc::clone(&clients);
        let max_read_bytes = max_read_bytes.max(1);
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
                let context_for_reader = context_for_accept.clone();
                let end_flag_for_reader = end_flag.clone();

                let reader_task = async_runtime::spawn(async move {
                    let mut read_buffer = vec![0_u8; max_read_bytes];
                    let mut packet_buffer = Vec::<u8>::new();

                    loop {
                        match reader.read(&mut read_buffer).await {
                            Ok(0) => break,
                            Ok(size) => {
                                if Self::publish_received(
                                    &context_for_reader,
                                    &mut packet_buffer,
                                    end_flag_for_reader.as_deref(),
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

                    if let Ok(mut clients) = clients_for_reader.lock() {
                        clients.remove(&client_id);
                    }
                });

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

        if let Ok(mut task) = step.accept_task.lock() {
            *task = Some(accept_task);
        }

        Ok(step)
    }

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

impl BaseStep for TcpServerStep {
    fn read_up(&self, step_msg: StepMsg<Value>) {
        let payload = match value_to_bytes(&step_msg.msg) {
            Ok(payload) => payload,
            Err(err) => {
                eprintln!("tcpserverstep ignored invalid message: {err}");
                return;
            }
        };
        if payload.is_empty() {
            return;
        }

        if let Ok(clients) = self.clients.lock() {
            for client_tx in clients.values() {
                let _ = client_tx.send(payload.clone());
            }
        }
    }
}

impl StepManifestProvider for TcpServerStep {
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "TcpServerStep",
            data: StepManifestData {
                name: "TCP 服务端",
                description:
                    "监听本地 TCP 端口，接收客户端数据并发布上行消息，读取下行消息后广播写回客户端",
                columns: vec![
                    serde_json::json!({
                        "title": "结束符(HEX)",
                        "dataIndex": "end_flag",
                        "valueType": "text",
                        "initialValue": null
                    }),
                    serde_json::json!({
                        "title": "监听IP地址",
                        "dataIndex": "bind_addr",
                        "valueType": "text",
                        "initialValue": "0.0.0.0"
                    }),
                    serde_json::json!({
                        "title": "监听端口",
                        "dataIndex": "port",
                        "valueType": "digit",
                        "initialValue": 502
                    }),
                ],
            },
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
        if let Ok(mut tasks) = self.client_tasks.lock() {
            for task in tasks.drain(..) {
                task.abort();
            }
        }
    }
}
