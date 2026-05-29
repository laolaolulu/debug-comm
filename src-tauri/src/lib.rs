pub mod step;

#[cfg(not(target_os = "android"))]
use serialport::available_ports;

use serde_json::Value;
use step::model::{value_to_bytes, MsgType, StepManifest, WorkflowDefinition};
use step::workflow::Workflow;
use tauri::AppHandle;

/// 启动工作流：
/// 前端传入工作流设计器导出的 JSON 字符串，
/// Rust 侧完成反序列化并创建工作流实例，同时注册到全局实例集合中。
#[tauri::command]
fn start_workflow(app: AppHandle, json: &str) -> Result<String, String> {
    let workflow = start_workflow_instance(json, Some(app))?;
    let workflow_id = workflow.id().to_string();

    Ok(workflow_id)
}

fn start_workflow_instance(
    json: &str,
    app: Option<AppHandle>,
) -> Result<std::sync::Arc<Workflow>, String> {
    let definition =
        serde_json::from_str::<WorkflowDefinition>(json).map_err(|err| err.to_string())?;

    Workflow::remove(&definition.id);

    let workflow = match app {
        Some(app) => Workflow::new_with_app(definition, app),
        None => Workflow::new(definition),
    };
    workflow.run()?;
    Workflow::register_running(&workflow);

    Ok(workflow)
}

#[tauri::command]
fn publish_step_message(workflow_id: &str, step_id: &str, msg: Value) -> Result<(), String> {
    let workflow =
        Workflow::get(workflow_id).ok_or_else(|| format!("workflow not found: {workflow_id}"))?;
    let payload = value_to_bytes(&msg)?;
    workflow.publish(step_id.to_string(), MsgType::Down, payload)?;
    Ok(())
}

/// 停止工作流：
/// 前端传入工作流 id，
/// Rust 侧从全局实例集合中移除对应实例，移除后如果没有其他引用，实例会被自动销毁。
#[tauri::command]
fn stop_workflow(id: &str) -> Result<(), String> {
    if Workflow::remove(id) {
        Ok(())
    } else {
        Err(format!("workflow not found: {id}"))
    }
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
fn get_step_manifests() -> Vec<StepManifest> {
    Workflow::available_steps()
}

/// 查询当前系统可用串口列表。
/// 前端可用于串口步骤的下拉选择，同时仍允许用户手动输入。
#[tauri::command]
fn get_serial_ports() -> Result<Vec<String>, String> {
    #[cfg(not(target_os = "android"))]
    {
        available_ports()
            .map(|ports| ports.into_iter().map(|port| port.port_name).collect())
            .map_err(|err| err.to_string())
    }
    #[cfg(target_os = "android")]
    {
        Ok(vec![])
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            start_workflow,
            stop_workflow,
            publish_step_message,
            get_workflow_ids,
            get_step_manifests,
            get_serial_ports
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
