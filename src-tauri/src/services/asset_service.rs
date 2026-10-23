use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::domain::asset::{mime_for_extension, Asset, AssetStatus, MediaKind};
use crate::domain::section::SectionKey;
use crate::domain::time::now_iso;
use crate::domain::work::Work;
use crate::errors::{AppError, AppResult};
use crate::infrastructure::database::Database;
use crate::infrastructure::filesystem;
use crate::repositories::{assets, works};

/// 预览上限 24MB，避免把超大图片读入内存导致界面卡顿。
pub const MAX_PREVIEW_BYTES: u64 = 24 * 1024 * 1024;

/// 列出素材并对磁盘做一次存在性校准：缺失标为 `missing`，恢复则回到 `ready`。
pub fn list_assets(
    database: &Database,
    work_id: &str,
    section_key: Option<SectionKey>,
) -> AppResult<Vec<Asset>> {
    let work = database
        .with_conn(|connection| works::find(connection, work_id))?
        .ok_or_else(|| AppError::not_found("作品不存在或已被移除"))?;
    let root = PathBuf::from(&work.root_path);
    let mut items =
        database.with_conn(|connection| assets::list_by_work(connection, work_id, section_key))?;

    for asset in items.iter_mut() {
        let (absolute_path, exists) = match filesystem::resolve_within(&root, &asset.relative_path)
        {
            Ok(path) => {
                let exists = path.is_file();
                (filesystem::path_to_string(&path), exists)
            }
            Err(error) => {
                tracing::warn!(
                    asset_id = %asset.id,
                    message = %error.message,
                    "素材路径无法解析，按缺失处理"
                );
                (filesystem::path_to_string(&root), false)
            }
        };
        asset.absolute_path = absolute_path;

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

    Ok(items)
}

pub fn get_asset(database: &Database, asset_id: &str) -> AppResult<Asset> {
    database.with_conn(|connection| {
        assets::find_by_id(connection, asset_id)?
            .ok_or_else(|| AppError::not_found("素材不存在或已被移除"))
    })
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
