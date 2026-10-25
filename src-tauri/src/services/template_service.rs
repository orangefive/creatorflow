//! 作品模板服务：内置模板只读，自定义模板可复制、编辑、删除与设为默认。

use rusqlite::Connection;

use crate::domain::ids::new_id;
use crate::domain::task::TaskStatus;
use crate::domain::template::{self, TemplateInput, WorkTemplate};
use crate::domain::time::now_iso;
use crate::errors::{AppError, AppResult};
use crate::infrastructure::database::Database;
use crate::repositories::{settings as settings_repo, tasks, templates};

const KEY_DEFAULT_TEMPLATE: &str = "default_template_id";
const FALLBACK_TEMPLATE_ID: &str = "builtin-video";

pub fn list_templates(database: &Database) -> AppResult<Vec<WorkTemplate>> {
    database.with_conn(|connection| templates::list(connection))
}

pub fn get_template(database: &Database, template_id: &str) -> AppResult<WorkTemplate> {
    database
        .with_conn(|connection| templates::find_by_id(connection, template_id))?
        .ok_or_else(|| AppError::not_found("模板不存在或已被删除"))
}

fn write_tasks(
    connection: &Connection,
    template_id: &str,
    input: &template::NormalizedTemplateInput,
    created_at: &str,
) -> AppResult<()> {
    for (index, task) in input.tasks.iter().enumerate() {
        templates::insert_task(
            connection,
            &new_id(),
            template_id,
            &templates::NewTemplateTask {
                title: &task.title,
                description: task.description.as_deref(),
                priority: task.priority,
                sort_order: index as i64,
                created_at,
            },
        )?;
    }
    Ok(())
}

pub fn create_template(database: &Database, input: &TemplateInput) -> AppResult<WorkTemplate> {
    let normalized = input.normalize()?;
    let now = now_iso();
    let id = new_id();

    database.with_conn(|connection| {
        let transaction = connection
            .transaction()
            .map_err(|error| AppError::db_context("无法开启模板事务", error))?;
        templates::insert(
            &transaction,
            &WorkTemplate {
                id: id.clone(),
                name: normalized.name.clone(),
                description: normalized.description.clone(),
                is_builtin: false,
                created_at: now.clone(),
                updated_at: now.clone(),
                tasks: Vec::new(),
            },
        )?;
        write_tasks(&transaction, &id, &normalized, &now)?;
        transaction
            .commit()
            .map_err(|error| AppError::db_context("提交模板事务失败", error))
    })?;

    get_template(database, &id)
}

/// 复制模板（内置模板或自定义模板都可复制为新的自定义模板）。
pub fn copy_template(
    database: &Database,
    source_id: &str,
    name: Option<&str>,
) -> AppResult<WorkTemplate> {
    let source = get_template(database, source_id)?;
    let requested = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{} 副本", source.name));
    let input = TemplateInput {
        name: requested,
        description: source.description.clone(),
        tasks: source
            .tasks
            .iter()
            .map(|task| template::TemplateTaskInput {
                title: task.title.clone(),
                description: task.description.clone(),
                priority: Some(task.priority.as_str().to_string()),
            })
            .collect(),
    };
    create_template(database, &input)
}

pub fn update_template(
    database: &Database,
    template_id: &str,
    input: &TemplateInput,
) -> AppResult<WorkTemplate> {
    let existing = get_template(database, template_id)?;
    if existing.is_builtin {
        return Err(AppError::conflict("内置模板不能修改，请先复制为自定义模板"));
    }
    let normalized = input.normalize()?;
    let now = now_iso();

    database.with_conn(|connection| {
        let transaction = connection
            .transaction()
            .map_err(|error| AppError::db_context("无法开启模板事务", error))?;
        templates::update_meta(
            &transaction,
            template_id,
            &normalized.name,
            normalized.description.as_deref(),
            &now,
        )?;
        templates::clear_tasks(&transaction, template_id)?;
        write_tasks(&transaction, template_id, &normalized, &now)?;
        transaction
            .commit()
            .map_err(|error| AppError::db_context("提交模板事务失败", error))
    })?;

    get_template(database, template_id)
}

pub fn delete_template(database: &Database, template_id: &str) -> AppResult<()> {
    let existing = get_template(database, template_id)?;
    if existing.is_builtin {
        return Err(AppError::conflict("内置模板不能删除"));
    }

    database.with_conn(|connection| {
        templates::delete(connection, template_id)?;
        if default_template_id_from(connection)?.as_deref() == Some(template_id) {
            settings_repo::remove(connection, KEY_DEFAULT_TEMPLATE)?;
        }
        Ok(())
    })?;
    Ok(())
}

fn default_template_id_from(connection: &Connection) -> AppResult<Option<String>> {
    let raw = settings_repo::get(connection, KEY_DEFAULT_TEMPLATE)?;
    Ok(raw.and_then(|json| serde_json::from_str::<String>(&json).ok()))
}

/// 默认模板 id；设置缺失或被删除时回落到内置「通用视频」。
pub fn default_template_id(database: &Database) -> AppResult<String> {
    let stored = database.with_conn(|connection| default_template_id_from(connection))?;
    match stored {
        Some(id) => {
            let exists = database.with_conn(|connection| templates::find_by_id(connection, &id))?;
            Ok(if exists.is_some() {
                id
            } else {
                FALLBACK_TEMPLATE_ID.to_string()
            })
        }
        None => Ok(FALLBACK_TEMPLATE_ID.to_string()),
    }
}

pub fn default_template(database: &Database) -> AppResult<WorkTemplate> {
    let id = default_template_id(database)?;
    get_template(database, &id)
}

pub fn set_default_template(database: &Database, template_id: &str) -> AppResult<String> {
    get_template(database, template_id)?;
    let json = serde_json::to_string(template_id)?;
    database.with_conn(|connection| {
        settings_repo::set(connection, KEY_DEFAULT_TEMPLATE, &json, &now_iso())
    })?;
    Ok(template_id.to_string())
}

/// 在事务内把模板任务写入作品，返回创建的任务数量。
///
/// 只复制数据，不读写磁盘；模板后续修改不会影响已经写入的任务。
pub fn apply_template(
    connection: &Connection,
    work_id: &str,
    template_id: &str,
    created_at: &str,
) -> AppResult<usize> {
    let template = templates::find_by_id(connection, template_id)?
        .ok_or_else(|| AppError::not_found("模板不存在或已被删除"))?;

    for (index, item) in template.tasks.iter().enumerate() {
        tasks::insert(
            connection,
            &crate::domain::task::Task {
                id: new_id(),
                work_id: work_id.to_string(),
                title: item.title.clone(),
                description: item.description.clone(),
                status: TaskStatus::Todo,
                priority: item.priority,
                due_at: None,
                sort_order: index as i64,
                created_at: created_at.to_string(),
                updated_at: created_at.to_string(),
                completed_at: None,
                is_overdue: false,
            },
        )?;
    }
    Ok(template.tasks.len())
}
