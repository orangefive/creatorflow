use rusqlite::{params, Connection};

use crate::domain::ids::new_id;
use crate::domain::time::now_iso;
use crate::errors::{AppError, AppResult};

/// 记录一条关键操作。失败不阻断主流程，但必须留下日志。
pub fn record(
    connection: &Connection,
    work_id: Option<&str>,
    asset_id: Option<&str>,
    action: &str,
    summary: &str,
    details: Option<&serde_json::Value>,
) -> AppResult<()> {
    let details_json = match details {
        Some(value) => Some(serde_json::to_string(value)?),
        None => None,
    };
    connection
        .execute(
            "INSERT INTO activity_logs (id, work_id, asset_id, action, summary, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new_id(),
                work_id,
                asset_id,
                action,
                summary,
                details_json,
                now_iso()
            ],
        )
        .map_err(|error| AppError::db_context("无法写入活动记录", error))?;
    Ok(())
}

/// 活动记录属于辅助信息，写入失败只告警，不影响主业务结果。
pub fn record_best_effort(
    connection: &Connection,
    work_id: Option<&str>,
    asset_id: Option<&str>,
    action: &str,
    summary: &str,
    details: Option<&serde_json::Value>,
) {
    if let Err(error) = record(connection, work_id, asset_id, action, summary, details) {
        tracing::warn!(
            action,
            message = %error.message,
            detail = error.detail.as_deref().unwrap_or(""),
            "写入活动记录失败"
        );
    }
}
