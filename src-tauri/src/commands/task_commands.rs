use tauri::State;

use crate::commands::blocking;
use crate::domain::task::{Task, TaskInput, TaskQuery, TaskSummary};
use crate::errors::AppResult;
use crate::services::task_service::{self, WorkDashboard};
use crate::AppState;

#[tauri::command]
pub async fn get_work_dashboard(
    state: State<'_, AppState>,
    work_id: String,
) -> AppResult<WorkDashboard> {
    let state = state.inner().clone();
    let cache = state.thumbnail_cache.clone();
    blocking(move || task_service::work_dashboard(&state.database, &work_id, &cache)).await
}

#[tauri::command]
pub async fn list_tasks(
    state: State<'_, AppState>,
    work_id: String,
    query: Option<TaskQuery>,
) -> AppResult<Vec<Task>> {
    let state = state.inner().clone();
    let query = query.unwrap_or_default();
    blocking(move || task_service::list_tasks(&state.database, &work_id, &query)).await
}

#[tauri::command]
pub async fn get_task_summary(
    state: State<'_, AppState>,
    work_id: String,
) -> AppResult<TaskSummary> {
    let state = state.inner().clone();
    blocking(move || task_service::task_summary(&state.database, &work_id)).await
}

#[tauri::command]
pub async fn create_task(
    state: State<'_, AppState>,
    work_id: String,
    input: TaskInput,
) -> AppResult<Task> {
    let state = state.inner().clone();
    blocking(move || task_service::create_task(&state.database, &work_id, &input)).await
}

#[tauri::command]
pub async fn update_task(
    state: State<'_, AppState>,
    task_id: String,
    input: TaskInput,
) -> AppResult<Task> {
    let state = state.inner().clone();
    blocking(move || task_service::update_task(&state.database, &task_id, &input)).await
}

#[tauri::command]
pub async fn set_task_status(
    state: State<'_, AppState>,
    task_id: String,
    status: String,
) -> AppResult<Task> {
    let state = state.inner().clone();
    blocking(move || task_service::set_task_status(&state.database, &task_id, &status)).await
}

#[tauri::command]
pub async fn delete_task(state: State<'_, AppState>, task_id: String) -> AppResult<()> {
    let state = state.inner().clone();
    blocking(move || task_service::delete_task(&state.database, &task_id)).await
}

#[tauri::command]
pub async fn move_task(
    state: State<'_, AppState>,
    task_id: String,
    direction: String,
) -> AppResult<Task> {
    let state = state.inner().clone();
    blocking(move || task_service::move_task(&state.database, &task_id, &direction)).await
}
