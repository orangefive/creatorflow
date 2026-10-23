use rusqlite::{params, Connection, Row};

use crate::domain::section::{Section, SectionKey};
use crate::errors::{AppError, AppResult};

const SELECT_COLUMNS: &str = "
    s.id, s.work_id, s.key, s.name, s.relative_path, s.sort_order, s.created_at,
    (SELECT COUNT(*) FROM assets a WHERE a.section_id = s.id AND a.deleted_at IS NULL) AS asset_count
";

fn map_section(row: &Row<'_>) -> rusqlite::Result<Section> {
    let key_text: String = row.get("key")?;
    let key = SectionKey::parse(&key_text).unwrap_or_else(|_| {
        tracing::warn!(key = %key_text, "数据库中存在未知的分区标识");
        SectionKey::Documents
    });
    Ok(Section {
        id: row.get("id")?,
        work_id: row.get("work_id")?,
        key,
        name: row.get("name")?,
        relative_path: row.get("relative_path")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        asset_count: row.get("asset_count")?,
    })
}

pub fn insert(connection: &Connection, section: &Section) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO sections (id, work_id, key, name, relative_path, sort_order, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                section.id,
                section.work_id,
                section.key.as_str(),
                section.name,
                section.relative_path,
                section.sort_order,
                section.created_at,
            ],
        )
        .map_err(|error| AppError::db_context("无法登记作品分区", error))?;
    Ok(())
}

pub fn list_by_work(connection: &Connection, work_id: &str) -> AppResult<Vec<Section>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM sections s WHERE s.work_id = ?1 ORDER BY s.sort_order ASC"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询作品分区", error))?;
    let rows = statement
        .query_map(params![work_id], map_section)
        .map_err(|error| AppError::db_context("无法查询作品分区", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::db_context("读取作品分区失败", error))
}

pub fn find_by_id(connection: &Connection, section_id: &str) -> AppResult<Option<Section>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM sections s WHERE s.id = ?1");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询分区", error))?;
    let mut rows = statement
        .query_map(params![section_id], map_section)
        .map_err(|error| AppError::db_context("无法查询分区", error))?;
    match rows.next() {
        Some(result) => {
            Ok(Some(result.map_err(|error| {
                AppError::db_context("读取分区记录失败", error)
            })?))
        }
        None => Ok(None),
    }
}

pub fn find_by_key(
    connection: &Connection,
    work_id: &str,
    key: SectionKey,
) -> AppResult<Option<Section>> {
    let sql =
        format!("SELECT {SELECT_COLUMNS} FROM sections s WHERE s.work_id = ?1 AND s.key = ?2");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询分区", error))?;
    let mut rows = statement
        .query_map(params![work_id, key.as_str()], map_section)
        .map_err(|error| AppError::db_context("无法查询分区", error))?;
    match rows.next() {
        Some(result) => {
            Ok(Some(result.map_err(|error| {
                AppError::db_context("读取分区记录失败", error)
            })?))
        }
        None => Ok(None),
    }
}
