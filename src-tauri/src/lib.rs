mod receive_log;
pub mod step;

use receive_log::{append_record, clear_records, create_record, read_records, ReceiveLogRecord};
use serde_json::Value;
use serialport::available_ports;
use std::path::PathBuf;
use step::model::{MsgType, StepMsg, WorkflowDefinition};
use step::workflow::Workflow;
use tauri::{AppHandle, Emitter, Manager};

/// 启动工作流：
/// 前端传入工作流设计器导出的 JSON 字符串，
/// Rust 侧完成反序列化并创建工作流实例，同时注册到全局实例集合中。
#[tauri::command]
fn start_workflow(app: AppHandle, json: &str) -> Result<String, String> {
    let (workflow, output_step_ids) = start_workflow_instance(json)?;
    let workflow_id = workflow.id().to_string();

    spawn_output_step_bridges(app, workflow, output_step_ids);

    Ok(workflow_id)
}

fn start_workflow_instance(json: &str) -> Result<(std::sync::Arc<Workflow>, Vec<String>), String> {
    let definition =
        serde_json::from_str::<WorkflowDefinition>(json).map_err(|err| err.to_string())?;

    let output_step_ids = definition
        .nodes
        .iter()
        .filter(|node| node.r#type.eq_ignore_ascii_case("disoutputstep"))
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();

    Workflow::remove(&definition.id);

    let workflow = Workflow::new(definition);
    workflow.run()?;
    Workflow::register_running(&workflow);

    Ok((workflow, output_step_ids))
}

fn spawn_output_step_bridges(
    app: AppHandle,
    workflow: std::sync::Arc<Workflow>,
    output_step_ids: Vec<String>,
) {
    let workflow_id = workflow.id().to_string();
    for step_id in output_step_ids {
        let app = app.clone();
        let workflow = workflow.clone();
        let workflow_id = workflow_id.clone();
        tauri::async_runtime::spawn(async move {
            let mut subscription = workflow.subscribe_step_related(step_id.clone());
            drop(workflow);
            while let Some(StepMsg {
                step_id: source_step_id,
                msg,
                ..
            }) = subscription.rx.recv().await
            {
                let record =
                    create_record(workflow_id.clone(), step_id.clone(), source_step_id, msg);
                match receive_log_base_dir(&app)
                    .and_then(|base_dir| append_record(&base_dir, &record))
                {
                    Ok(()) => {
                        let _ = app.emit("workflow-step-message", record);
                    }
                    Err(err) => {
                        eprintln!("failed to persist receive log: {err}");
                        let _ = app.emit("workflow-step-message", record);
                    }
                }
            }
        });
    }
}

#[tauri::command]
fn publish_step_message(workflow_id: &str, step_id: &str, msg: Value) -> Result<(), String> {
    let workflow =
        Workflow::get(workflow_id).ok_or_else(|| format!("workflow not found: {workflow_id}"))?;
    workflow.publish(step_id.to_string(), MsgType::Down, msg)?;
    Ok(())
}

/// 停止工作流：
/// 前端传入工作流 id，
/// Rust 侧从全局实例集合中移除对应实例，移除后如果没有其他引用，实例会被自动销毁。
#[tauri::command]
fn stop_workflow(id: &str) -> Result<(), String> {
    if Workflow::get(id).is_none() {
        return Err(format!("workflow not found: {id}"));
    }

    Workflow::remove(id);
    Ok(())
}

/// 获取当前所有执行中的工作流 id 集合。
/// 数据来源就是当前进程中的全局工作流实例集合。
#[tauri::command]
fn get_workflow_ids() -> Vec<String> {
    Workflow::list_ids()
}

/// 获取所有可创建的步骤类型定义。
/// 前端 StepList 可直接使用该数据生成拖拽列表。
#[tauri::command]
fn get_step_manifests() -> serde_json::Value {
    serde_json::to_value(Workflow::available_steps()).unwrap_or_default()
}

/// 查询当前系统可用串口列表。
/// 前端可用于串口步骤的下拉选择，同时仍允许用户手动输入。
#[tauri::command]
fn get_serial_ports() -> Result<Vec<String>, String> {
    available_ports()
        .map(|ports| ports.into_iter().map(|port| port.port_name).collect())
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_receive_logs(
    app: AppHandle,
    workflow_id: &str,
    step_id: &str,
    before: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ReceiveLogRecord>, String> {
    let base_dir = receive_log_base_dir(&app)?;
    read_records(&base_dir, workflow_id, step_id, before.as_deref(), limit)
}

#[tauri::command]
fn clear_receive_logs(app: AppHandle, workflow_id: &str, step_id: &str) -> Result<(), String> {
    let base_dir = receive_log_base_dir(&app)?;
    clear_records(&base_dir, workflow_id, step_id)
}

fn receive_log_base_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Mutex, OnceLock};

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn workflow_definition(id: &str) -> WorkflowDefinition {
        serde_json::from_value(json!({
            "id": id,
            "name": "test workflow",
            "nodes": [],
            "edges": []
        }))
        .unwrap()
    }

    fn workflow_json(id: &str) -> String {
        json!({
            "id": id,
            "name": "test workflow",
            "nodes": [],
            "edges": []
        })
        .to_string()
    }

    fn failing_workflow_json(id: &str) -> String {
        json!({
            "id": id,
            "name": "failing workflow",
            "nodes": [
                {
                    "id": "serial-1",
                    "type": "SerialPortStep",
                    "position": { "x": 0.0, "y": 0.0 },
                    "data": {
                        "name": "Serial"
                    }
                }
            ],
            "edges": []
        })
        .to_string()
    }

    #[test]
    fn workflow_new_does_not_register_running_instance() {
        let _guard = test_lock();
        let id = "test-new-does-not-register";
        Workflow::remove(id);

        let workflow = Workflow::new(workflow_definition(id));

        assert!(!Workflow::list_ids().contains(&id.to_string()));

        drop(workflow);
        Workflow::remove(id);
    }

    #[test]
    fn register_running_makes_workflow_queryable() {
        let _guard = test_lock();
        let id = "test-register-running";
        Workflow::remove(id);

        let workflow = Workflow::new(workflow_definition(id));
        Workflow::register_running(&workflow);

        assert!(Workflow::get(id).is_some());
        assert!(Workflow::list_ids().contains(&id.to_string()));

        Workflow::remove(id);
    }

    #[test]
    fn remove_deletes_running_workflow() {
        let _guard = test_lock();
        let id = "test-remove-running";
        Workflow::remove(id);

        let workflow = Workflow::new(workflow_definition(id));
        Workflow::register_running(&workflow);
        Workflow::remove(id);

        assert!(Workflow::get(id).is_none());
        assert!(!Workflow::list_ids().contains(&id.to_string()));
    }

    #[test]
    fn invalid_json_does_not_register_workflow() {
        let _guard = test_lock();
        let ids_before = Workflow::list_ids();

        let result = start_workflow_instance("{not valid json");

        assert!(result.is_err());
        assert_eq!(Workflow::list_ids(), ids_before);
    }

    #[test]
    fn failed_start_does_not_register_workflow() {
        let _guard = test_lock();
        let id = "test-failed-start";
        Workflow::remove(id);

        let result = start_workflow_instance(&failing_workflow_json(id));

        assert!(result.is_err());
        assert!(Workflow::get(id).is_none());
        assert!(!Workflow::list_ids().contains(&id.to_string()));
    }

    #[test]
    fn repeated_start_keeps_single_running_id() {
        let _guard = test_lock();
        let id = "test-repeated-start";
        Workflow::remove(id);

        let first = start_workflow_instance(&workflow_json(id)).unwrap();
        let second = start_workflow_instance(&workflow_json(id)).unwrap();

        assert_eq!(
            Workflow::list_ids()
                .into_iter()
                .filter(|running_id| running_id == id)
                .count(),
            1
        );

        drop(first);
        drop(second);
        Workflow::remove(id);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            start_workflow,
            stop_workflow,
            publish_step_message,
            get_workflow_ids,
            get_receive_logs,
            clear_receive_logs,
            get_step_manifests,
            get_serial_ports
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
