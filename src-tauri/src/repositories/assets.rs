use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, Row};

use crate::domain::asset::{Asset, AssetStatus, MediaKind, MetadataStatus, ThumbnailStatus};
use crate::domain::query::{AssetPage, AssetSortKey, NormalizedAssetQuery};
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
    pub sha256: Option<String>,
    pub status: AssetStatus,
    pub created_at: String,
    pub updated_at: String,
}

const SELECT_COLUMNS: &str = "
    a.id, a.work_id, a.section_id, a.display_name, a.original_name, a.relative_path,
    a.source_path, a.extension, a.media_kind, a.size_bytes, a.status, a.is_favorite,
    a.created_at, a.updated_at, a.sha256,
    a.width, a.height, a.duration_ms, a.frame_rate, a.sample_rate, a.channels, a.codec,
    a.metadata_status, a.metadata_error, a.thumbnail_status, a.thumbnail_relative_path,
    a.thumbnail_error
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
    let metadata_text: String = row.get("metadata_status")?;
    let metadata_status = MetadataStatus::parse(&metadata_text).unwrap_or_else(|_| {
        tracing::warn!(status = %metadata_text, "未知的元数据状态，按 pending 处理");
        MetadataStatus::Pending
    });
    let thumbnail_text: String = row.get("thumbnail_status")?;
    let thumbnail_status = ThumbnailStatus::parse(&thumbnail_text).unwrap_or_else(|_| {
        tracing::warn!(status = %thumbnail_text, "未知的缩略图状态，按 pending 处理");
        ThumbnailStatus::Pending
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
        width: row.get("width")?,
        height: row.get("height")?,
        duration_ms: row.get("duration_ms")?,
        frame_rate: row.get("frame_rate")?,
        sample_rate: row.get("sample_rate")?,
        channels: row.get("channels")?,
        codec: row.get("codec")?,
        sha256: row.get("sha256")?,
        metadata_status,
        metadata_error: row.get("metadata_error")?,
        thumbnail_status,
        thumbnail_relative_path: row.get("thumbnail_relative_path")?,
        thumbnail_path: None,
        thumbnail_error: row.get("thumbnail_error")?,
        tags: Vec::new(),
    })
}

pub fn insert(connection: &Connection, asset: &NewAsset) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO assets (
                id, work_id, section_id, display_name, original_name, relative_path,
                source_path, extension, detected_mime_type, media_kind, size_bytes, sha256,
                status, is_favorite, created_at, updated_at, deleted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, ?14, ?15, NULL)",
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
                asset.sha256,
                asset.status.as_str(),
                asset.created_at,
                asset.updated_at,
            ],
        )
        .map_err(|error| AppError::db_context("无法登记素材", error))?;
    Ok(())
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

fn escape_like(keyword: &str) -> String {
    let mut escaped = String::with_capacity(keyword.len() + 2);
    for character in keyword.chars() {
        match character {
            '\\' | '%' | '_' => {
                escaped.push('\\');
                escaped.push(character);
            }
            other => escaped.push(other),
        }
    }
    escaped
}

fn build_filters(query: &NormalizedAssetQuery) -> (String, Vec<Value>) {
    let mut conditions: Vec<&str> = vec!["a.deleted_at IS NULL"];
    let mut values: Vec<Value> = Vec::new();

    if query.work_id.is_some() {
        conditions.push("a.work_id = ?");
    }
    if query.section_key.is_some() {
        conditions.push("s.key = ?");
    }
    if query.media_kind.is_some() {
        conditions.push("a.media_kind = ?");
    }
    if query.status.is_some() {
        conditions.push("a.status = ?");
    }
    if query.tag_id.is_some() {
        conditions.push(
            "EXISTS (SELECT 1 FROM asset_tags at WHERE at.asset_id = a.id AND at.tag_id = ?)",
        );
    }
    if query.keyword.is_some() {
        conditions
            .push("(a.display_name LIKE ? ESCAPE '\\' OR a.original_name LIKE ? ESCAPE '\\')");
    }

    // 参数顺序必须与 conditions 中的占位符顺序一致
    if let Some(work_id) = &query.work_id {
        values.push(Value::Text(work_id.clone()));
    }
    if let Some(section) = query.section_key {
        values.push(Value::Text(section.as_str().to_string()));
    }
    if let Some(kind) = query.media_kind {
        values.push(Value::Text(kind.as_str().to_string()));
    }
    if let Some(status) = query.status {
        values.push(Value::Text(status.as_str().to_string()));
    }
    if let Some(tag_id) = &query.tag_id {
        values.push(Value::Text(tag_id.clone()));
    }
    if let Some(keyword) = &query.keyword {
        let pattern = format!("%{}%", escape_like(keyword));
        values.push(Value::Text(pattern.clone()));
        values.push(Value::Text(pattern));
    }

    (conditions.join(" AND "), values)
}

fn order_clause(query: &NormalizedAssetQuery) -> String {
    let direction = if query.descending { "DESC" } else { "ASC" };
    match query.sort_key {
        AssetSortKey::Name => format!("a.display_name COLLATE NOCASE {direction}, a.id ASC"),
        AssetSortKey::CreatedAt => format!("a.created_at {direction}, a.id ASC"),
        AssetSortKey::Size => format!("a.size_bytes {direction}, a.id ASC"),
        AssetSortKey::Duration => {
            format!("a.duration_ms IS NULL ASC, a.duration_ms {direction}, a.id ASC")
        }
    }
}

pub fn search(connection: &Connection, query: &NormalizedAssetQuery) -> AppResult<AssetPage> {
    let (where_clause, values) = build_filters(query);

    let count_sql = format!(
        "SELECT COUNT(*) FROM assets a JOIN sections s ON s.id = a.section_id WHERE {where_clause}"
    );
    let total: i64 = connection
        .query_row(&count_sql, params_from_iter(values.iter()), |row| {
            row.get(0)
        })
        .map_err(|error| AppError::db_context("无法统计素材数量", error))?;

    let sql = format!(
        "SELECT {SELECT_COLUMNS}
         FROM assets a JOIN sections s ON s.id = a.section_id
         WHERE {where_clause}
         ORDER BY {}
         LIMIT ? OFFSET ?",
        order_clause(query)
    );
    let mut page_values = values;
    page_values.push(Value::Integer(query.limit));
    page_values.push(Value::Integer(query.offset));

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询素材", error))?;
    let rows = statement
        .query_map(params_from_iter(page_values.iter()), map_asset)
        .map_err(|error| AppError::db_context("无法查询素材", error))?;
    let items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::db_context("读取素材列表失败", error))?;

    Ok(AssetPage {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    })
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

pub struct MetadataUpdate {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub frame_rate: Option<f64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub codec: Option<String>,
    pub status: MetadataStatus,
    pub error: Option<String>,
    pub updated_at: String,
}

pub fn update_metadata(
    connection: &Connection,
    asset_id: &str,
    update: &MetadataUpdate,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE assets SET
                width = ?1, height = ?2, duration_ms = ?3, frame_rate = ?4,
                sample_rate = ?5, channels = ?6, codec = ?7,
                metadata_status = ?8, metadata_error = ?9, metadata_updated_at = ?10
             WHERE id = ?11",
            params![
                update.width,
                update.height,
                update.duration_ms,
                update.frame_rate,
                update.sample_rate,
                update.channels,
                update.codec,
                update.status.as_str(),
                update.error,
                update.updated_at,
                asset_id,
            ],
        )
        .map_err(|error| AppError::db_context("无法更新素材元数据", error))
}

pub fn update_thumbnail(
    connection: &Connection,
    asset_id: &str,
    status: ThumbnailStatus,
    relative_path: Option<&str>,
    error: Option<&str>,
    updated_at: &str,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE assets SET thumbnail_status = ?1, thumbnail_relative_path = ?2,
                thumbnail_error = ?3, thumbnail_updated_at = ?4
             WHERE id = ?5",
            params![status.as_str(), relative_path, error, updated_at, asset_id],
        )
        .map_err(|error| AppError::db_context("无法更新缩略图状态", error))
}

/// 启动时为历史素材补提取元数据。
pub fn list_pending_metadata(connection: &Connection, limit: i64) -> AppResult<Vec<String>> {
    let mut statement = connection
        .prepare(
            "SELECT a.id FROM assets a
             WHERE a.deleted_at IS NULL AND a.status = 'ready' AND a.metadata_status = 'pending'
             ORDER BY a.created_at ASC
             LIMIT ?1",
        )
        .map_err(|error| AppError::db_context("无法查询待处理素材", error))?;
    let rows = statement
        .query_map(params![limit], |row| row.get::<_, String>(0))
        .map_err(|error| AppError::db_context("无法查询待处理素材", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::db_context("读取待处理素材失败", error))
}

/// 重新定位素材后的路径更新。
pub struct LocationUpdate<'a> {
    pub relative_path: &'a str,
    pub size_bytes: i64,
    pub sha256: Option<&'a str>,
    pub reset_metadata: bool,
    pub updated_at: &'a str,
}

pub fn update_location(
    connection: &Connection,
    asset_id: &str,
    update: &LocationUpdate<'_>,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE assets SET
                relative_path = ?1, size_bytes = ?2, sha256 = ?3, status = 'ready',
                updated_at = ?4,
                metadata_status = CASE WHEN ?5 THEN 'pending' ELSE metadata_status END,
                metadata_error = CASE WHEN ?5 THEN NULL ELSE metadata_error END,
                thumbnail_status = CASE WHEN ?5 THEN 'pending' ELSE thumbnail_status END,
                thumbnail_error = CASE WHEN ?5 THEN NULL ELSE thumbnail_error END
             WHERE id = ?6",
            params![
                update.relative_path,
                update.size_bytes,
                update.sha256,
                update.updated_at,
                update.reset_metadata,
                asset_id,
            ],
        )
        .map_err(|error| AppError::db_context("无法更新素材位置", error))
}

/// 软删除素材：记录保留，列表按 `deleted_at IS NULL` 过滤。
pub fn soft_delete(connection: &Connection, asset_id: &str, deleted_at: &str) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE assets
                SET deleted_at = ?1, status = 'trashed', updated_at = ?1
              WHERE id = ?2 AND deleted_at IS NULL",
            params![deleted_at, asset_id],
        )
        .map_err(|error| AppError::db_context("无法移除素材记录", error))
}

/// 删除作品时级联软删除其全部素材。
pub fn soft_delete_by_work(
    connection: &Connection,
    work_id: &str,
    deleted_at: &str,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE assets
                SET deleted_at = ?1, status = 'trashed', updated_at = ?1
              WHERE work_id = ?2 AND deleted_at IS NULL",
            params![deleted_at, work_id],
        )
        .map_err(|error| AppError::db_context("无法移除作品下的素材记录", error))
}
