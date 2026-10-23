use rusqlite::{params, Connection};

use crate::errors::{AppError, AppResult};

pub fn get(connection: &Connection, key: &str) -> AppResult<Option<String>> {
    let mut statement = connection
        .prepare("SELECT value_json FROM app_settings WHERE key = ?1")
        .map_err(|error| AppError::db_context("无法读取设置", error))?;
    let mut rows = statement
        .query_map(params![key], |row| row.get::<_, String>(0))
        .map_err(|error| AppError::db_context("无法读取设置", error))?;
    match rows.next() {
        Some(result) => {
            Ok(Some(result.map_err(|error| {
                AppError::db_context("读取设置失败", error)
            })?))
        }
        None => Ok(None),
    }
}

pub fn set(
    connection: &Connection,
    key: &str,
    value_json: &str,
    updated_at: &str,
) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![key, value_json, updated_at],
        )
        .map_err(|error| AppError::db_context("无法保存设置", error))?;
    Ok(())
}

pub fn remove(connection: &Connection, key: &str) -> AppResult<()> {
    connection
        .execute("DELETE FROM app_settings WHERE key = ?1", params![key])
        .map_err(|error| AppError::db_context("无法清除设置", error))?;
    Ok(())
}
