use rusqlite::{params, Connection, Row};

use crate::domain::work::{Work, WorkStatus};
use crate::errors::{AppError, AppResult};

const SELECT_COLUMNS: &str = "
    w.id, w.name, w.description, w.status, w.root_path, w.template_key,
    w.created_at, w.updated_at, w.archived_at, w.archived_from_status,
    (SELECT COUNT(*) FROM assets a WHERE a.work_id = w.id AND a.deleted_at IS NULL) AS asset_count
";

fn map_work(row: &Row<'_>) -> rusqlite::Result<Work> {
    let status_text: String = row.get("status")?;
    let status = WorkStatus::parse(&status_text).unwrap_or_else(|_| {
        tracing::warn!(status = %status_text, "数据库中存在未知的作品状态，暂按 error 处理");
        WorkStatus::Error
    });
    Ok(Work {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        status,
        root_path: row.get("root_path")?,
        template_key: row.get("template_key")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        archived_at: row.get("archived_at")?,
        archived_from_status: row.get("archived_from_status")?,
        asset_count: row.get("asset_count")?,
    })
}

pub fn insert(connection: &Connection, work: &Work) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO works (
                id, name, description, status, root_path, template_key,
                created_at, updated_at, archived_at, archived_from_status, deleted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL)",
            params![
                work.id,
                work.name,
                work.description,
                work.status.as_str(),
                work.root_path,
                work.template_key,
                work.created_at,
                work.updated_at,
                work.archived_at,
            ],
        )
        .map_err(|error| AppError::db_context("无法登记作品", error))?;
    Ok(())
}

pub fn find(connection: &Connection, work_id: &str) -> AppResult<Option<Work>> {
    let sql =
        format!("SELECT {SELECT_COLUMNS} FROM works w WHERE w.id = ?1 AND w.deleted_at IS NULL");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询作品", error))?;
    let mut rows = statement
        .query_map(params![work_id], map_work)
        .map_err(|error| AppError::db_context("无法查询作品", error))?;
    match rows.next() {
        Some(result) => {
            Ok(Some(result.map_err(|error| {
                AppError::db_context("读取作品记录失败", error)
            })?))
        }
        None => Ok(None),
    }
}

pub fn list(connection: &Connection, include_archived: bool) -> AppResult<Vec<Work>> {
    let mut sql = format!("SELECT {SELECT_COLUMNS} FROM works w WHERE w.deleted_at IS NULL");
    if !include_archived {
        sql.push_str(" AND w.status <> 'archived'");
    }
    sql.push_str(" ORDER BY w.updated_at DESC");

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询作品列表", error))?;
    let rows = statement
        .query_map([], map_work)
        .map_err(|error| AppError::db_context("无法查询作品列表", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::db_context("读取作品列表失败", error))
}

pub fn update_status(
    connection: &Connection,
    work_id: &str,
    status: WorkStatus,
    updated_at: &str,
    archived_at: Option<&str>,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE works SET status = ?1, updated_at = ?2, archived_at = ?3 WHERE id = ?4",
            params![status.as_str(), updated_at, archived_at, work_id],
        )
        .map_err(|error| AppError::db_context("无法更新作品状态", error))
}

pub fn rename(
    connection: &Connection,
    work_id: &str,
    new_name: &str,
    new_root_path: &str,
    updated_at: &str,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE works SET name = ?1, root_path = ?2, updated_at = ?3 WHERE id = ?4",
            params![new_name, new_root_path, updated_at, work_id],
        )
        .map_err(|error| AppError::db_context("无法更新作品名称", error))
}

pub fn touch(connection: &Connection, work_id: &str, updated_at: &str) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE works SET updated_at = ?1 WHERE id = ?2",
            params![updated_at, work_id],
        )
        .map_err(|error| AppError::db_context("无法更新作品时间", error))
}

/// 记录归档前的状态；传 `None` 表示清除（取消归档时）。
pub fn set_archived_from(
    connection: &Connection,
    work_id: &str,
    status: Option<&str>,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE works SET archived_from_status = ?1 WHERE id = ?2",
            params![status, work_id],
        )
        .map_err(|error| AppError::db_context("无法更新归档来源状态", error))
}

/// 软删除作品：只写 `deleted_at`，记录保留，便于回溯与恢复。
pub fn soft_delete(connection: &Connection, work_id: &str, deleted_at: &str) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE works SET deleted_at = ?1, updated_at = ?1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![deleted_at, work_id],
        )
        .map_err(|error| AppError::db_context("无法移除作品记录", error))
}
