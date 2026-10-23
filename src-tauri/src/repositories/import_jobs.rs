use std::path::Path;

use rusqlite::{params, Connection, Row};

use crate::domain::import::{ImportJob, ImportStatus};
use crate::errors::{AppError, AppResult};

pub struct NewImportJob {
    pub id: String,
    pub work_id: String,
    pub section_id: String,
    pub source_path: String,
    pub created_at: String,
    pub updated_at: String,
}

const SELECT_COLUMNS: &str = "
    j.id, j.work_id, j.section_id, j.source_path, j.target_relative_path,
    j.temporary_relative_path, j.source_size_bytes, j.bytes_copied, j.status,
    j.error_code, j.error_message, j.asset_id, j.created_at, j.updated_at, j.completed_at
";

fn map_job(row: &Row<'_>) -> rusqlite::Result<ImportJob> {
    let status_text: String = row.get("status")?;
    let status = ImportStatus::parse(&status_text).unwrap_or_else(|_| {
        tracing::warn!(status = %status_text, "数据库中未知的导入状态，暂按 failed 处理");
        ImportStatus::Failed
    });
    let source_path: String = row.get("source_path")?;
    let file_name = Path::new(&source_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| source_path.clone());
    Ok(ImportJob {
        id: row.get("id")?,
        work_id: row.get("work_id")?,
        section_id: row.get("section_id")?,
        source_path,
        file_name,
        target_relative_path: row.get("target_relative_path")?,
        temporary_relative_path: row.get("temporary_relative_path")?,
        source_size_bytes: row.get("source_size_bytes")?,
        bytes_copied: row.get("bytes_copied")?,
        status,
        error_code: row.get("error_code")?,
        error_message: row.get("error_message")?,
        asset_id: row.get("asset_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        completed_at: row.get("completed_at")?,
    })
}

pub fn insert(connection: &Connection, job: &NewImportJob) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO import_jobs (
                id, work_id, section_id, source_path, target_relative_path,
                temporary_relative_path, source_size_bytes, bytes_copied, status,
                error_code, error_message, asset_id, created_at, updated_at, completed_at
             ) VALUES (?1, ?2, ?3, ?4, NULL, NULL, NULL, 0, 'queued', NULL, NULL, NULL, ?5, ?6, NULL)",
            params![
                job.id,
                job.work_id,
                job.section_id,
                job.source_path,
                job.created_at,
                job.updated_at,
            ],
        )
        .map_err(|error| AppError::db_context("无法创建导入任务", error))?;
    Ok(())
}

pub fn find_by_id(connection: &Connection, job_id: &str) -> AppResult<Option<ImportJob>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM import_jobs j WHERE j.id = ?1");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询导入任务", error))?;
    let mut rows = statement
        .query_map(params![job_id], map_job)
        .map_err(|error| AppError::db_context("无法查询导入任务", error))?;
    match rows.next() {
        Some(result) => {
            Ok(Some(result.map_err(|error| {
                AppError::db_context("读取导入任务失败", error)
            })?))
        }
        None => Ok(None),
    }
}

pub fn list(connection: &Connection, work_id: Option<&str>) -> AppResult<Vec<ImportJob>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM import_jobs j
         WHERE (?1 IS NULL OR j.work_id = ?1)
         ORDER BY j.created_at DESC"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询导入任务", error))?;
    let rows = statement
        .query_map(params![work_id], map_job)
        .map_err(|error| AppError::db_context("无法查询导入任务", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::db_context("读取导入任务列表失败", error))
}

/// 启动时用于恢复的非终态任务。
pub fn list_unfinished(connection: &Connection) -> AppResult<Vec<ImportJob>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM import_jobs j
         WHERE j.status IN ('queued', 'copying', 'verifying')
         ORDER BY j.created_at ASC"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询未完成导入任务", error))?;
    let rows = statement
        .query_map([], map_job)
        .map_err(|error| AppError::db_context("无法查询未完成导入任务", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::db_context("读取未完成导入任务失败", error))
}

pub fn update_paths(
    connection: &Connection,
    job_id: &str,
    target_relative_path: &str,
    temporary_relative_path: &str,
    source_size_bytes: i64,
    updated_at: &str,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE import_jobs
             SET target_relative_path = ?1, temporary_relative_path = ?2,
                 source_size_bytes = ?3, updated_at = ?4
             WHERE id = ?5",
            params![
                target_relative_path,
                temporary_relative_path,
                source_size_bytes,
                updated_at,
                job_id
            ],
        )
        .map_err(|error| AppError::db_context("无法更新导入任务路径", error))
}

pub fn update_progress(
    connection: &Connection,
    job_id: &str,
    bytes_copied: i64,
    updated_at: &str,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE import_jobs SET bytes_copied = ?1, updated_at = ?2 WHERE id = ?3",
            params![bytes_copied, updated_at, job_id],
        )
        .map_err(|error| AppError::db_context("无法更新导入进度", error))
}

pub struct JobStatusUpdate<'a> {
    pub status: ImportStatus,
    pub bytes_copied: Option<i64>,
    pub error_code: Option<&'a str>,
    pub error_message: Option<&'a str>,
    pub asset_id: Option<&'a str>,
    pub completed_at: Option<&'a str>,
    pub updated_at: &'a str,
}

pub fn update_status(
    connection: &Connection,
    job_id: &str,
    update: &JobStatusUpdate<'_>,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE import_jobs
             SET status = ?1,
                 bytes_copied = COALESCE(?2, bytes_copied),
                 error_code = ?3,
                 error_message = ?4,
                 asset_id = COALESCE(?5, asset_id),
                 completed_at = ?6,
                 updated_at = ?7
             WHERE id = ?8",
            params![
                update.status.as_str(),
                update.bytes_copied,
                update.error_code,
                update.error_message,
                update.asset_id,
                update.completed_at,
                update.updated_at,
                job_id
            ],
        )
        .map_err(|error| AppError::db_context("无法更新导入任务状态", error))
}

/// 重试前重置任务：清除进度、错误与目标路径，保留来源信息。
pub fn reset_for_retry(
    connection: &Connection,
    job_id: &str,
    updated_at: &str,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE import_jobs
             SET status = 'queued', bytes_copied = 0, error_code = NULL, error_message = NULL,
                 target_relative_path = NULL, temporary_relative_path = NULL,
                 source_size_bytes = NULL, asset_id = NULL, completed_at = NULL, updated_at = ?1
             WHERE id = ?2",
            params![updated_at, job_id],
        )
        .map_err(|error| AppError::db_context("无法重置导入任务", error))
}
