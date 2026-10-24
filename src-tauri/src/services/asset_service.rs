use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::domain::asset::{mime_for_extension, Asset, AssetStatus, MediaKind};
use crate::domain::query::{AssetPage, AssetQuery, NormalizedAssetQuery};
use crate::domain::relocation::RelocationResult;
use crate::domain::section::SectionKey;
use crate::domain::time::now_iso;
use crate::domain::work::Work;
use crate::errors::{AppError, AppResult};
use crate::infrastructure::database::Database;
use crate::infrastructure::filesystem;
use crate::infrastructure::recycle_bin;
use crate::repositories::{activity, assets, tags, works};

/// 预览上限 24MB，避免把超大图片读入内存导致界面卡顿。
pub const MAX_PREVIEW_BYTES: u64 = 24 * 1024 * 1024;

fn resolve_assets_path(root: &Path, items: &mut [Asset]) {
    for asset in items.iter_mut() {
        match filesystem::resolve_within(root, &asset.relative_path) {
            Ok(path) => asset.absolute_path = filesystem::path_to_string(&path),
            Err(error) => {
                tracing::warn!(
                    asset_id = %asset.id,
                    message = %error.message,
                    "素材路径无法解析，按缺失处理"
                );
                asset.absolute_path = filesystem::path_to_string(root);
            }
        }
    }
}

/// 对当前页素材做存在性校准：缺失标为 `missing`，恢复则回到 `ready`。
fn reconcile_status(database: &Database, items: &mut [Asset]) {
    for asset in items.iter_mut() {
        let absolute = PathBuf::from(&asset.absolute_path);
        let exists = absolute.is_file();
        let desired = if exists {
            AssetStatus::Ready
        } else {
            AssetStatus::Missing
        };
        let reconcilable = matches!(asset.status, AssetStatus::Ready | AssetStatus::Missing);
        if reconcilable && asset.status != desired {
            if let Err(error) = database.with_conn(|connection| {
                assets::update_status(connection, &asset.id, desired, &now_iso())
            }) {
                tracing::warn!(
                    asset_id = %asset.id,
                    message = %error.message,
                    "同步素材存在状态失败"
                );
            }
            asset.status = desired;
        }
    }
}

fn attach_tags(database: &Database, items: &mut [Asset]) -> AppResult<()> {
    let ids: Vec<String> = items.iter().map(|asset| asset.id.clone()).collect();
    let mut map = database.with_conn(|connection| tags::list_for_assets(connection, &ids))?;
    for asset in items.iter_mut() {
        asset.tags = map.remove(&asset.id).unwrap_or_default();
    }
    Ok(())
}

/// 分页查询素材。查询、过滤与排序全部在 SQLite 中完成。
pub fn search(
    database: &Database,
    query: &AssetQuery,
    thumbnail_cache: &Path,
) -> AppResult<AssetPage> {
    let normalized: NormalizedAssetQuery = query.normalize()?;

    let work = match &normalized.work_id {
        Some(work_id) => Some(
            database
                .with_conn(|connection| works::find(connection, work_id))?
                .ok_or_else(|| AppError::not_found("作品不存在或已被移除"))?,
        ),
        None => None,
    };

    let mut page = database.with_conn(|connection| assets::search(connection, &normalized))?;
    attach_tags(database, &mut page.items)?;

    if let Some(work) = &work {
        let root = PathBuf::from(&work.root_path);
        resolve_assets_path(&root, &mut page.items);
        reconcile_status(database, &mut page.items);
    }
    for asset in page.items.iter_mut() {
        asset.thumbnail_path = asset
            .thumbnail_relative_path
            .as_ref()
            .map(|relative| filesystem::path_to_string(&thumbnail_cache.join(relative)));
    }

    Ok(page)
}

/// 兼容第一阶段的列表接口：默认按导入时间倒序取第一页。
pub fn list_assets(
    database: &Database,
    work_id: &str,
    section_key: Option<SectionKey>,
    thumbnail_cache: &Path,
) -> AppResult<Vec<Asset>> {
    let query = AssetQuery {
        work_id: Some(work_id.to_string()),
        section_key,
        media_kind: None,
        status: None,
        tag_id: None,
        keyword: None,
        sort_key: None,
        sort_direction: None,
        limit: Some(200),
        offset: Some(0),
    };
    Ok(search(database, &query, thumbnail_cache)?.items)
}

pub fn get_asset(database: &Database, asset_id: &str) -> AppResult<Asset> {
    let mut asset = database.with_conn(|connection| {
        assets::find_by_id(connection, asset_id)?
            .ok_or_else(|| AppError::not_found("素材不存在或已被移除"))
    })?;
    asset.tags = database.with_conn(|connection| tags::list_for_asset(connection, asset_id))?;
    Ok(asset)
}

/// 读取素材与所属作品，并解析出绝对路径。
pub fn get_asset_with_path(
    database: &Database,
    asset_id: &str,
) -> AppResult<(Asset, Work, PathBuf)> {
    let asset = get_asset(database, asset_id)?;
    let work = database
        .with_conn(|connection| works::find(connection, &asset.work_id))?
        .ok_or_else(|| AppError::not_found("素材所属作品不存在"))?;
    let path = filesystem::resolve_within(Path::new(&work.root_path), &asset.relative_path)?;
    Ok((asset, work, path))
}

/// 生成 data URL 形式的图片预览，避免向渲染进程开放任意文件系统访问。
pub fn preview_data_url(database: &Database, asset_id: &str) -> AppResult<String> {
    let (asset, _work, path) = get_asset_with_path(database, asset_id)?;
    if asset.media_kind != MediaKind::Image {
        return Err(AppError::validation("只有图片素材支持预览"));
    }
    if asset.status != AssetStatus::Ready || !path.is_file() {
        return Err(AppError::not_found("图片文件缺失，无法预览"));
    }
    let size = filesystem::file_size(&path)?;
    if size > MAX_PREVIEW_BYTES {
        return Err(AppError::validation(format!(
            "图片体积过大（{}），请使用默认程序打开",
            human_size(size)
        )));
    }

    let bytes = fs::read(&path).map_err(|error| AppError::io_context("无法读取图片文件", error))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let mime = mime_for_extension(extension);
    let encoded = STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

/// 人工重新定位素材：校验文件、比较大小与哈希，必要时要求二次确认。
///
/// 只允许指向当前作品目录内的文件，避免把素材索引指向作品外部的随机文件。
pub fn relocate_asset(
    database: &Database,
    asset_id: &str,
    new_path: &str,
    allow_mismatch: bool,
) -> AppResult<RelocationResult> {
    let asset = get_asset(database, asset_id)?;
    let work = database
        .with_conn(|connection| works::find(connection, &asset.work_id))?
        .ok_or_else(|| AppError::not_found("素材所属作品不存在"))?;

    let trimmed = new_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("请选择新的文件位置"));
    }
    let candidate = PathBuf::from(trimmed);
    if !candidate.is_file() {
        return Err(AppError::validation("所选路径不存在或不是文件"));
    }

    let root = PathBuf::from(&work.root_path)
        .canonicalize()
        .map_err(|error| AppError::io_context("无法访问作品目录", error))?;
    let canonical = candidate
        .canonicalize()
        .map_err(|error| AppError::io_context("无法访问所选文件", error))?;
    if !canonical.starts_with(&root) {
        return Err(AppError::validation(
            "请选择当前作品目录内的文件。作品外部的文件不会被引用。",
        ));
    }

    let relative_path = canonical
        .strip_prefix(&root)
        .map_err(|_| AppError::validation("无法计算作品内相对路径"))?
        .to_string_lossy()
        .replace('\\', "/");
    let normalized_relative = filesystem::normalize_relative(&relative_path)?;

    if let Some(existing) = database.with_conn(|connection| {
        assets::find_by_relative_path(connection, &asset.work_id, &normalized_relative)
    })? {
        if existing.id != asset.id {
            return Err(AppError::conflict(
                "该路径已被同一作品中的另一个素材使用，无法重复关联",
            ));
        }
    }

    let new_size = filesystem::file_size(&canonical)? as i64;
    let size_matches = new_size == asset.size_bytes;

    let hash_matches = if size_matches && asset.sha256.is_some() {
        if new_size as u64 <= filesystem::MAX_HASH_BYTES {
            let computed = filesystem::sha256_file(&canonical)?;
            Some(asset.sha256.as_deref() == Some(computed.as_str()))
        } else {
            None
        }
    } else {
        None
    };

    let content_conflict = !size_matches || hash_matches == Some(false);
    if content_conflict && !allow_mismatch {
        let reason = if !size_matches {
            format!(
                "文件大小与原记录不一致（原 {} 字节，当前 {} 字节）",
                asset.size_bytes, new_size
            )
        } else {
            "文件内容哈希与原记录不一致".to_string()
        };
        return Ok(RelocationResult {
            applied: false,
            requires_confirmation: true,
            size_matches,
            hash_matches,
            relative_path: normalized_relative,
            message: format!("{reason}，确认后仍要关联该文件吗？"),
            asset: None,
        });
    }

    let now = now_iso();
    let recomputed_hash = if hash_matches.is_none() && new_size as u64 <= filesystem::MAX_HASH_BYTES
    {
        Some(filesystem::sha256_file(&canonical)?)
    } else {
        None
    };
    let hash_value = recomputed_hash.as_deref().or(asset.sha256.as_deref());

    database.with_conn(|connection| {
        let transaction = connection
            .transaction()
            .map_err(|error| AppError::db_context("无法开启重新定位事务", error))?;
        assets::update_location(
            &transaction,
            &asset.id,
            &assets::LocationUpdate {
                relative_path: &normalized_relative,
                size_bytes: new_size,
                sha256: hash_value,
                reset_metadata: content_conflict,
                updated_at: &now,
            },
        )?;
        activity::record_best_effort(
            &transaction,
            Some(&asset.work_id),
            Some(&asset.id),
            "asset.relocated",
            &format!(
                "素材「{}」重新定位到 {}",
                asset.display_name, normalized_relative
            ),
            None,
        );
        transaction
            .commit()
            .map_err(|error| AppError::db_context("提交重新定位事务失败", error))?;
        Ok(())
    })?;

    let updated = get_asset(database, asset_id)?;
    Ok(RelocationResult {
        applied: true,
        requires_confirmation: false,
        size_matches,
        hash_matches,
        relative_path: normalized_relative,
        message: if content_conflict {
            "已按用户确认关联该文件，元数据将重新提取".to_string()
        } else {
            "已重新关联文件".to_string()
        },
        asset: Some(updated),
    })
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// 删除单个素材：真实文件移入系统回收站，记录做软删除。
///
/// 与删除作品一致，先动磁盘再改库；回收站失败时记录保持不变。
pub fn delete_asset(database: &Database, asset_id: &str, thumbnail_cache: &Path) -> AppResult<()> {
    let (asset, work) = database.with_conn(|connection| {
        let asset = assets::find_by_id(connection, asset_id)?
            .ok_or_else(|| AppError::not_found("素材不存在或已被移除"))?;
        let work = works::find(connection, &asset.work_id)?
            .ok_or_else(|| AppError::not_found("作品不存在或已被移除"))?;
        Ok((asset, work))
    })?;

    let root = PathBuf::from(&work.root_path);
    // 路径异常时只记录警告：记录仍可移除，但不会去动作品目录以外的文件
    match filesystem::resolve_within(&root, &asset.relative_path) {
        Ok(path) => recycle_bin::move_to_recycle_bin(&path)?,
        Err(error) => {
            tracing::warn!(
                asset_id,
                relative = %asset.relative_path,
                message = %error.message,
                "素材路径异常，跳过回收站操作"
            );
        }
    }

    let now = now_iso();
    database.with_conn(|connection| {
        assets::soft_delete(connection, asset_id, &now)?;
        tags::detach_all(connection, asset_id)?;
        activity::record_best_effort(
            connection,
            Some(&asset.work_id),
            Some(asset_id),
            "asset.deleted",
            &format!("删除素材「{}」，文件已移入回收站", asset.display_name),
            None,
        );
        Ok(())
    })?;

    if let Some(relative) = asset.thumbnail_relative_path.as_ref() {
        let thumbnail = thumbnail_cache.join(relative);
        if thumbnail.is_file() {
            if let Err(error) = fs::remove_file(&thumbnail) {
                tracing::warn!(path = %thumbnail.display(), error = %error, "清理缩略图失败");
            }
        }
    }

    tracing::info!(asset_id, work_id = %asset.work_id, "素材已删除并移入回收站");
    Ok(())
}
