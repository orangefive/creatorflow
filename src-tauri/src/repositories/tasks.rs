use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, Row};

use crate::domain::task::{
    NormalizedTaskInput, NormalizedTaskQuery, Task, TaskPriority, TaskSortKey, TaskStatus,
    TaskSummary,
};
use crate::domain::time::now_iso;
use crate::errors::{AppError, AppResult};

const SELECT_COLUMNS: &str = "
    id, work_id, title, description, status, priority, due_at, sort_order,
    created_at, updated_at, completed_at
";

fn map_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    let status_text: String = row.get("status")?;
    let status = TaskStatus::parse(&status_text).unwrap_or_else(|_| {
        tracing::warn!(status = %status_text, "数据库中未知的任务状态，按 todo 处理");
        TaskStatus::Todo
    });
    let priority_text: String = row.get("priority")?;
    let priority = TaskPriority::parse(&priority_text).unwrap_or_else(|_| {
        tracing::warn!(priority = %priority_text, "数据库中未知的优先级，按 normal 处理");
        TaskPriority::Normal
    });
    let due_at: Option<String> = row.get("due_at")?;

    let is_overdue = matches!(
        (&due_at, status.is_open()),
        (Some(due), true) if is_past(due)
    );

    Ok(Task {
        id: row.get("id")?,
        work_id: row.get("work_id")?,
        title: row.get("title")?,
        description: row.get("description")?,
        status,
        priority,
        due_at,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        completed_at: row.get("completed_at")?,
        is_overdue,
    })
}

fn is_past(timestamp: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| value.with_timezone(&chrono::Utc) < chrono::Utc::now())
        .unwrap_or(false)
}

pub fn insert(connection: &Connection, task: &Task) -> AppResult<()> {
    connection
        .execute(
            "INSERT INTO tasks (
                id, work_id, title, description, status, priority, due_at,
                sort_order, created_at, updated_at, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                task.id,
                task.work_id,
                task.title,
                task.description,
                task.status.as_str(),
                task.priority.as_str(),
                task.due_at,
                task.sort_order,
                task.created_at,
                task.updated_at,
                task.completed_at,
            ],
        )
        .map_err(|error| AppError::db_context("无法创建任务", error))?;
    Ok(())
}

pub fn find_by_id(connection: &Connection, task_id: &str) -> AppResult<Option<Task>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM tasks WHERE id = ?1");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询任务", error))?;
    let mut rows = statement
        .query_map(params![task_id], map_task)
        .map_err(|error| AppError::db_context("无法查询任务", error))?;
    match rows.next() {
        Some(result) => {
            Ok(Some(result.map_err(|error| {
                AppError::db_context("读取任务失败", error)
            })?))
        }
        None => Ok(None),
    }
}

pub fn list(
    connection: &Connection,
    work_id: &str,
    query: &NormalizedTaskQuery,
) -> AppResult<Vec<Task>> {
    let mut conditions: Vec<&str> = vec!["work_id = ?"];
    let mut values: Vec<Value> = vec![Value::Text(work_id.to_string())];

    if let Some(status) = query.status {
        conditions.push("status = ?");
        values.push(Value::Text(status.as_str().to_string()));
    }
    if let Some(priority) = query.priority {
        conditions.push("priority = ?");
        values.push(Value::Text(priority.as_str().to_string()));
    }
    if query.overdue_only {
        conditions.push("status IN ('todo', 'in_progress') AND due_at IS NOT NULL AND due_at < ?");
        values.push(Value::Text(now_iso()));
    }

    let direction = if query.descending { "DESC" } else { "ASC" };
    let order = match query.sort_key {
        TaskSortKey::Manual => "sort_order ASC, created_at ASC, id ASC".to_string(),
        TaskSortKey::CreatedAt => format!("created_at {direction}, id ASC"),
        TaskSortKey::DueAt => format!("due_at IS NULL ASC, due_at {direction}, id ASC"),
        TaskSortKey::Priority => format!(
            "CASE priority WHEN 'urgent' THEN 4 WHEN 'high' THEN 3 WHEN 'normal' THEN 2 ELSE 1 END {direction}, sort_order ASC, id ASC"
        ),
    };

    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM tasks WHERE {} ORDER BY {order}",
        conditions.join(" AND ")
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询任务", error))?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), map_task)
        .map_err(|error| AppError::db_context("无法查询任务", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::db_context("读取任务列表失败", error))
}

pub fn update(
    connection: &Connection,
    task_id: &str,
    input: &NormalizedTaskInput,
    sort_order: i64,
    updated_at: &str,
    completed_at: Option<&str>,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE tasks
                SET title = ?1, description = ?2, status = ?3, priority = ?4,
                    due_at = ?5, sort_order = ?6, updated_at = ?7, completed_at = ?8
              WHERE id = ?9",
            params![
                input.title,
                input.description,
                input.status.as_str(),
                input.priority.as_str(),
                input.due_at,
                sort_order,
                updated_at,
                completed_at,
                task_id,
            ],
        )
        .map_err(|error| AppError::db_context("无法更新任务", error))
}

pub fn update_status(
    connection: &Connection,
    task_id: &str,
    status: TaskStatus,
    updated_at: &str,
    completed_at: Option<&str>,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE tasks SET status = ?1, updated_at = ?2, completed_at = ?3 WHERE id = ?4",
            params![status.as_str(), updated_at, completed_at, task_id],
        )
        .map_err(|error| AppError::db_context("无法更新任务状态", error))
}

pub fn update_sort_order(
    connection: &Connection,
    task_id: &str,
    sort_order: i64,
) -> AppResult<usize> {
    connection
        .execute(
            "UPDATE tasks SET sort_order = ?1 WHERE id = ?2",
            params![sort_order, task_id],
        )
        .map_err(|error| AppError::db_context("无法调整任务顺序", error))
}

pub fn delete(connection: &Connection, task_id: &str) -> AppResult<usize> {
    connection
        .execute("DELETE FROM tasks WHERE id = ?1", params![task_id])
        .map_err(|error| AppError::db_context("无法删除任务", error))
}

pub fn next_sort_order(connection: &Connection, work_id: &str) -> AppResult<i64> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM tasks WHERE work_id = ?1",
            params![work_id],
            |row| row.get(0),
        )
        .map_err(|error| AppError::db_context("无法计算任务顺序", error))
}

pub fn summary(connection: &Connection, work_id: &str) -> AppResult<TaskSummary> {
    connection
        .query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN status = 'todo' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'in_progress' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'done' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'cancelled' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status IN ('todo', 'in_progress')
                                   AND due_at IS NOT NULL AND due_at < ?2 THEN 1 ELSE 0 END), 0)
             FROM tasks WHERE work_id = ?1",
            params![work_id, now_iso()],
            |row| {
                Ok(TaskSummary::from_counts(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(|error| AppError::db_context("无法统计任务", error))
}

/// 相邻任务：用于上移 / 下移交换顺序。
pub fn neighbor(
    connection: &Connection,
    work_id: &str,
    sort_order: i64,
    move_up: bool,
) -> AppResult<Option<Task>> {
    let (comparison, order) = if move_up { ("<", "DESC") } else { (">", "ASC") };
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM tasks
          WHERE work_id = ?1 AND sort_order {comparison} ?2
          ORDER BY sort_order {order} LIMIT 1"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| AppError::db_context("无法查询相邻任务", error))?;
    let mut rows = statement
        .query_map(params![work_id, sort_order], map_task)
        .map_err(|error| AppError::db_context("无法查询相邻任务", error))?;
    match rows.next() {
        Some(result) => {
            Ok(Some(result.map_err(|error| {
                AppError::db_context("读取相邻任务失败", error)
            })?))
        }
        None => Ok(None),
    }
}
