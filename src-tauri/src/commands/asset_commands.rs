use tauri::State;

use crate::commands::blocking;
use crate::domain::asset::Asset;
use crate::domain::section::SectionKey;
use crate::errors::{AppError, AppResult};
use crate::services::asset_service;
use crate::AppState;

#[tauri::command]
pub async fn list_assets(
    state: State<'_, AppState>,
    work_id: String,
    section_key: Option<String>,
) -> AppResult<Vec<Asset>> {
    let state = state.inner().clone();
    let section = match section_key {
        Some(value) if !value.trim().is_empty() => Some(SectionKey::parse(&value)?),
        _ => None,
    };
    blocking(move || asset_service::list_assets(&state.database, &work_id, section)).await
}

#[tauri::command]
pub async fn get_asset_preview(state: State<'_, AppState>, asset_id: String) -> AppResult<String> {
    let state = state.inner().clone();
    blocking(move || asset_service::preview_data_url(&state.database, &asset_id)).await
}

/// 用系统默认程序打开素材文件。
#[tauri::command]
pub async fn open_asset(state: State<'_, AppState>, asset_id: String) -> AppResult<()> {
    let state = state.inner().clone();
    let (asset, _work, path) =
        blocking(move || asset_service::get_asset_with_path(&state.database, &asset_id)).await?;
    if !path.is_file() {
        return Err(AppError::not_found(format!(
            "文件「{}」在磁盘上不存在，可能已被移动或删除",
            asset.display_name
        )));
    }
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|error| {
        AppError::internal("无法调用系统默认程序打开该文件").with_detail(error.to_string())
    })
}

/// 在资源管理器中定位素材文件。
#[tauri::command]
pub async fn reveal_asset(state: State<'_, AppState>, asset_id: String) -> AppResult<()> {
    let state = state.inner().clone();
    let (asset, _work, path) =
        blocking(move || asset_service::get_asset_with_path(&state.database, &asset_id)).await?;
    if !path.exists() {
        return Err(AppError::not_found(format!(
            "文件「{}」在磁盘上不存在，无法定位",
            asset.display_name
        )));
    }
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|error| {
        AppError::internal("无法在资源管理器中显示该文件").with_detail(error.to_string())
    })
}
