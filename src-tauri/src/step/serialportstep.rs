use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{
    find_bytes, parse_hex_end_flag, value_to_bytes, StepManifest, StepManifestData, StepMsg,
    WorkflowNode,
};
use crate::step::workflow::Workflow;
use serde_json::Value;
use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::async_runtime::{self, JoinHandle};

fn default_data_bits() -> u8 {
    8
}

fn default_stop_bits() -> u8 {
    1
}

fn default_parity() -> String {
    "none".to_string()
}

fn default_flow_control() -> String {
    "none".to_string()
}

pub struct SerialPortStep {
    context: BaseStepContext,
    running: Arc<AtomicBool>,
    writer: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    read_task: Mutex<Option<JoinHandle<()>>>,
}

impl SerialPortStep {
    pub fn new(node: &WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        let context = BaseStepContext::new(node, Arc::clone(&workflow));
        let end_flag =
            parse_hex_end_flag(context.get_optional_data::<String>("end_flag")?.as_deref())
                .map_err(|err| {
                    format!("serialportstep[{}] invalid end_flag: {err}", context.id())
                })?;
        let port_name = context.get_data::<String>("port_name")?;
        let baud_rate = context.get_data::<u32>("baud_rate")?;
        let data_bits = context.get_data::<u8>("data_bits")?;
        let stop_bits = context.get_data::<u8>("stop_bits")?;
        let parity = context.get_data::<String>("parity")?;
        let flow_control = context.get_data::<String>("flow_control")?;

        let writer = serialport::new(&port_name, baud_rate)
            .data_bits(Self::parse_data_bits(data_bits)?)
            .stop_bits(Self::parse_stop_bits(stop_bits)?)
            .parity(Self::parse_parity(&parity)?)
            .flow_control(Self::parse_flow_control(&flow_control)?)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|err| format!("open serial port {port_name} failed: {err}"))?;
        let mut reader = writer
            .try_clone()
            .map_err(|err| format!("clone serial port {port_name} failed: {err}"))?;
        let writer = Arc::new(Mutex::new(writer));

        let step = Arc::new(Self {
            context,
            running: Arc::new(AtomicBool::new(true)),
            writer: Arc::clone(&writer),
            read_task: Mutex::new(None),
        });

        let context_for_read = step.context.clone();
        let running_for_read = Arc::clone(&step.running);
        let read_task = async_runtime::spawn_blocking(move || {
            let mut buffer = vec![0_u8; 1024];
            let mut packet_buffer = Vec::<u8>::new();

            while running_for_read.load(Ordering::Relaxed) {
                match reader.read(&mut buffer) {
                    Ok(size) if size > 0 => {
                        let received = &buffer[..size];

                        if let Some(flag) = &end_flag {
                            packet_buffer.extend_from_slice(received);
                            while let Some(index) = find_bytes(&packet_buffer, flag) {
                                let packet_end = index + flag.len();
                                let payload = packet_buffer[..packet_end].to_vec();
                                packet_buffer.drain(..packet_end);
                                if context_for_read.write_up(payload).is_err() {
                                    return;
                                }
                            }
                        } else {
                            let payload = received.to_vec();
                            if context_for_read.write_up(payload).is_err() {
                                return;
                            }
                        }
                    }
                    Ok(_) => continue,
                    Err(err) if err.kind() == ErrorKind::TimedOut => continue,
                    Err(_) => break,
                }
            }
        });

        if let Ok(mut task) = step.read_task.lock() {
            *task = Some(read_task);
        }

        Ok(step)
    }

    fn parse_data_bits(value: u8) -> Result<serialport::DataBits, String> {
        match value {
            5 => Ok(serialport::DataBits::Five),
            6 => Ok(serialport::DataBits::Six),
            7 => Ok(serialport::DataBits::Seven),
            8 => Ok(serialport::DataBits::Eight),
            _ => Err(format!("unsupported data_bits: {value}")),
        }
    }

    fn parse_stop_bits(value: u8) -> Result<serialport::StopBits, String> {
        match value {
            1 => Ok(serialport::StopBits::One),
            2 => Ok(serialport::StopBits::Two),
            _ => Err(format!("unsupported stop_bits: {value}")),
        }
    }

    fn parse_parity(value: &str) -> Result<serialport::Parity, String> {
        match value.to_lowercase().as_str() {
            "none" | "no" => Ok(serialport::Parity::None),
            "odd" => Ok(serialport::Parity::Odd),
            "even" => Ok(serialport::Parity::Even),
            _ => Err(format!("unsupported parity: {value}")),
        }
    }

    fn parse_flow_control(value: &str) -> Result<serialport::FlowControl, String> {
        match value.to_lowercase().as_str() {
            "none" | "no" => Ok(serialport::FlowControl::None),
            "software" | "xonxoff" => Ok(serialport::FlowControl::Software),
            "hardware" | "rtscts" => Ok(serialport::FlowControl::Hardware),
            _ => Err(format!("unsupported flow_control: {value}")),
        }
    }
}

impl BaseStep for SerialPortStep {
    fn read_up(&self, step_msg: StepMsg<Value>) {
        let payload = match value_to_bytes(&step_msg.msg) {
            Ok(payload) => payload,
            Err(err) => {
                eprintln!("serialportstep ignored invalid message: {err}");
                return;
            }
        };
        if payload.is_empty() {
            return;
        }

        if let Ok(mut port) = self.writer.lock() {
            let _ = port.write_all(&payload);
            let _ = port.flush();
        }
    }
}

impl StepManifestProvider for SerialPortStep {
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "SerialPortStep",
            data: StepManifestData {
                name: "串口通信",
                description: "读取上级消息并写入串口，串口收到数据后再向上级发布消息",
                columns: vec![
                    serde_json::json!({
                        "title": "结束符(HEX)",
                        "dataIndex": "end_flag",
                        "valueType": "text",
                        "initialValue": null
                    }),
                    serde_json::json!({
                        "title": "串口号",
                        "dataIndex": "port_name",
                        "valueType": "text",
                        "initialValue": "COM1"
                    }),
                    serde_json::json!({
                        "title": "波特率",
                        "dataIndex": "baud_rate",
                        "valueType": "digit",
                        "initialValue": 9600
                    }),
                    serde_json::json!({
                        "title": "数据位",
                        "dataIndex": "data_bits",
                        "valueType": "digit",
                        "initialValue": default_data_bits()
                    }),
                    serde_json::json!({
                        "title": "停止位",
                        "dataIndex": "stop_bits",
                        "valueType": "digit",
                        "initialValue": default_stop_bits()
                    }),
                    serde_json::json!({
                        "title": "校验位",
                        "dataIndex": "parity",
                        "valueType": "select",
                        "initialValue": default_parity(),
                        "valueEnum": {
                            "none": { "text": "None" },
                            "odd": { "text": "Odd" },
                            "even": { "text": "Even" }
                        }
                    }),
                    serde_json::json!({
                        "title": "控制流",
                        "dataIndex": "flow_control",
                        "valueType": "select",
                        "initialValue": default_flow_control(),
                        "valueEnum": {
                            "none": { "text": "None" },
                            "software": { "text": "Software" },
                            "hardware": { "text": "Hardware" }
                        }
                    }),
                ],
            },
        }
    }
}
impl Drop for SerialPortStep {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);

        if let Ok(mut task) = self.read_task.lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
    }
}
