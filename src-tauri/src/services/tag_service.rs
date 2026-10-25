use crate::domain::asset::Tag;
use crate::domain::batch::BatchOutcome;
use crate::domain::time::now_iso;
use crate::errors::{AppError, AppResult};
use crate::infrastructure::database::Database;
use crate::repositories::{activity, assets, tags, works};

pub fn list_work_tags(database: &Database, work_id: &str) -> AppResult<Vec<Tag>> {
    database.with_conn(|connection| {
        works::find(connection, work_id)?
            .ok_or_else(|| AppError::not_found("作品不存在或已被移除"))?;
        tags::list_for_work(connection, work_id)
    })
}

pub fn list_asset_tags(database: &Database, asset_id: &str) -> AppResult<Vec<Tag>> {
    database.with_conn(|connection| {
        assets::find_by_id(connection, asset_id)?
            .ok_or_else(|| AppError::not_found("素材不存在或已被移除"))?;
        tags::list_for_asset(connection, asset_id)
    })
}

/// 新增标签并关联到素材；同名标签自动复用。
pub fn add_tag(database: &Database, asset_id: &str, name: &str) -> AppResult<Vec<Tag>> {
    let asset = database.with_conn(|connection| {
        assets::find_by_id(connection, asset_id)?
            .ok_or_else(|| AppError::not_found("素材不存在或已被移除"))
    })?;
    let now = now_iso();

    database.with_conn(|connection| {
        let transaction = connection
            .transaction()
            .map_err(|error| AppError::db_context("无法开启标签事务", error))?;
        let tag = tags::ensure(&transaction, name, &now)?;
        tags::attach(&transaction, &asset.id, &tag.id, &now)?;
        activity::record_best_effort(
            &transaction,
            Some(&asset.work_id),
            Some(&asset.id),
            "asset.tag_added",
            &format!("为素材「{}」添加标签「{}」", asset.display_name, tag.name),
            None,
        );
        transaction
            .commit()
            .map_err(|error| AppError::db_context("提交标签事务失败", error))?;
        Ok(())
    })?;

    list_asset_tags(database, asset_id)
}

pub fn remove_tag(database: &Database, asset_id: &str, tag_id: &str) -> AppResult<Vec<Tag>> {
    let asset = database.with_conn(|connection| {
        assets::find_by_id(connection, asset_id)?
            .ok_or_else(|| AppError::not_found("素材不存在或已被移除"))
    })?;
    let tag = database
        .with_conn(|connection| tags::find_by_id(connection, tag_id))?
        .ok_or_else(|| AppError::not_found("标签不存在"))?;

    database.with_conn(|connection| {
        let transaction = connection
            .transaction()
            .map_err(|error| AppError::db_context("无法开启标签事务", error))?;
        tags::detach(&transaction, &asset.id, &tag.id)?;
        activity::record_best_effort(
            &transaction,
            Some(&asset.work_id),
            Some(&asset.id),
            "asset.tag_removed",
            &format!("移除素材「{}」的标签「{}」", asset.display_name, tag.name),
            None,
        );
        transaction
            .commit()
            .map_err(|error| AppError::db_context("提交标签事务失败", error))?;
        Ok(())
    })?;

    list_asset_tags(database, asset_id)
}

/// 跨作品列出全部标签，供全局搜索筛选。
pub fn list_all_tags(database: &Database) -> AppResult<Vec<Tag>> {
    database.with_conn(|connection| tags::list_all(connection))
}

/// 确保标签存在（同名复用），返回标签记录。
fn ensure_tag(database: &Database, name: &str) -> AppResult<Tag> {
    let now = now_iso();
    database.with_conn(|connection| tags::ensure(connection, name, &now))
}

/// 批量为素材添加标签；同名标签复用。
pub fn add_tag_to_assets(
    database: &Database,
    tag_name: &str,
    asset_ids: &[String],
) -> AppResult<BatchOutcome> {
    let tag = ensure_tag(database, tag_name)?;
    let now = now_iso();
    let mut outcome = BatchOutcome::new();

    for asset_id in asset_ids {
        let name = database
            .with_conn(|connection| assets::find_by_id(connection, asset_id))
            .ok()
            .flatten()
            .map(|asset| asset.display_name)
            .unwrap_or_else(|| asset_id.clone());

        match database.with_conn(|connection| tags::attach(connection, asset_id, &tag.id, &now)) {
            Ok(()) => outcome.record_success(),
            Err(error) => outcome.record_failure(asset_id, &name, error.message),
        }
    }
    Ok(outcome)
}

/// 批量移除素材上的指定标签。
pub fn remove_tag_from_assets(
    database: &Database,
    tag_id: &str,
    asset_ids: &[String],
) -> AppResult<BatchOutcome> {
    let mut outcome = BatchOutcome::new();
    for asset_id in asset_ids {
        let name = database
            .with_conn(|connection| assets::find_by_id(connection, asset_id))
            .ok()
            .flatten()
            .map(|asset| asset.display_name)
            .unwrap_or_else(|| asset_id.clone());

        match database.with_conn(|connection| tags::detach(connection, asset_id, tag_id)) {
            Ok(_) => outcome.record_success(),
            Err(error) => outcome.record_failure(asset_id, &name, error.message),
        }
    }
    Ok(outcome)
}
