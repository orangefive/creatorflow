use std::path::PathBuf;

use serde::Serialize;

use crate::domain::time::now_iso;
use crate::errors::{AppError, AppResult};
use crate::infrastructure::database::Database;
use crate::repositories::settings as settings_repo;
use crate::services::import_service::{DEFAULT_IMPORT_CONCURRENCY, MAX_IMPORT_CONCURRENCY};

const KEY_WORKSPACE_ROOT: &str = "workspace_root";
const KEY_LAST_OPENED_WORK: &str = "last_opened_work_id";
pub const KEY_IMPORT_CONCURRENCY: &str = "import_concurrency";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub workspace_root: Option<String>,
    pub last_opened_work_id: Option<String>,
}

fn read_string(connection: &rusqlite::Connection, key: &str) -> AppResult<Option<String>> {
    let raw = settings_repo::get(connection, key)?;
    match raw {
        None => Ok(None),
        Some(json) => serde_json::from_str::<String>(&json)
            .map(Some)
            .map_err(|error| {
                tracing::warn!(key, error = %error, "设置项内容不是合法字符串，已忽略");
                AppError::internal("设置数据格式异常")
            }),
    }
}

fn write_string(connection: &rusqlite::Connection, key: &str, value: &str) -> AppResult<()> {
    let json = serde_json::to_string(value)?;
    settings_repo::set(connection, key, &json, &now_iso())
}

pub fn get_settings(database: &Database) -> AppResult<AppSettings> {
    database.with_conn(|connection| {
        Ok(AppSettings {
            workspace_root: read_string(connection, KEY_WORKSPACE_ROOT)?,
            last_opened_work_id: read_string(connection, KEY_LAST_OPENED_WORK)?,
        })
    })
}

/// 设置作品根目录：必须是已存在的绝对目录；保存规范化后的绝对路径。
pub fn set_workspace_root(database: &Database, raw_path: &str) -> AppResult<AppSettings> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("作品根目录不能为空"));
    }
    let candidate = PathBuf::from(trimmed);
    if !candidate.is_absolute() {
        return Err(AppError::validation("请选择绝对路径作为作品根目录"));
    }
    if !candidate.is_dir() {
        return Err(AppError::validation("所选作品根目录不存在或不是文件夹"));
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|error| AppError::io_context("无法访问所选作品根目录", error))?;
    let path_string = crate::infrastructure::filesystem::path_to_string(&canonical);

    database.with_conn(|connection| {
        write_string(connection, KEY_WORKSPACE_ROOT, &path_string)?;
        Ok(())
    })?;
    get_settings(database)
}

pub fn set_last_opened_work(database: &Database, work_id: Option<&str>) -> AppResult<AppSettings> {
    database.with_conn(|connection| match work_id {
        Some(id) => write_string(connection, KEY_LAST_OPENED_WORK, id),
        None => settings_repo::remove(connection, KEY_LAST_OPENED_WORK),
    })?;
    get_settings(database)
}

/// 读取导入并发数；配置缺失或非法时回落到默认值（2）。
pub fn import_concurrency(database: &Database) -> AppResult<usize> {
    let raw =
        database.with_conn(|connection| settings_repo::get(connection, KEY_IMPORT_CONCURRENCY))?;
    let Some(json) = raw else {
        return Ok(DEFAULT_IMPORT_CONCURRENCY);
    };
    match serde_json::from_str::<usize>(&json) {
        Ok(value) => Ok(value.clamp(1, MAX_IMPORT_CONCURRENCY)),
        Err(error) => {
            tracing::warn!(error = %error, "导入并发配置格式错误，使用默认值");
            Ok(DEFAULT_IMPORT_CONCURRENCY)
        }
    }
}

/// 设置导入并发数，返回实际生效值（已裁剪到 1..=4）。
pub fn set_import_concurrency(database: &Database, value: usize) -> AppResult<usize> {
    let clamped = value.clamp(1, MAX_IMPORT_CONCURRENCY);
    let json = serde_json::to_string(&clamped)?;
    database.with_conn(|connection| {
        settings_repo::set(connection, KEY_IMPORT_CONCURRENCY, &json, &now_iso())
    })?;
    Ok(clamped)
}
