use crate::step::basestep::{BaseStep, BaseStepContext, StepManifestProvider};
use crate::step::model::{MsgType, StepManifest, StepManifestData, StepMsg, WorkflowNode};
use crate::step::workflow::Workflow;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsTypedArray;
use boa_engine::{Context, JsResult, JsString, JsValue, Source};
use serde_json::Value;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const DEFAULT_SCRIPT: &str = r#"function read_up(stepMsg) {
  write_down(stepMsg.msg);
}

function read_down(stepMsg) {
  write_up(stepMsg.msg);
}
"#;

const JOB_POLL_INTERVAL: Duration = Duration::from_millis(10);

enum JavaScriptStepEvent {
    Message(StepMsg<Value>),
    Stop,
}

pub struct JavaScriptStep {
    tx: Sender<JavaScriptStepEvent>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl JavaScriptStep {
    /// 创建 JS 脚本步骤并启动脚本运行线程。
    pub fn new(node: &WorkflowNode, workflow: Arc<Workflow>) -> Result<Arc<Self>, String> {
        let context = BaseStepContext::new(node, workflow);
        let script = context.get_data::<String>("script")?;
        let (tx, rx) = mpsc::channel::<JavaScriptStepEvent>();
        let (init_tx, init_rx) = mpsc::channel::<Result<(), String>>();
        let worker_context = context.clone();
        let worker = thread::spawn(move || {
            run_worker(worker_context, script, rx, init_tx);
        });

        match init_rx.recv() {
            Ok(Ok(())) => Ok(Arc::new(Self {
                tx,
                worker: Mutex::new(Some(worker)),
            })),
            Ok(Err(err)) => {
                let _ = worker.join();
                Err(err)
            }
            Err(err) => {
                let _ = worker.join();
                Err(format!(
                    "javascriptstep[{}] init failed: {err}",
                    context.id()
                ))
            }
        }
    }

    /// 将工作流消息发送给脚本运行线程。
    fn send_message(&self, step_msg: StepMsg<Value>) {
        let _ = self.tx.send(JavaScriptStepEvent::Message(step_msg));
    }
}

impl BaseStep for JavaScriptStep {
    /// 上级消息下发到脚本步骤时触发 JS read_up。
    fn read_up(&self, step_msg: StepMsg<Value>) {
        self.send_message(step_msg);
    }

    /// 下级消息上行到脚本步骤时触发 JS read_down。
    fn read_down(&self, step_msg: StepMsg<Value>) {
        self.send_message(step_msg);
    }
}

impl StepManifestProvider for JavaScriptStep {
    /// 返回 JS 脚本步骤元数据。
    fn manifest() -> StepManifest {
        StepManifest {
            r#type: "JavaScriptStep".into(),
            data: StepManifestData {
                name: "JS 自动化脚本".into(),
                description: "使用 JavaScript 处理上下行消息".into(),
                columns: vec![serde_json::json!({
                    "title": "JS 自动化脚本",
                    "dataIndex": "script",
                    "valueType": "textarea",
                    "fieldProps": { "autoSize": true },
                    "initialValue": DEFAULT_SCRIPT
                })],
            },
        }
    }
}

impl Drop for JavaScriptStep {
    /// 停止脚本运行线程。
    fn drop(&mut self) {
        let _ = self.tx.send(JavaScriptStepEvent::Stop);

        if let Ok(mut worker) = self.worker.lock() {
            if let Some(handle) = worker.take() {
                let _ = handle.join();
            }
        }
    }
}

/// 初始化 Boa 上下文并处理脚本消息循环。
fn run_worker(
    step_context: BaseStepContext,
    script: String,
    rx: Receiver<JavaScriptStepEvent>,
    init_tx: Sender<Result<(), String>>,
) {
    let mut context = Context::default();
    let step_id = step_context.id().to_string();
    let init_result = register_runtime(&mut context, &step_id)
        .and_then(|_| {
            register_write_function(&mut context, "write_up", MsgType::Up, step_context.clone())
        })
        .and_then(|_| {
            register_write_function(
                &mut context,
                "write_down",
                MsgType::Down,
                step_context.clone(),
            )
        })
        .and_then(|_| {
            context
                .eval(Source::from_bytes(script.as_str()))
                .map(|_| ())
                .map_err(|err| {
                    format!(
                        "javascriptstep[{}] init script failed: {err}",
                        step_context.id()
                    )
                })
        });

    if init_tx.send(init_result.clone()).is_err() || init_result.is_err() {
        return;
    }

    loop {
        run_runtime_jobs(&mut context, &step_id, "runtime");

        let event = match rx.recv_timeout(JOB_POLL_INTERVAL) {
            Ok(event) => Some(event),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };

        if let Some(event) = event {
            match event {
                JavaScriptStepEvent::Message(step_msg) => {
                    if let Err(err) = call_script_reader(&mut context, &step_msg) {
                        eprintln!(
                            "javascriptstep[{}] message ignored: {err}",
                            step_context.id()
                        );
                    }
                    run_runtime_jobs(&mut context, &step_id, "message");
                }
                JavaScriptStepEvent::Stop => break,
            }
        }
    }
}

/// 注册脚本可调用的 write_up 或 write_down 函数。
fn register_runtime(context: &mut Context, step_id: &str) -> Result<(), String> {
    boa_runtime::register(
        boa_runtime::extensions::ConsoleExtension::default(),
        None,
        context,
    )
    .map_err(|err| format!("javascriptstep[{step_id}] register runtime failed: {err}"))
}

fn run_runtime_jobs(context: &mut Context, step_id: &str, source: &str) {
    if let Err(err) = context.run_jobs() {
        eprintln!("javascriptstep[{step_id}] {source} jobs ignored: {err}");
    }
}

fn register_write_function(
    context: &mut Context,
    name: &str,
    action: MsgType,
    step_context: BaseStepContext,
) -> Result<(), String> {
    let function_context = step_context.clone();
    let native_function = unsafe {
        NativeFunction::from_closure(move |_, args, context| {
            let msg = args
                .first()
                .map(|value| js_value_to_json(value, context))
                .transpose()?
                .unwrap_or(Value::Null);
            let result = match action {
                MsgType::Up => function_context.write_up(msg),
                MsgType::Down => function_context.write_down(msg),
            };

            match result {
                Ok(count) => Ok(JsValue::from(count as i32)),
                Err(err) => {
                    eprintln!(
                        "javascriptstep[{}] write failed: {err}",
                        function_context.id()
                    );
                    Ok(JsValue::undefined())
                }
            }
        })
    };

    context
        .register_global_builtin_callable(JsString::from(name), 1, native_function)
        .map_err(|err| {
            format!(
                "javascriptstep[{}] register {name} failed: {err}",
                step_context.id()
            )
        })
}

/// 将脚本返回值转换为工作流消息使用的 JSON 值。
fn js_value_to_json(value: &JsValue, context: &mut Context) -> JsResult<Value> {
    if let Some(object) = value.as_object() {
        if let Ok(typed_array) = JsTypedArray::from_object(object.clone()) {
            let length = typed_array.length(context)?;
            let mut items = Vec::with_capacity(length);

            for index in 0..length {
                let item = typed_array.at(index as i64, context)?.to_u32(context)?;
                items.push(Value::from(item));
            }

            return Ok(Value::Array(items));
        }
    }

    Ok(value.to_json(context)?.unwrap_or(Value::Null))
}

/// 调用脚本中的 read_up 或 read_down 回调。
fn call_script_reader(context: &mut Context, step_msg: &StepMsg<Value>) -> Result<(), String> {
    let reader_name = match step_msg.action {
        MsgType::Down => "read_up",
        MsgType::Up => "read_down",
    };
    let msg_json = serde_json::to_string(&serde_json::json!({
        "step_id": &step_msg.step_id,
        "action": step_msg.action as u8,
        "msg": &step_msg.msg,
    }))
    .map_err(|err| err.to_string())?;
    let msg_literal = serde_json::to_string(&msg_json).map_err(|err| err.to_string())?;
    let source = format!(
        r#"if (typeof {reader_name} === "function") {{
  {reader_name}(JSON.parse({msg_literal}));
}}"#
    );

    context
        .eval(Source::from_bytes(source.as_str()))
        .map(|_| ())
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_supports_utf8_text_encoder() {
        let mut context = Context::default();

        register_runtime(&mut context, "test").expect("runtime should register");
        context
            .eval(Source::from_bytes(
                r#"
const bytes = new TextEncoder().encode("A\u4e2d\ud83d\ude00");
if (!(bytes instanceof Uint8Array)) {
  throw new Error("TextEncoder must return Uint8Array");
}
if (Array.from(bytes).join(",") !== "65,228,184,173,240,159,152,128") {
  throw new Error("TextEncoder must encode utf-8");
}
if (new TextDecoder().decode(bytes) !== "A\u4e2d\ud83d\ude00") {
  throw new Error("TextDecoder must decode utf-8");
}
"#,
            ))
            .expect("runtime should encode and decode UTF-8");
    }

    #[test]
    fn write_value_converts_uint8array_to_json_bytes() {
        let mut context = Context::default();

        register_runtime(&mut context, "test").expect("runtime should register");
        let value = context
            .eval(Source::from_bytes(r#"new TextEncoder().encode("Hello")"#))
            .expect("TextEncoder should return a value");
        let json = js_value_to_json(&value, &mut context).expect("Uint8Array should convert");

        assert_eq!(json, serde_json::json!([72, 101, 108, 108, 111]));
    }

    #[test]
    fn runtime_runs_timeout_and_interval_callbacks() {
        let mut context = Context::default();

        register_runtime(&mut context, "test").expect("runtime should register");
        context
            .eval(Source::from_bytes(
                r#"
var timeoutCount = 0;
var intervalCount = 0;
setTimeout(function (value) {
  timeoutCount += value;
}, 1, 2);
var intervalId = setInterval(function () {
  intervalCount += 1;
  if (intervalCount === 2) {
    clearInterval(intervalId);
  }
}, 1);
"#,
            ))
            .expect("timers should initialize");

        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(2));
            run_runtime_jobs(&mut context, "test", "test");
        }

        context
            .eval(Source::from_bytes(
                r#"
if (timeoutCount !== 2) {
  throw new Error("setTimeout callback did not run");
}
if (intervalCount !== 2) {
  throw new Error("setInterval callback did not repeat and clear");
}
"#,
            ))
            .expect("timers should run callbacks");
    }
}
