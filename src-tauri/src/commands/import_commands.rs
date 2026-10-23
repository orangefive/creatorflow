use tauri::State;

use crate::commands::blocking;
use crate::domain::import::{ImportJob, StartupRecoveryReport};
use crate::errors::AppResult;
use crate::services::import_service::{self, PrepareImportResult};
use crate::AppState;

#[tauri::command]
pub async fn prepare_import(
    state: State<'_, AppState>,
    work_id: String,
    source_paths: Vec<String>,
) -> AppResult<PrepareImportResult> {
    let state = state.inner().clone();
    blocking(move || state.imports.prepare(&work_id, &source_paths)).await
}

#[tauri::command]
pub async fn start_import(
    state: State<'_, AppState>,
    work_id: String,
    section_key: String,
    source_paths: Vec<String>,
) -> AppResult<Vec<ImportJob>> {
    let imports = state.imports.clone();
    blocking(move || imports.start(&work_id, &section_key, &source_paths)).await
}

#[tauri::command]
pub async fn list_import_jobs(
    state: State<'_, AppState>,
    work_id: Option<String>,
) -> AppResult<Vec<ImportJob>> {
    let state = state.inner().clone();
    blocking(move || {
        state
            .database
            .with_conn(|connection| import_service::list_jobs(connection, work_id.as_deref()))
    })
    .await
}

#[tauri::command]
pub async fn cancel_import(state: State<'_, AppState>, job_id: String) -> AppResult<()> {
    let imports = state.imports.clone();
    blocking(move || imports.cancel(&job_id)).await
}

#[tauri::command]
pub async fn retry_import(state: State<'_, AppState>, job_id: String) -> AppResult<ImportJob> {
    let imports = state.imports.clone();
    blocking(move || imports.retry(&job_id)).await
}

#[tauri::command]
pub fn get_startup_recovery_report(
    state: State<'_, AppState>,
) -> AppResult<Option<StartupRecoveryReport>> {
    state
        .startup_report
        .lock()
        .map(|report| report.clone())
        .map_err(|_| crate::errors::AppError::internal("启动检查状态不可用"))
}
