use tauri::State;

use crate::commands::blocking;
use crate::errors::AppResult;
use crate::services::cache_service::{self, CacheCleanupResult, CacheStats};
use crate::AppState;

/// 当前缩略图缓存占用与容量上限。
#[tauri::command]
pub async fn get_cache_stats(state: State<'_, AppState>) -> AppResult<CacheStats> {
    let state = state.inner().clone();
    let cache = state.thumbnail_cache.clone();
    blocking(move || cache_service::stats(&state.database, &cache)).await
}

/// 设置缓存容量上限（字节）；返回值已裁剪到允许范围。
#[tauri::command]
pub async fn set_cache_limit(state: State<'_, AppState>, bytes: u64) -> AppResult<u64> {
    let state = state.inner().clone();
    blocking(move || cache_service::set_limit(&state.database, bytes)).await
}

/// 清理全部可再生成的缩略图缓存。
#[tauri::command]
pub async fn clear_thumbnail_cache(state: State<'_, AppState>) -> AppResult<CacheCleanupResult> {
    let state = state.inner().clone();
    let cache = state.thumbnail_cache.clone();
    blocking(move || cache_service::clear_all(&cache)).await
}
