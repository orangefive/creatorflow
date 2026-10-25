//! 任务服务：CRUD、状态切换、排序与工作台统计。

use serde::Serialize;

use crate::domain::asset::Asset;
use crate::domain::ids::new_id;
use crate::domain::query::{AssetQuery, AssetSortKey};
use crate::domain::task::{
    NormalizedTaskQuery, Task, TaskInput, TaskQuery, TaskStatus, TaskSummary,
};
use crate::domain::time::now_iso;
use crate::domain::work::Work;
use crate::errors::{AppError, AppResult};
use crate::infrastructure::database::Database;
use crate::repositories::{activity, tasks, works};
use crate::services::asset_service;

/// 工作台聚合数据：作品信息 + 任务统计 + 待办预览 + 最近素材。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkDashboard {
    pub work: Work,
    pub summary: TaskSummary,
    pub upcoming: Vec<Task>,
    pub recent_assets: Vec<Asset>,
}

fn ensure_work(database: &Database, work_id: &str) -> AppResult<Work> {
    database
        .with_conn(|connection| works::find(connection, work_id))?
        .ok_or_else(|| AppError::not_found("作品不存在或已被移除"))
}

fn load_task(database: &Database, task_id: &str) -> AppResult<Task> {
    database
        .with_conn(|connection| tasks::find_by_id(connection, task_id))?
        .ok_or_else(|| AppError::not_found("任务不存在或已被删除"))
}

fn completed_at_for(status: TaskStatus, existing: Option<String>) -> Option<String> {
    match status {
        TaskStatus::Done => Some(existing.unwrap_or_else(now_iso)),
        _ => None,
    }
}

pub fn list_tasks(database: &Database, work_id: &str, query: &TaskQuery) -> AppResult<Vec<Task>> {
    ensure_work(database, work_id)?;
    let normalized = query.normalize()?;
    database.with_conn(|connection| tasks::list(connection, work_id, &normalized))
}

pub fn create_task(database: &Database, work_id: &str, input: &TaskInput) -> AppResult<Task> {
    let work = ensure_work(database, work_id)?;
    if !work.status.accepts_new_tasks() {
        return Err(AppError::validation("当前作品状态不允许新增任务"));
    }
    let normalized = input.normalize()?;
    let now = now_iso();
    let sort_order =
        database.with_conn(|connection| tasks::next_sort_order(connection, work_id))?;
    let task = Task {
        id: new_id(),
        work_id: work_id.to_string(),
        title: normalized.title.clone(),
        description: normalized.description.clone(),
        status: normalized.status,
        priority: normalized.priority,
        due_at: normalized.due_at.clone(),
        sort_order,
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: completed_at_for(normalized.status, None),
        is_overdue: false,
    };

    database.with_conn(|connection| {
        tasks::insert(connection, &task)?;
        activity::record_best_effort(
            connection,
            Some(work_id),
            None,
            "task.created",
            &format!("新建任务「{}」", task.title),
            None,
        );
        Ok(())
    })?;
    load_task(database, &task.id)
}

pub fn update_task(database: &Database, task_id: &str, input: &TaskInput) -> AppResult<Task> {
    let existing = load_task(database, task_id)?;
    let normalized = input.normalize()?;
    let now = now_iso();
    let completed_at = completed_at_for(normalized.status, existing.completed_at.clone());

    database.with_conn(|connection| {
        tasks::update(
            connection,
            task_id,
            &normalized,
            existing.sort_order,
            &now,
            completed_at.as_deref(),
        )
    })?;
    load_task(database, task_id)
}

pub fn set_task_status(database: &Database, task_id: &str, status_text: &str) -> AppResult<Task> {
    let status = TaskStatus::parse(status_text)?;
    let existing = load_task(database, task_id)?;
    let now = now_iso();
    let completed_at = completed_at_for(status, existing.completed_at);

    database.with_conn(|connection| {
        tasks::update_status(connection, task_id, status, &now, completed_at.as_deref())
    })?;
    load_task(database, task_id)
}

pub fn delete_task(database: &Database, task_id: &str) -> AppResult<()> {
    load_task(database, task_id)?;
    database.with_conn(|connection| tasks::delete(connection, task_id))?;
    Ok(())
}

/// 上移 / 下移：按手工顺序交换两个任务的位置，并重排为连续序号。
pub fn move_task(database: &Database, task_id: &str, direction: &str) -> AppResult<Task> {
    let move_up = match direction {
        "up" => true,
        "down" => false,
        other => return Err(AppError::validation(format!("未知的移动方向：{other}"))),
    };
    let task = load_task(database, task_id)?;

    database.with_conn(|connection| {
        let mut ordered = tasks::list(
            connection,
            &task.work_id,
            &crate::domain::task::NormalizedTaskQuery {
                status: None,
                priority: None,
                overdue_only: false,
                sort_key: crate::domain::task::TaskSortKey::Manual,
                descending: false,
            },
        )?;
        let index = ordered
            .iter()
            .position(|item| item.id == task_id)
            .ok_or_else(|| AppError::not_found("任务不存在或已被删除"))?;
        let target = if move_up {
            index.checked_sub(1)
        } else if index + 1 < ordered.len() {
            Some(index + 1)
        } else {
            None
        };
        let Some(target) = target else {
            return Ok(());
        };
        ordered.swap(index, target);
        for (position, item) in ordered.iter().enumerate() {
            tasks::update_sort_order(connection, &item.id, position as i64)?;
        }
        Ok(())
    })?;

    load_task(database, task_id)
}

pub fn task_summary(database: &Database, work_id: &str) -> AppResult<TaskSummary> {
    ensure_work(database, work_id)?;
    database.with_conn(|connection| tasks::summary(connection, work_id))
}

/// 工作台：一次返回作品、统计、待办预览与最近素材。
pub fn work_dashboard(
    database: &Database,
    work_id: &str,
    thumbnail_cache: &std::path::Path,
) -> AppResult<WorkDashboard> {
    let work = ensure_work(database, work_id)?;
    let summary = database.with_conn(|connection| tasks::summary(connection, work_id))?;

    let upcoming = database.with_conn(|connection| {
        tasks::list(
            connection,
            work_id,
            &NormalizedTaskQuery {
                status: None,
                priority: None,
                overdue_only: false,
                sort_key: crate::domain::task::TaskSortKey::Manual,
                descending: false,
            },
        )
    })?;
    let upcoming: Vec<Task> = upcoming
        .into_iter()
        .filter(|task| task.status.is_open())
        .take(5)
        .collect();

    let recent_assets = asset_service::search(
        database,
        &AssetQuery {
            work_id: Some(work_id.to_string()),
            section_key: None,
            media_kind: None,
            status: None,
            tag_id: None,
            keyword: None,
            sort_key: Some(AssetSortKey::CreatedAt),
            sort_direction: None,
            limit: Some(6),
            offset: Some(0),
        },
        thumbnail_cache,
    )?
    .items;

    Ok(WorkDashboard {
        work,
        summary,
        upcoming,
        recent_assets,
    })
}
