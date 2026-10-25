//! 作品任务：状态、优先级、逾期与完成率。

use serde::{Deserialize, Serialize};

use crate::domain::query::SortDirection;
use crate::errors::{AppError, AppResult};

pub const MAX_TASK_TITLE_LENGTH: usize = 120;
pub const MAX_TASK_DESCRIPTION_LENGTH: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "todo" => Ok(Self::Todo),
            "in_progress" => Ok(Self::InProgress),
            "done" => Ok(Self::Done),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(AppError::validation(format!("未知的任务状态：{other}"))),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Todo => "待办",
            Self::InProgress => "进行中",
            Self::Done => "已完成",
            Self::Cancelled => "已取消",
        }
    }

    /// 未完成且未取消，才可能逾期。
    pub fn is_open(self) -> bool {
        matches!(self, Self::Todo | Self::InProgress)
    }

    /// 计入完成率分母。
    pub fn counts_towards_progress(self) -> bool {
        matches!(self, Self::Todo | Self::InProgress | Self::Done)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl TaskPriority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "low" => Ok(Self::Low),
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            "urgent" => Ok(Self::Urgent),
            other => Err(AppError::validation(format!("未知的任务优先级：{other}"))),
        }
    }

    /// 排序用权重，数字越大越紧急。
    pub fn rank(self) -> i64 {
        match self {
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Urgent => 4,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub work_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub due_at: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    /// 截止时间已过且未完成、未取消
    pub is_overdue: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub due_at: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// 校验后的任务输入。
#[derive(Debug, Clone)]
pub struct NormalizedTaskInput {
    pub title: String,
    pub description: Option<String>,
    pub priority: TaskPriority,
    pub due_at: Option<String>,
    pub status: TaskStatus,
}

impl TaskInput {
    pub fn normalize(&self) -> AppResult<NormalizedTaskInput> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err(AppError::validation("任务标题不能为空"));
        }
        if title.chars().count() > MAX_TASK_TITLE_LENGTH {
            return Err(AppError::validation(format!(
                "任务标题不能超过 {MAX_TASK_TITLE_LENGTH} 个字符"
            )));
        }

        let description = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if let Some(text) = &description {
            if text.chars().count() > MAX_TASK_DESCRIPTION_LENGTH {
                return Err(AppError::validation(format!(
                    "任务描述不能超过 {MAX_TASK_DESCRIPTION_LENGTH} 个字符"
                )));
            }
        }

        let priority = match &self.priority {
            Some(value) => TaskPriority::parse(value)?,
            None => TaskPriority::Normal,
        };
        let status = match &self.status {
            Some(value) => TaskStatus::parse(value)?,
            None => TaskStatus::Todo,
        };
        let due_at = match &self.due_at {
            Some(value) if !value.trim().is_empty() => {
                let parsed = chrono::DateTime::parse_from_rfc3339(value.trim())
                    .map_err(|_| AppError::validation("截止时间格式不正确"))?;
                Some(
                    parsed
                        .with_timezone(&chrono::Utc)
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                )
            }
            _ => None,
        };

        Ok(NormalizedTaskInput {
            title: title.to_string(),
            description,
            priority,
            due_at,
            status,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskSortKey {
    Manual,
    CreatedAt,
    DueAt,
    Priority,
}

impl TaskSortKey {
    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "manual" => Ok(Self::Manual),
            "created_at" => Ok(Self::CreatedAt),
            "due_at" => Ok(Self::DueAt),
            "priority" => Ok(Self::Priority),
            other => Err(AppError::validation(format!("未知的任务排序方式：{other}"))),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskQuery {
    pub status: Option<String>,
    pub priority: Option<String>,
    #[serde(default)]
    pub overdue_only: bool,
    pub sort_key: Option<String>,
    pub sort_direction: Option<SortDirection>,
}

#[derive(Debug, Clone)]
pub struct NormalizedTaskQuery {
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub overdue_only: bool,
    pub sort_key: TaskSortKey,
    pub descending: bool,
}

impl TaskQuery {
    pub fn normalize(&self) -> AppResult<NormalizedTaskQuery> {
        let status = match &self.status {
            Some(value) if !value.is_empty() => Some(TaskStatus::parse(value)?),
            _ => None,
        };
        let priority = match &self.priority {
            Some(value) if !value.is_empty() => Some(TaskPriority::parse(value)?),
            _ => None,
        };
        let sort_key = match &self.sort_key {
            Some(value) if !value.is_empty() => TaskSortKey::parse(value)?,
            _ => TaskSortKey::Manual,
        };
        let descending = matches!(self.sort_direction, Some(SortDirection::Desc));

        Ok(NormalizedTaskQuery {
            status,
            priority,
            overdue_only: self.overdue_only,
            sort_key,
            descending,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub total: i64,
    pub todo: i64,
    pub in_progress: i64,
    pub done: i64,
    pub cancelled: i64,
    pub overdue: i64,
    /// done / (todo + in_progress + done)；没有可计入的任务时为 None
    pub completion_rate: Option<f64>,
}

impl TaskSummary {
    pub fn from_counts(
        todo: i64,
        in_progress: i64,
        done: i64,
        cancelled: i64,
        overdue: i64,
    ) -> Self {
        let countable = todo + in_progress + done;
        let completion_rate = if countable > 0 {
            Some(done as f64 / countable as f64)
        } else {
            None
        };
        Self {
            total: todo + in_progress + done + cancelled,
            todo,
            in_progress,
            done,
            cancelled,
            overdue,
            completion_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_excludes_cancelled_and_handles_empty() {
        let summary = TaskSummary::from_counts(2, 1, 3, 4, 1);
        assert_eq!(summary.total, 10);
        assert_eq!(summary.completion_rate, Some(0.5));

        let empty = TaskSummary::from_counts(0, 0, 0, 3, 0);
        assert_eq!(empty.total, 3);
        assert_eq!(empty.completion_rate, None, "只有已取消任务时应视为无任务");
    }

    #[test]
    fn rejects_blank_title_and_bad_due_date() {
        let blank = TaskInput {
            title: "   ".into(),
            description: None,
            priority: None,
            due_at: None,
            status: None,
        };
        assert!(blank.normalize().is_err());

        let bad_due = TaskInput {
            title: "写脚本".into(),
            description: None,
            priority: None,
            due_at: Some("2026-13-40".into()),
            status: None,
        };
        assert!(bad_due.normalize().is_err());
    }

    #[test]
    fn normalizes_priority_and_status() {
        let input = TaskInput {
            title: " 粗剪 ".into(),
            description: Some("  ".into()),
            priority: Some("high".into()),
            due_at: Some("2026-11-01T10:00:00+08:00".into()),
            status: Some("in_progress".into()),
        };
        let normalized = input.normalize().unwrap();
        assert_eq!(normalized.title, "粗剪");
        assert_eq!(normalized.description, None);
        assert_eq!(normalized.priority, TaskPriority::High);
        assert_eq!(normalized.status, TaskStatus::InProgress);
        assert_eq!(
            normalized.due_at.as_deref(),
            Some("2026-11-01T02:00:00.000Z")
        );
    }
}
