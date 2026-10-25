use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::domain::ids::new_id;
use crate::domain::section::{Section, SectionKey};
use crate::domain::time::now_iso;
use crate::domain::work::{validate_work_name, Work, WorkManifest, WorkStatus};
use crate::errors::{AppError, AppResult};
use crate::infrastructure::database::Database;
use crate::infrastructure::filesystem;
use crate::infrastructure::recycle_bin;
use crate::repositories::{activity, assets, sections, works};
use crate::services::settings_service;
use crate::services::template_service;

pub const MANIFEST_FILE_NAME: &str = "creatorflow.manifest.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkInput {
    pub name: String,
    pub description: Option<String>,
    pub root_directory: Option<String>,
    pub template_key: Option<String>,
}

fn resolve_root_directory(database: &Database, input: &CreateWorkInput) -> AppResult<PathBuf> {
    let candidate = match input
        .root_directory
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(explicit) => PathBuf::from(explicit),
        None => match settings_service::get_settings(database)?.workspace_root {
            Some(root) => PathBuf::from(root),
            None => {
                return Err(AppError::validation(
                    "请先在设置中选择作品根目录，再创建作品",
                ))
            }
        },
    };

    if !candidate.is_absolute() {
        return Err(AppError::validation("作品根目录必须是绝对路径"));
    }
    if !candidate.is_dir() {
        return Err(AppError::validation("作品根目录不存在或不可用"));
    }
    candidate
        .canonicalize()
        .map_err(|error| AppError::io_context("无法访问作品根目录", error))
}

/// 反向清理本次创建且仍为空的目录，返回是否全部清理成功。
fn cleanup_created_dirs(created: &[PathBuf]) -> bool {
    let mut all_removed = true;
    for path in created.iter().rev() {
        if path.exists() && !filesystem::remove_dir_if_empty(path) {
            all_removed = false;
            tracing::warn!(path = %path.display(), "无法清理创建失败的目录，已保留以便排查");
        }
    }
    all_removed
}

fn section_rows(work_id: &str, created_at: &str) -> Vec<Section> {
    SectionKey::ALL
        .iter()
        .map(|key| Section {
            id: new_id(),
            work_id: work_id.to_string(),
            key: *key,
            name: key.display_name().to_string(),
            relative_path: key.folder_name().to_string(),
            sort_order: key.sort_order(),
            created_at: created_at.to_string(),
            asset_count: 0,
        })
        .collect()
}

/// 创建作品：DB 登记 → 建目录 → 写 manifest → 登记分区 → 置为 idea。
/// 任一步失败都不返回成功，并尽量清理本次新建的空目录。
pub fn create_work(database: &Database, input: CreateWorkInput) -> AppResult<Work> {
    let name = validate_work_name(input.name.trim())?;
    let root_directory = resolve_root_directory(database, &input)?;
    let work_directory = root_directory.join(&name);

    if work_directory.exists() {
        return Err(AppError::conflict(format!(
            "「{}」目录下已存在同名文件夹，请更换作品名称或选择其他根目录",
            root_directory.display()
        )));
    }

    let created_at = now_iso();
    let work_id = new_id();
    let description = input
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    // 模板 id 与清单里的 template_key 共用同一字段；先校验存在性，避免创建到一半才失败
    let template_key = input
        .template_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(template_id) = &template_key {
        template_service::get_template(database, template_id)?;
    }

    let mut work = Work {
        id: work_id.clone(),
        name: name.clone(),
        description,
        status: WorkStatus::Creating,
        root_path: filesystem::path_to_string(&work_directory),
        template_key: template_key.clone(),
        created_at: created_at.clone(),
        updated_at: created_at.clone(),
        archived_at: None,
        archived_from_status: None,
        asset_count: 0,
    };

    database.with_conn(|connection| works::insert(connection, &work))?;

    let mut created_dirs: Vec<PathBuf> = Vec::new();
    let build_result = (|| -> AppResult<Vec<Section>> {
        filesystem::ensure_dir(&work_directory)?;
        created_dirs.push(work_directory.clone());

        for key in SectionKey::ALL {
            let section_dir = work_directory.join(key.folder_name());
            filesystem::ensure_dir(&section_dir)?;
            created_dirs.push(section_dir);
        }

        let manifest = WorkManifest::new(&work.id, &name, template_key.as_deref(), &created_at);
        filesystem::write_json_atomic(&work_directory.join(MANIFEST_FILE_NAME), &manifest)?;

        let rows = section_rows(&work.id, &created_at);
        database.with_conn(|connection| {
            let transaction = connection
                .transaction()
                .map_err(|error| AppError::db_context("无法开启分区登记事务", error))?;
            for section in &rows {
                sections::insert(&transaction, section)?;
            }
            if let Some(template_id) = &template_key {
                template_service::apply_template(&transaction, &work.id, template_id, &created_at)?;
            }
            transaction
                .commit()
                .map_err(|error| AppError::db_context("提交分区登记事务失败", error))?;
            Ok(())
        })?;

        Ok(rows)
    })();

    match build_result {
        Ok(_) => {
            let updated_at = now_iso();
            database.with_conn(|connection| {
                works::update_status(connection, &work.id, WorkStatus::Idea, &updated_at, None)?;
                activity::record_best_effort(
                    connection,
                    Some(&work.id),
                    None,
                    "work.created",
                    &format!("创建作品「{name}」"),
                    None,
                );
                Ok(())
            })?;
            work.status = WorkStatus::Idea;
            work.updated_at = updated_at;
            Ok(work)
        }
        Err(error) => {
            tracing::error!(
                work_id = %work.id,
                message = %error.message,
                detail = error.detail.as_deref().unwrap_or(""),
                "创建作品失败"
            );
            let fully_cleaned = cleanup_created_dirs(&created_dirs);
            if fully_cleaned {
                if let Err(cleanup_error) = database.with_conn(|connection| {
                    connection
                        .execute("DELETE FROM works WHERE id = ?1", [&work.id])
                        .map_err(|error| AppError::db_context("无法回滚作品记录", error))?;
                    Ok(())
                }) {
                    tracing::warn!(message = %cleanup_error.message, "回滚作品记录失败");
                }
            } else if let Err(mark_error) = database.with_conn(|connection| {
                works::update_status(connection, &work.id, WorkStatus::Error, &now_iso(), None)?;
                Ok(())
            }) {
                tracing::warn!(message = %mark_error.message, "无法将作品标记为失败状态");
            }
            Err(error)
        }
    }
}

pub fn get_work(database: &Database, work_id: &str) -> AppResult<Work> {
    database.with_conn(|connection| {
        works::find(connection, work_id)?.ok_or_else(|| AppError::not_found("作品不存在或已被移除"))
    })
}

pub fn list_works(database: &Database, include_archived: bool) -> AppResult<Vec<Work>> {
    database.with_conn(|connection| works::list(connection, include_archived))
}

/// 重命名：先重命名目录，再更新数据库；数据库失败时回滚目录重命名。
pub fn rename_work(database: &Database, work_id: &str, new_name: &str) -> AppResult<Work> {
    let work = get_work(database, work_id)?;
    let name = validate_work_name(new_name.trim())?;
    if name == work.name {
        return Ok(work);
    }

    let old_directory = PathBuf::from(&work.root_path);
    if !old_directory.is_dir() {
        return Err(AppError::not_found(
            "作品目录不存在，无法重命名。请检查磁盘上的作品文件夹是否被移动或删除。",
        ));
    }
    let parent = old_directory
        .parent()
        .ok_or_else(|| AppError::validation("作品目录结构异常，无法重命名"))?;
    let new_directory = parent.join(&name);
    if new_directory.exists() {
        return Err(AppError::conflict(format!(
            "目标位置已存在同名文件夹「{name}」，请换一个名称"
        )));
    }

    fs::rename(&old_directory, &new_directory).map_err(|error| {
        AppError::io_context("无法重命名作品文件夹，可能有文件正被其他程序占用", error)
    })?;

    let updated_at = now_iso();
    let new_root_path = filesystem::path_to_string(&new_directory);
    let database_result = database.with_conn(|connection| {
        let transaction = connection
            .transaction()
            .map_err(|error| AppError::db_context("无法开启重命名事务", error))?;
        works::rename(&transaction, work_id, &name, &new_root_path, &updated_at)?;
        activity::record_best_effort(
            &transaction,
            Some(work_id),
            None,
            "work.renamed",
            &format!("作品重命名为「{name}」"),
            None,
        );
        transaction
            .commit()
            .map_err(|error| AppError::db_context("提交重命名事务失败", error))?;
        Ok(())
    });

    if let Err(error) = database_result {
        match fs::rename(&new_directory, &old_directory) {
            Ok(()) => tracing::warn!("重命名数据库更新失败，已回滚目录名称"),
            Err(rollback_error) => {
                tracing::error!(
                    error = %rollback_error,
                    "数据库更新失败且目录回滚失败，作品目录与数据库可能不一致"
                );
                return Err(AppError::internal(
                    "作品已重命名但其索引更新失败，且目录回滚失败。请重启应用后在作品列表中检查。",
                )
                .with_detail(rollback_error.to_string()));
            }
        }
        return Err(error);
    }

    // manifest 属于辅助识别文件，失败只记录日志，不回滚已成功的重命名。
    let manifest = WorkManifest::new(
        &work.id,
        &name,
        work.template_key.as_deref(),
        &work.created_at,
    );
    if let Err(error) =
        filesystem::write_json_atomic(&new_directory.join(MANIFEST_FILE_NAME), &manifest)
    {
        tracing::warn!(
            message = %error.message,
            detail = error.detail.as_deref().unwrap_or(""),
            "更新作品清单文件失败"
        );
    }

    get_work(database, work_id)
}

pub fn update_status(database: &Database, work_id: &str, status_text: &str) -> AppResult<Work> {
    let status = WorkStatus::parse(status_text)?;
    if !status.is_user_selectable() {
        return Err(AppError::validation("该状态由系统流程控制，不能手动设置"));
    }
    let work = get_work(database, work_id)?;
    let updated_at = now_iso();
    database.with_conn(|connection| {
        works::update_status(connection, work_id, status, &updated_at, None)?;
        activity::record_best_effort(
            connection,
            Some(work_id),
            None,
            "work.status_changed",
            &format!("作品「{}」状态更新为 {}", work.name, status.as_str()),
            None,
        );
        Ok(())
    })?;
    get_work(database, work_id)
}

/// 归档：只改状态，不移动、不删除目录。
pub fn archive_work(database: &Database, work_id: &str) -> AppResult<Work> {
    let work = get_work(database, work_id)?;
    if work.status == WorkStatus::Archived {
        return Ok(work);
    }
    let now = now_iso();
    let previous = work.status.as_str();
    database.with_conn(|connection| {
        works::update_status(connection, work_id, WorkStatus::Archived, &now, Some(&now))?;
        // 记住归档前的状态，取消归档时按它恢复
        works::set_archived_from(connection, work_id, Some(previous))?;
        activity::record_best_effort(
            connection,
            Some(work_id),
            None,
            "work.archived",
            &format!("归档作品「{}」", work.name),
            None,
        );
        Ok(())
    })?;
    get_work(database, work_id)
}

/// 取消归档：恢复到归档前的状态；旧数据没有记录时回落到「构思中」。
pub fn unarchive_work(database: &Database, work_id: &str) -> AppResult<Work> {
    let work = get_work(database, work_id)?;
    if work.status != WorkStatus::Archived {
        return Err(AppError::validation("该作品当前不在归档状态"));
    }

    let restored = work
        .archived_from_status
        .as_deref()
        .and_then(|text| WorkStatus::parse(text).ok())
        .filter(|status| status.is_user_selectable())
        .unwrap_or(WorkStatus::Idea);

    let now = now_iso();
    database.with_conn(|connection| {
        works::update_status(connection, work_id, restored, &now, None)?;
        works::set_archived_from(connection, work_id, None)?;
        activity::record_best_effort(
            connection,
            Some(work_id),
            None,
            "work.unarchived",
            &format!("取消归档「{}」，恢复为 {}", work.name, restored.label()),
            None,
        );
        Ok(())
    })?;
    get_work(database, work_id)
}

/// 删除作品：整个作品目录移入系统回收站，记录做软删除。
///
/// 顺序上先动磁盘再改库：回收站失败（文件被占用等）时直接返回错误，
/// 数据库保持原状，不会出现「记录没了、文件还在」。
pub fn delete_work(
    database: &Database,
    work_id: &str,
    thumbnail_cache: &std::path::Path,
) -> AppResult<()> {
    let work = get_work(database, work_id)?;
    let root = PathBuf::from(&work.root_path);
    recycle_bin::move_to_recycle_bin(&root)?;

    let now = now_iso();
    let removed_assets = database.with_conn(|connection| {
        works::soft_delete(connection, work_id, &now)?;
        let removed = assets::soft_delete_by_work(connection, work_id, &now)?;
        activity::record_best_effort(
            connection,
            Some(work_id),
            None,
            "work.deleted",
            &format!(
                "删除作品「{}」，目录已移入回收站（含 {} 个素材）",
                work.name, removed
            ),
            None,
        );
        Ok(removed)
    })?;

    // 缩略图缓存可以直接丢弃，必要时会重新生成
    let cache_directory = thumbnail_cache.join(work_id);
    if cache_directory.exists() {
        if let Err(error) = fs::remove_dir_all(&cache_directory) {
            tracing::warn!(path = %cache_directory.display(), error = %error, "清理缩略图缓存失败");
        }
    }

    tracing::info!(work_id, removed_assets, path = %work.root_path, "作品已删除并移入回收站");
    Ok(())
}
