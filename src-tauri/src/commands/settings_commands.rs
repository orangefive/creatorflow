use serde::Serialize;
use tauri::State;

use crate::commands::blocking;
use crate::errors::AppResult;
use crate::services::settings_service::{self, AppSettings};
use crate::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub tauri_version: String,
    pub database_path: String,
    pub log_directory: String,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    let state = state.inner().clone();
    blocking(move || settings_service::get_settings(&state.database)).await
}

#[tauri::command]
pub async fn set_workspace_root(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<AppSettings> {
    let state = state.inner().clone();
    blocking(move || settings_service::set_workspace_root(&state.database, &path)).await
}

#[tauri::command]
pub async fn set_last_opened_work(
    state: State<'_, AppState>,
    work_id: Option<String>,
) -> AppResult<AppSettings> {
    let state = state.inner().clone();
    blocking(move || settings_service::set_last_opened_work(&state.database, work_id.as_deref()))
        .await
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: state.app_version.clone(),
        tauri_version: state.tauri_version.clone(),
        database_path: state.database_path.to_string_lossy().to_string(),
        log_directory: state.log_directory.to_string_lossy().to_string(),
    }
}
