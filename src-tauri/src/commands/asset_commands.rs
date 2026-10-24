use serde::Serialize;
use tauri::State;

use crate::commands::blocking;
use crate::domain::asset::{Asset, Tag};
use crate::domain::query::{AssetPage, AssetQuery};
use crate::domain::relocation::RelocationResult;
use crate::domain::section::SectionKey;
use crate::errors::{AppError, AppResult};
use crate::infrastructure::filesystem;
use crate::services::{asset_service, tag_service};
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
    blocking(move || {
        asset_service::list_assets(&state.database, &work_id, section, &state.thumbnail_cache)
    })
    .await
}

#[tauri::command]
pub async fn search_assets(state: State<'_, AppState>, query: AssetQuery) -> AppResult<AssetPage> {
    let state = state.inner().clone();
    blocking(move || asset_service::search(&state.database, &query, &state.thumbnail_cache)).await
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

/// 人工重新定位缺失素材；内容不一致时需要 `allowMismatch` 二次确认。
#[tauri::command]
pub async fn relocate_asset(
    state: State<'_, AppState>,
    asset_id: String,
    new_path: String,
    allow_mismatch: Option<bool>,
) -> AppResult<RelocationResult> {
    let state = state.inner().clone();
    let allow = allow_mismatch.unwrap_or(false);
    let result = blocking(move || {
        asset_service::relocate_asset(&state.database, &asset_id, &new_path, allow)
    })
    .await?;

    if result.applied {
        if let Some(asset) = &result.asset {
            if asset.metadata_status == crate::domain::asset::MetadataStatus::Pending {
                state.metadata.enqueue(asset.id.clone());
            }
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn list_work_tags(state: State<'_, AppState>, work_id: String) -> AppResult<Vec<Tag>> {
    let state = state.inner().clone();
    blocking(move || tag_service::list_work_tags(&state.database, &work_id)).await
}

#[tauri::command]
pub async fn list_asset_tags(state: State<'_, AppState>, asset_id: String) -> AppResult<Vec<Tag>> {
    let state = state.inner().clone();
    blocking(move || tag_service::list_asset_tags(&state.database, &asset_id)).await
}

#[tauri::command]
pub async fn add_asset_tag(
    state: State<'_, AppState>,
    asset_id: String,
    name: String,
) -> AppResult<Vec<Tag>> {
    let state = state.inner().clone();
    blocking(move || tag_service::add_tag(&state.database, &asset_id, &name)).await
}

#[tauri::command]
pub async fn remove_asset_tag(
    state: State<'_, AppState>,
    asset_id: String,
    tag_id: String,
) -> AppResult<Vec<Tag>> {
    let state = state.inner().clone();
    blocking(move || tag_service::remove_tag(&state.database, &asset_id, &tag_id)).await
}

/// 重新提取某个素材的媒体信息（例如上次提取失败后重试）。
#[tauri::command]
pub async fn retry_asset_metadata(
    state: State<'_, AppState>,
    asset_id: String,
) -> AppResult<Asset> {
    let state = state.inner().clone();
    let metadata = state.metadata.clone();
    let cache = state.thumbnail_cache.clone();
    blocking(move || {
        metadata.process_now(&asset_id)?;
        let mut asset = asset_service::get_asset(&state.database, &asset_id)?;
        asset.thumbnail_path = asset
            .thumbnail_relative_path
            .as_ref()
            .map(|relative| filesystem::path_to_string(&cache.join(relative)));
        Ok(asset)
    })
    .await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaCapabilities {
    pub ffmpeg_available: bool,
}

#[tauri::command]
pub fn get_media_capabilities(state: State<'_, AppState>) -> MediaCapabilities {
    MediaCapabilities {
        ffmpeg_available: state.metadata.ffmpeg_available(),
    }
}

/// 删除素材：文件移入回收站，记录软删除后不再出现在列表里。
#[tauri::command]
pub async fn delete_asset(state: State<'_, AppState>, asset_id: String) -> AppResult<()> {
    let state = state.inner().clone();
    let cache = state.thumbnail_cache.clone();
    blocking(move || asset_service::delete_asset(&state.database, &asset_id, &cache)).await
}
