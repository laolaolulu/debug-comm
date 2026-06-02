pub mod step;

use step::model::{StepManifest, WorkflowDefinition};
use step::workflow::Workflow;
use tauri::AppHandle;

/// 启动工作流。
#[tauri::command]
fn start_workflow(json: &str, app: AppHandle) -> Result<(), String> {
    let definition =
        serde_json::from_str::<WorkflowDefinition>(json).map_err(|err| err.to_string())?;

    Workflow::remove(&definition.id);

    let workflow = Workflow::new_with_app(definition, app);
    workflow.run()?;

    Ok(())
}

/// 停止工作流。
#[tauri::command]
fn stop_workflow(id: &str) -> Result<(), String> {
    if Workflow::remove(id) {
        Ok(())
    } else {
        Err(format!("workflow not found: {id}"))
    }
}

/// 获取当前执行中的工作流所有 id。
#[tauri::command]
fn get_workflow_ids() -> Vec<String> {
    Workflow::list_ids()
}

/// 获取所有可创建步骤。
#[tauri::command]
fn get_step_manifests() -> Vec<StepManifest> {
    Workflow::available_steps()
}

/// 启动 Tauri 应用并注册后端命令。
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
            get_workflow_ids,
            get_step_manifests
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
