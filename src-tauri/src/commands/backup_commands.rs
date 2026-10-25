use tauri::State;

use crate::commands::blocking;
use crate::errors::AppResult;
use crate::services::backup_service::{
    self, BackupEntry, BackupOverview, BackupVerification, RestoreOutcome,
};
use crate::AppState;

/// 备份概览：目录、当前结构版本与已有备份清单。
#[tauri::command]
pub async fn get_backup_overview(state: State<'_, AppState>) -> AppResult<BackupOverview> {
    let state = state.inner().clone();
    blocking(move || backup_service::overview(&state.database)).await
}

/// 创建数据库备份（不包含作品原文件与缩略图缓存）。
#[tauri::command]
pub async fn create_database_backup(state: State<'_, AppState>) -> AppResult<BackupEntry> {
    let state = state.inner().clone();
    let version = state.app_version.clone();
    blocking(move || backup_service::create_backup(&state.database, &version)).await
}

/// 校验指定备份文件，不修改当前数据库。
#[tauri::command]
pub async fn verify_database_backup(path: String) -> AppResult<BackupVerification> {
    blocking(move || Ok(backup_service::verify_file(std::path::Path::new(&path)))).await
}

/// 从备份恢复：先校验、再快照当前库、最后切换。
#[tauri::command]
pub async fn restore_database_backup(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<RestoreOutcome> {
    let state = state.inner().clone();
    let version = state.app_version.clone();
    blocking(move || backup_service::restore_backup(&state.database, &path, &version)).await
}

/// 设置备份目录（绝对路径，不存在时创建）。
#[tauri::command]
pub async fn set_backup_directory(state: State<'_, AppState>, path: String) -> AppResult<String> {
    let state = state.inner().clone();
    blocking(move || backup_service::set_backup_directory(&state.database, &path)).await
}
