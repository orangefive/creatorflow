use tauri::State;

use crate::commands::blocking;
use crate::domain::template::{TemplateInput, WorkTemplate};
use crate::errors::AppResult;
use crate::services::template_service;
use crate::AppState;

#[tauri::command]
pub async fn list_templates(state: State<'_, AppState>) -> AppResult<Vec<WorkTemplate>> {
    let state = state.inner().clone();
    blocking(move || template_service::list_templates(&state.database)).await
}

#[tauri::command]
pub async fn get_template(
    state: State<'_, AppState>,
    template_id: String,
) -> AppResult<WorkTemplate> {
    let state = state.inner().clone();
    blocking(move || template_service::get_template(&state.database, &template_id)).await
}

#[tauri::command]
pub async fn create_template(
    state: State<'_, AppState>,
    input: TemplateInput,
) -> AppResult<WorkTemplate> {
    let state = state.inner().clone();
    blocking(move || template_service::create_template(&state.database, &input)).await
}

#[tauri::command]
pub async fn copy_template(
    state: State<'_, AppState>,
    template_id: String,
    name: Option<String>,
) -> AppResult<WorkTemplate> {
    let state = state.inner().clone();
    blocking(move || {
        template_service::copy_template(&state.database, &template_id, name.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn update_template(
    state: State<'_, AppState>,
    template_id: String,
    input: TemplateInput,
) -> AppResult<WorkTemplate> {
    let state = state.inner().clone();
    blocking(move || template_service::update_template(&state.database, &template_id, &input)).await
}

#[tauri::command]
pub async fn delete_template(state: State<'_, AppState>, template_id: String) -> AppResult<()> {
    let state = state.inner().clone();
    blocking(move || template_service::delete_template(&state.database, &template_id)).await
}

#[tauri::command]
pub async fn set_default_template(
    state: State<'_, AppState>,
    template_id: String,
) -> AppResult<String> {
    let state = state.inner().clone();
    blocking(move || template_service::set_default_template(&state.database, &template_id)).await
}

#[tauri::command]
pub async fn get_default_template(state: State<'_, AppState>) -> AppResult<WorkTemplate> {
    let state = state.inner().clone();
    blocking(move || template_service::default_template(&state.database)).await
}
