use rusqlite::{params, Connection, Row};

use crate::domain::asset::{Asset, AssetStatus, MediaKind};
use crate::domain::section::SectionKey;
use crate::errors::{AppError, AppResult};

/// 插入素材所需的字段（`absolutePath` 由服务层按需解析，不落库）。
pub struct NewAsset {
    pub id: String,
    pub work_id: String,
    pub section_id: String,
    pub display_name: String,
    pub original_name: String,
    pub relative_path: String,
    pub source_path: Option<String>,
    pub extension: Option<String>,
    pub detected_mime_type: Option<String>,
    pub media_kind: MediaKind,
    pub size_bytes: i64,
    pub status: AssetStatus,
    pub created_at: String,
    pub updated_at: String,
}

const SELECT_COLUMNS: &str = "
    a.id, a.work_id, a.section_id, a.display_name, a.original_name, a.relative_path,
    a.source_path, a.extension, a.media_kind, a.size_bytes, a.status, a.is_favorite,
    a.created_at, a.updated_at
";

fn map_asset(row: &Row<'_>) -> rusqlite::Result<Asset> {
    let kind_text: String = row.get("media_kind")?;
    let media_kind = MediaKind::parse(&kind_text).unwrap_or_else(|_| {
        tracing::warn!(kind = %kind_text, "数据库中未知的素材类型，暂按 other 处理");
        MediaKind::Other
    });
    let status_text: String = row.get("status")?;
    let status = AssetStatus::parse(&status_text).unwrap_or_else(|_| {
        tracing::warn!(status = %status_text, "数据库中未知的素材状态，暂按 failed 处理");
        AssetStatus::Failed
    });
    let is_favorite: i64 = row.get("is_favorite")?;
    Ok(Asset {
        id: row.get("id")?,
        work_id: row.get("work_id")?,
        section_id: row.get("section_id")?,
        display_name: row.get("display_name")?,
        original_name: row.get("original_name")?,
        relative_path: row.get("relative_path")?,
        source_path: row.get("source_path")?,
        extension: row.get("extension")?,
        media_kind,
        size_bytes: row.get("size_bytes")?,
        status,
        is_favorite: is_favorite != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        absolute_path: String::new(),
    })
}

pub fn insert(connection: &Connection, asset: &NewAsset) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO assets (
                id, work_id, section_id, display_name, original_name, relative_path,
                source_path, extension, detected_mime_type, media_kind, size_bytes,
                status, is_favorite, created_at, updated_at, deleted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, ?13, ?14, NULL)",
            params![
                asset.id,
                asset.work_id,
                asset.section_id,
                asset.display_name,
                asset.original_name,
                asset.relative_path,
                asset.source_path,
                asset.extension,
                asset.detected_mime_type,
                asset.media_kind.as_str(),
                asset.size_bytes,
                asset.status.as_str(),
                asset.created_at,
                asset.updated_at,
            ],
        )
        .map_err(|error| AppError::db_context("无法登记素材", error))?;
    Ok(())
}

pub fn list_by_work(
    connection: &Connection,
    work_id: &str,
    section_key: Option<SectionKey>,
) -> AppResult<Vec<Asset>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS}
         FROM assets a
         JOIN sections s ON s.id = a.section_id
         WHERE a.work_id = ?1
           AND a.deleted_at IS NULL
           AND (?2 IS NULL OR s.key = ?2)
         ORDER BY a.created_at DESC, a.display_name ASC"
    );
    let key_filter: Option<&str> = section_key.map(SectionKey::as_str);
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询素材", error))?;
    let rows = statement
        .query_map(params![work_id, key_filter], map_asset)
        .map_err(|error| AppError::db_context("无法查询素材", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::db_context("读取素材列表失败", error))
}

pub fn find_by_id(connection: &Connection, asset_id: &str) -> AppResult<Option<Asset>> {
    let sql =
        format!("SELECT {SELECT_COLUMNS} FROM assets a WHERE a.id = ?1 AND a.deleted_at IS NULL");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询素材", error))?;
    let mut rows = statement
        .query_map(params![asset_id], map_asset)
        .map_err(|error| AppError::db_context("无法查询素材", error))?;
    match rows.next() {
        Some(result) => {
            Ok(Some(result.map_err(|error| {
                AppError::db_context("读取素材记录失败", error)
            })?))
        }
        None => Ok(None),
    }
}

pub fn find_by_relative_path(
    connection: &Connection,
    work_id: &str,
    relative_path: &str,
) -> AppResult<Option<Asset>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM assets a
         WHERE a.work_id = ?1 AND a.relative_path = ?2 AND a.deleted_at IS NULL"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询素材", error))?;
    let mut rows = statement
        .query_map(params![work_id, relative_path], map_asset)
        .map_err(|error| AppError::db_context("无法查询素材", error))?;
    match rows.next() {
        Some(result) => {
            Ok(Some(result.map_err(|error| {
                AppError::db_context("读取素材记录失败", error)
            })?))
        }
        None => Ok(None),
    }
}

pub fn update_status(
    connection: &Connection,
    asset_id: &str,
    status: AssetStatus,
    updated_at: &str,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE assets SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.as_str(), updated_at, asset_id],
        )
        .map_err(|error| AppError::db_context("无法更新素材状态", error))
}

pub fn touch(connection: &Connection, asset_id: &str, updated_at: &str) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE assets SET updated_at = ?1 WHERE id = ?2",
            params![updated_at, asset_id],
        )
        .map_err(|error| AppError::db_context("无法更新素材时间", error))
}
