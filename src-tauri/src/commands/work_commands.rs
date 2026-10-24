use tauri::State;

use crate::commands::blocking;
use crate::domain::section::Section;
use crate::domain::work::Work;
use crate::errors::AppResult;
use crate::services::work_service::{self, CreateWorkInput};
use crate::AppState;

#[tauri::command]
pub async fn create_work(state: State<'_, AppState>, input: CreateWorkInput) -> AppResult<Work> {
    let state = state.inner().clone();
    blocking(move || work_service::create_work(&state.database, input)).await
}

#[tauri::command]
pub async fn list_works(
    state: State<'_, AppState>,
    include_archived: Option<bool>,
) -> AppResult<Vec<Work>> {
    let state = state.inner().clone();
    let include = include_archived.unwrap_or(false);
    blocking(move || work_service::list_works(&state.database, include)).await
}

#[tauri::command]
pub async fn get_work(state: State<'_, AppState>, work_id: String) -> AppResult<Work> {
    let state = state.inner().clone();
    blocking(move || work_service::get_work(&state.database, &work_id)).await
}

#[tauri::command]
pub async fn rename_work(
    state: State<'_, AppState>,
    work_id: String,
    new_name: String,
) -> AppResult<Work> {
    let state = state.inner().clone();
    blocking(move || work_service::rename_work(&state.database, &work_id, &new_name)).await
}

#[tauri::command]
pub async fn archive_work(state: State<'_, AppState>, work_id: String) -> AppResult<Work> {
    let state = state.inner().clone();
    blocking(move || work_service::archive_work(&state.database, &work_id)).await
}

#[tauri::command]
pub async fn unarchive_work(state: State<'_, AppState>, work_id: String) -> AppResult<Work> {
    let state = state.inner().clone();
    blocking(move || work_service::unarchive_work(&state.database, &work_id)).await
}

#[tauri::command]
pub async fn delete_work(state: State<'_, AppState>, work_id: String) -> AppResult<()> {
    let state = state.inner().clone();
    let cache = state.thumbnail_cache.clone();
    blocking(move || work_service::delete_work(&state.database, &work_id, &cache)).await
}

#[tauri::command]
pub async fn update_work_status(
    state: State<'_, AppState>,
    work_id: String,
    status: String,
) -> AppResult<Work> {
    let state = state.inner().clone();
    blocking(move || work_service::update_status(&state.database, &work_id, &status)).await
}

#[tauri::command]
pub async fn list_sections(state: State<'_, AppState>, work_id: String) -> AppResult<Vec<Section>> {
    let state = state.inner().clone();
    blocking(move || {
        state.database.with_conn(|connection| {
            crate::repositories::works::find(connection, &work_id)?
                .ok_or_else(|| crate::errors::AppError::not_found("作品不存在或已被移除"))?;
            crate::repositories::sections::list_by_work(connection, &work_id)
        })
    })
    .await
}
