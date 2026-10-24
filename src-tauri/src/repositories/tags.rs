use std::collections::HashMap;

use rusqlite::{params, params_from_iter, Connection};

use crate::domain::asset::Tag;
use crate::errors::{AppError, AppResult};

pub const MAX_TAG_NAME_LENGTH: usize = 24;

fn normalize(name: &str) -> AppResult<(String, String)> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("标签名称不能为空"));
    }
    if trimmed.chars().count() > MAX_TAG_NAME_LENGTH {
        return Err(AppError::validation(format!(
            "标签名称不能超过 {MAX_TAG_NAME_LENGTH} 个字符"
        )));
    }
    if trimmed.chars().any(|character| character.is_control()) {
        return Err(AppError::validation("标签名称不能包含控制字符"));
    }
    Ok((trimmed.to_string(), trimmed.to_lowercase()))
}

/// 复用同名标签：已存在则直接返回，不存在时创建。
pub fn ensure(connection: &Connection, name: &str, created_at: &str) -> AppResult<Tag> {
    let (display, normalized) = normalize(name)?;

    connection
        .execute(
            "INSERT INTO tags (id, name, normalized_name, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (normalized_name) DO NOTHING",
            params![
                crate::domain::ids::new_id(),
                display,
                normalized,
                created_at
            ],
        )
        .map_err(|error| AppError::db_context("无法创建标签", error))?;

    let mut statement = connection
        .prepare("SELECT id, name FROM tags WHERE normalized_name = ?1")
        .map_err(|error| AppError::db_context("无法查询标签", error))?;
    let mut rows = statement
        .query_map(params![normalized], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                asset_count: 0,
            })
        })
        .map_err(|error| AppError::db_context("无法查询标签", error))?;
    match rows.next() {
        Some(result) => Ok(result.map_err(|error| AppError::db_context("读取标签失败", error))?),
        None => Err(AppError::internal("标签创建后无法读取")),
    }
}

pub fn find_by_id(connection: &Connection, tag_id: &str) -> AppResult<Option<Tag>> {
    let mut statement = connection
        .prepare("SELECT id, name FROM tags WHERE id = ?1")
        .map_err(|error| AppError::db_context("无法查询标签", error))?;
    let mut rows = statement
        .query_map(params![tag_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                asset_count: 0,
            })
        })
        .map_err(|error| AppError::db_context("无法查询标签", error))?;
    match rows.next() {
        Some(result) => {
            Ok(Some(result.map_err(|error| {
                AppError::db_context("读取标签失败", error)
            })?))
        }
        None => Ok(None),
    }
}

pub fn attach(
    connection: &Connection,
    asset_id: &str,
    tag_id: &str,
    created_at: &str,
) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO asset_tags (asset_id, tag_id, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (asset_id, tag_id) DO NOTHING",
            params![asset_id, tag_id, created_at],
        )
        .map_err(|error| AppError::db_context("无法为素材添加标签", error))?;
    Ok(())
}

pub fn detach(connection: &Connection, asset_id: &str, tag_id: &str) -> AppResult<usize> {
    connection
        .execute(
            "DELETE FROM asset_tags WHERE asset_id = ?1 AND tag_id = ?2",
            params![asset_id, tag_id],
        )
        .map_err(|error| AppError::db_context("无法移除素材标签", error))
}

pub fn list_for_asset(connection: &Connection, asset_id: &str) -> AppResult<Vec<Tag>> {
    let mut map = list_for_assets(connection, &[asset_id.to_string()])?;
    Ok(map.remove(asset_id).unwrap_or_default())
}

/// 批量查询标签，避免列表页 N+1 查询。
pub fn list_for_assets(
    connection: &Connection,
    asset_ids: &[String],
) -> AppResult<HashMap<String, Vec<Tag>>> {
    let mut result: HashMap<String, Vec<Tag>> = HashMap::new();
    if asset_ids.is_empty() {
        return Ok(result);
    }

    let placeholders = std::iter::repeat_n("?", asset_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT at.asset_id, t.id, t.name
         FROM asset_tags at JOIN tags t ON t.id = at.tag_id
         WHERE at.asset_id IN ({placeholders})
         ORDER BY t.normalized_name ASC"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询素材标签", error))?;
    let rows = statement
        .query_map(params_from_iter(asset_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                Tag {
                    id: row.get(1)?,
                    name: row.get(2)?,
                    asset_count: 0,
                },
            ))
        })
        .map_err(|error| AppError::db_context("无法查询素材标签", error))?;

    for row in rows {
        let (asset_id, tag) =
            row.map_err(|error| AppError::db_context("读取素材标签失败", error))?;
        result.entry(asset_id).or_default().push(tag);
    }
    Ok(result)
}

/// 当前作品内使用过的标签及素材数量。
pub fn list_for_work(connection: &Connection, work_id: &str) -> AppResult<Vec<Tag>> {
    let mut statement = connection
        .prepare(
            "SELECT t.id, t.name, COUNT(at.asset_id) AS asset_count
             FROM tags t
             JOIN asset_tags at ON at.tag_id = t.id
             JOIN assets a ON a.id = at.asset_id AND a.deleted_at IS NULL
             WHERE a.work_id = ?1
             GROUP BY t.id, t.name
             ORDER BY t.normalized_name ASC",
        )
        .map_err(|error| AppError::db_context("无法查询作品标签", error))?;
    let rows = statement
        .query_map(params![work_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                asset_count: row.get(2)?,
            })
        })
        .map_err(|error| AppError::db_context("无法查询作品标签", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::db_context("读取作品标签失败", error))
}

/// 移除素材上的全部标签关联。
pub fn detach_all(connection: &Connection, asset_id: &str) -> AppResult<usize> {
    connection
        .execute(
            "DELETE FROM asset_tags WHERE asset_id = ?1",
            params![asset_id],
        )
        .map_err(|error| AppError::db_context("无法清理素材标签", error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_names() {
        let (display, normalized) = normalize("  Highlight  ").unwrap();
        assert_eq!(display, "Highlight");
        assert_eq!(normalized, "highlight");
    }

    #[test]
    fn rejects_invalid_names() {
        assert!(normalize("   ").is_err());
        assert!(normalize(&"标".repeat(MAX_TAG_NAME_LENGTH + 1)).is_err());
    }
}
