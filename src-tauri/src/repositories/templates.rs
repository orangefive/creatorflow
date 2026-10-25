use std::collections::HashMap;

use rusqlite::{params, Connection, Row};

use crate::domain::task::TaskPriority;
use crate::domain::template::{TemplateTask, WorkTemplate};
use crate::errors::{AppError, AppResult};

const TEMPLATE_COLUMNS: &str = "id, name, description, is_builtin, created_at, updated_at";

fn map_template(row: &Row<'_>, tasks: Vec<TemplateTask>) -> rusqlite::Result<WorkTemplate> {
    let is_builtin: i64 = row.get("is_builtin")?;
    Ok(WorkTemplate {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        is_builtin: is_builtin != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        tasks,
    })
}

fn map_template_task(row: &Row<'_>) -> rusqlite::Result<TemplateTask> {
    let priority_text: String = row.get("priority")?;
    let priority = TaskPriority::parse(&priority_text).unwrap_or_else(|_| {
        tracing::warn!(priority = %priority_text, "模板任务优先级非法，按 normal 处理");
        TaskPriority::Normal
    });
    Ok(TemplateTask {
        id: row.get("id")?,
        title: row.get("title")?,
        description: row.get("description")?,
        priority,
        sort_order: row.get("sort_order")?,
    })
}

fn load_tasks(connection: &Connection, template_id: &str) -> AppResult<Vec<TemplateTask>> {
    let mut statement = connection
        .prepare(
            "SELECT id, title, description, priority, sort_order
               FROM template_tasks WHERE template_id = ?1
              ORDER BY sort_order ASC, created_at ASC",
        )
        .map_err(|error| AppError::db_context("无法查询模板任务", error))?;
    let rows = statement
        .query_map(params![template_id], map_template_task)
        .map_err(|error| AppError::db_context("无法查询模板任务", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::db_context("读取模板任务失败", error))
}

pub fn list(connection: &Connection) -> AppResult<Vec<WorkTemplate>> {
    // 一次性取回全部模板任务，按模板分组，避免逐个模板查询
    let mut grouped: HashMap<String, Vec<TemplateTask>> = HashMap::new();
    {
        let mut statement = connection
            .prepare(
                "SELECT id, template_id, title, description, priority, sort_order
                   FROM template_tasks ORDER BY template_id, sort_order ASC, created_at ASC",
            )
            .map_err(|error| AppError::db_context("无法查询模板任务", error))?;
        let rows = statement
            .query_map([], |row| {
                let template_id: String = row.get("template_id")?;
                Ok((template_id, map_template_task(row)?))
            })
            .map_err(|error| AppError::db_context("无法查询模板任务", error))?;
        for item in rows {
            let (template_id, task) =
                item.map_err(|error| AppError::db_context("读取模板任务失败", error))?;
            grouped.entry(template_id).or_default().push(task);
        }
    }

    let mut statement = connection
        .prepare(&format!(
            "SELECT {TEMPLATE_COLUMNS} FROM work_templates
              ORDER BY is_builtin DESC, name COLLATE NOCASE ASC"
        ))
        .map_err(|error| AppError::db_context("无法查询模板", error))?;
    let rows = statement
        .query_map([], |row| {
            let id: String = row.get("id")?;
            Ok((id, map_template(row, Vec::new())?))
        })
        .map_err(|error| AppError::db_context("无法查询模板", error))?;

    let mut templates = Vec::new();
    for item in rows {
        let (id, mut template) =
            item.map_err(|error| AppError::db_context("读取模板失败", error))?;
        template.tasks = grouped.remove(&id).unwrap_or_default();
        templates.push(template);
    }
    Ok(templates)
}

pub fn find_by_id(connection: &Connection, template_id: &str) -> AppResult<Option<WorkTemplate>> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {TEMPLATE_COLUMNS} FROM work_templates WHERE id = ?1"
        ))
        .map_err(|error| AppError::db_context("无法查询模板", error))?;
    let mut rows = statement
        .query_map(params![template_id], |row| map_template(row, Vec::new()))
        .map_err(|error| AppError::db_context("无法查询模板", error))?;

    let template = match rows.next() {
        Some(result) => result.map_err(|error| AppError::db_context("读取模板失败", error))?,
        None => return Ok(None),
    };
    let tasks = load_tasks(connection, template_id)?;
    Ok(Some(WorkTemplate { tasks, ..template }))
}

pub fn insert(connection: &Connection, template: &WorkTemplate) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO work_templates (id, name, description, is_builtin, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                template.id,
                template.name,
                template.description,
                i64::from(template.is_builtin),
                template.created_at,
                template.updated_at,
            ],
        )
        .map_err(|error| AppError::db_context("无法保存模板", error))?;
    Ok(())
}

pub fn update_meta(
    connection: &Connection,
    template_id: &str,
    name: &str,
    description: Option<&str>,
    updated_at: &str,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE work_templates SET name = ?1, description = ?2, updated_at = ?3 WHERE id = ?4",
            params![name, description, updated_at, template_id],
        )
        .map_err(|error| AppError::db_context("无法更新模板", error))
}

/// 新建模板任务的字段集合，避免参数过长。
pub struct NewTemplateTask<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub priority: TaskPriority,
    pub sort_order: i64,
    pub created_at: &'a str,
}

pub fn insert_task(
    connection: &Connection,
    id: &str,
    template_id: &str,
    task: &NewTemplateTask<'_>,
) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO template_tasks
                (id, template_id, title, description, priority, sort_order, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                template_id,
                task.title,
                task.description,
                task.priority.as_str(),
                task.sort_order,
                task.created_at
            ],
        )
        .map_err(|error| AppError::db_context("无法保存模板任务", error))?;
    Ok(())
}

pub fn clear_tasks(connection: &Connection, template_id: &str) -> AppResult<usize> {
    connection
        .execute(
            "DELETE FROM template_tasks WHERE template_id = ?1",
            params![template_id],
        )
        .map_err(|error| AppError::db_context("无法更新模板任务", error))
}

pub fn delete(connection: &Connection, template_id: &str) -> AppResult<usize> {
    connection
        .execute(
            "DELETE FROM work_templates WHERE id = ?1",
            params![template_id],
        )
        .map_err(|error| AppError::db_context("无法删除模板", error))
}
