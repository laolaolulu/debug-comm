use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{
    find_bytes, parse_hex_end_flag, value_to_bytes, MsgType, StepManifest, WorkflowNode,
};
use crate::step::workflow::Workflow;
use serde::{Deserialize, Serialize};
use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::async_runtime::{self, JoinHandle};

/// 串口步骤节点 data 结构。
/// 该结构会直接对应前端工作流节点中的 data 字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialPortStepData {
    /// 节点显示名称。
    pub name: String,
    /// 节点说明。
    #[serde(default)]
    pub description: String,
    /// 可选的 16 进制结束符，例如 0A0D。为空时读到多少就发布多少。
    #[serde(default)]
    pub end_flag: Option<String>,
    /// 串口号，例如 COM1。
    pub port_name: String,
    /// 波特率。
    pub baud_rate: u32,
    /// 数据位，常见值为 5/6/7/8。
    #[serde(default = "default_data_bits")]
    pub data_bits: u8,
    /// 停止位，支持 1 或 2。
    #[serde(default = "default_stop_bits")]
    pub stop_bits: u8,
    /// 校验位：none/odd/even。
    #[serde(default = "default_parity")]
    pub parity: String,
    /// 控制流：none/software/hardware。
    #[serde(default = "default_flow_control")]
    pub flow_control: String,
}

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

/// 串口步骤。
/// 1. 订阅来自上级步骤的消息。
/// 2. 收到消息后写入串口。
/// 3. 从串口读取到数据后，再向上级发布消息。
pub struct SerialPortStep {
    context: BaseStepContext,
    running: Arc<AtomicBool>,
    write_task: Mutex<Option<JoinHandle<()>>>,
    read_task: Mutex<Option<JoinHandle<()>>>,
}

impl SerialPortStep {
    /// 创建并启动串口步骤。
    pub fn new(node: WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        let context = BaseStepContext::new(node, Arc::clone(&workflow));
        let data = context
            .node
            .data
            .parse::<SerialPortStepData>()
            .map_err(|err| format!("serialportstep[{}] invalid data: {err}", context.id()))?;
        let end_flag = parse_hex_end_flag(data.end_flag.as_deref())
            .map_err(|err| format!("serialportstep[{}] invalid end_flag: {err}", context.id()))?;

        // 串口 builder 集中应用文档中定义的通信参数。
        // 这里把字符串配置转换成 serialport crate 的枚举，避免前端直接依赖 Rust 枚举名。
        let writer = serialport::new(&data.port_name, data.baud_rate)
            .data_bits(Self::parse_data_bits(data.data_bits)?)
            .stop_bits(Self::parse_stop_bits(data.stop_bits)?)
            .parity(Self::parse_parity(&data.parity)?)
            .flow_control(Self::parse_flow_control(&data.flow_control)?)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|err| format!("open serial port {} failed: {err}", data.port_name))?;
        let mut reader = writer
            .try_clone()
            .map_err(|err| format!("clone serial port {} failed: {err}", data.port_name))?;
        let writer = Arc::new(Mutex::new(writer));

        let step = Arc::new(Self {
            context,
            running: Arc::new(AtomicBool::new(true)),
            write_task: Mutex::new(None),
            read_task: Mutex::new(None),
        });

        let mut subscription = workflow.subscribe_step(step.id().to_string(), MsgType::Down);
        let writer_for_task = Arc::clone(&writer);
        let running_for_write = Arc::clone(&step.running);
        let write_task = async_runtime::spawn(async move {
            while running_for_write.load(Ordering::Relaxed) {
                let Some(step_msg) = subscription.rx.recv().await else {
                    break;
                };
                let payload = value_to_bytes(&step_msg.msg);
                if payload.is_empty() {
                    continue;
                }

                // 串口写入使用阻塞接口，这里直接串行写入即可。
                if let Ok(mut port) = writer_for_task.lock() {
                    let _ = port.write_all(&payload);
                    let _ = port.flush();
                }
            }
        });

        let workflow_for_read = Arc::downgrade(&workflow);
        let step_id = step.id().to_string();
        let running_for_read = Arc::clone(&step.running);
        let read_task = async_runtime::spawn_blocking(move || {
            let mut buffer = vec![0_u8; 1024];
            let mut packet_buffer = Vec::<u8>::new();

            while running_for_read.load(Ordering::Relaxed) {
                match reader.read(&mut buffer) {
                    Ok(size) if size > 0 => {
                        let received = &buffer[..size];

                        if let Some(flag) = &end_flag {
                            // 配置了结束符时，读任务会把多次读取结果累积到 packet_buffer，
                            // 每发现一个完整包就发布一次，尽量降低串口粘包/拆包对上层步骤的影响。
                            packet_buffer.extend_from_slice(received);
                            while let Some(index) = find_bytes(&packet_buffer, flag) {
                                let packet_end = index + flag.len();
                                let payload = packet_buffer[..packet_end].to_vec();
                                packet_buffer.drain(..packet_end);
                                if let Some(workflow) = workflow_for_read.upgrade() {
                                    let _ = workflow.publish(step_id.clone(), MsgType::Up, payload);
                                } else {
                                    return;
                                }
                            }
                        } else {
                            let payload = received.to_vec();
                            if let Some(workflow) = workflow_for_read.upgrade() {
                                let _ = workflow.publish(step_id.clone(), MsgType::Up, payload);
                            } else {
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

        if let Ok(mut task) = step.write_task.lock() {
            *task = Some(write_task);
        }
        if let Ok(mut task) = step.read_task.lock() {
            *task = Some(read_task);
        }

        Ok(step)
    }

    /// 将前端配置的数据位数字转换为 serialport crate 的枚举。
    fn parse_data_bits(value: u8) -> Result<serialport::DataBits, String> {
        match value {
            5 => Ok(serialport::DataBits::Five),
            6 => Ok(serialport::DataBits::Six),
            7 => Ok(serialport::DataBits::Seven),
            8 => Ok(serialport::DataBits::Eight),
            _ => Err(format!("unsupported data_bits: {value}")),
        }
    }

    /// 将前端配置的停止位数字转换为 serialport crate 的枚举。
    fn parse_stop_bits(value: u8) -> Result<serialport::StopBits, String> {
        match value {
            1 => Ok(serialport::StopBits::One),
            2 => Ok(serialport::StopBits::Two),
            _ => Err(format!("unsupported stop_bits: {value}")),
        }
    }

    /// 将字符串校验位配置转换为 serialport crate 的枚举。
    fn parse_parity(value: &str) -> Result<serialport::Parity, String> {
        match value.to_lowercase().as_str() {
            "none" | "no" => Ok(serialport::Parity::None),
            "odd" => Ok(serialport::Parity::Odd),
            "even" => Ok(serialport::Parity::Even),
            _ => Err(format!("unsupported parity: {value}")),
        }
    }

    /// 将字符串控制流配置转换为 serialport crate 的枚举。
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
    fn context(&self) -> &BaseStepContext {
        &self.context
    }
}

impl StepManifestProvider for SerialPortStep {
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "SerialPortStep".to_string(),
            name: "串口通信".to_string(),
            description: "订阅上级消息并写入串口，串口收到数据后再向上级发布消息".to_string(),
            default_data: serde_json::json!([
                        {
                            "title": "结束符(HEX)",
                            "dataIndex": "end_flag",
                            "valueType": "text",
                            "initialValue": null
                        },
                        {
                            "title": "串口号",
                            "dataIndex": "port_name",
                            "valueType": "text",
                            "initialValue": "COM1"
                        },
                        {
                            "title": "波特率",
                            "dataIndex": "baud_rate",
                            "valueType": "digit",
                            "initialValue": 9600
                        },
                        {
                            "title": "数据位",
                            "dataIndex": "data_bits",
                            "valueType": "digit",
                            "initialValue": default_data_bits()
                        },
                        {
                            "title": "停止位",
                            "dataIndex": "stop_bits",
                            "valueType": "digit",
                            "initialValue": default_stop_bits()
                        },
                        {
                            "title": "校验位",
                            "dataIndex": "parity",
                            "valueType": "select",
                            "initialValue": default_parity(),
                            "valueEnum": {
                                "none": { "text": "None" },
                                "odd": { "text": "Odd" },
                                "even": { "text": "Even" }
                            }
                        },
                        {
                            "title": "控制流",
                            "dataIndex": "flow_control",
                            "valueType": "select",
                            "initialValue": default_flow_control(),
                            "valueEnum": {
                                "none": { "text": "None" },
                                "software": { "text": "Software" },
                                "hardware": { "text": "Hardware" }
                            }
                        }

            ]),
        }
    }
}

impl Drop for SerialPortStep {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);

        if let Ok(mut task) = self.write_task.lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
        if let Ok(mut task) = self.read_task.lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
    }
}
