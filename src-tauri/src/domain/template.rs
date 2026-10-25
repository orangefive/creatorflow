//! 作品模板：内置模板与用户自定义模板。
//!
//! 模板只提供「新建作品时的初始任务」，应用后任务会复制成作品自己的记录，
//! 之后修改或删除模板不会影响已创建的作品。

use serde::{Deserialize, Serialize};

use crate::domain::task::{TaskPriority, MAX_TASK_DESCRIPTION_LENGTH, MAX_TASK_TITLE_LENGTH};
use crate::errors::{AppError, AppResult};

pub const MAX_TEMPLATE_NAME_LENGTH: usize = 60;
pub const MAX_TEMPLATE_TASKS: usize = 50;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateTask {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: TaskPriority,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkTemplate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_builtin: bool,
    pub created_at: String,
    pub updated_at: String,
    pub tasks: Vec<TemplateTask>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateTaskInput {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInput {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tasks: Vec<TemplateTaskInput>,
}

#[derive(Debug, Clone)]
pub struct NormalizedTemplateTask {
    pub title: String,
    pub description: Option<String>,
    pub priority: TaskPriority,
}

#[derive(Debug, Clone)]
pub struct NormalizedTemplateInput {
    pub name: String,
    pub description: Option<String>,
    pub tasks: Vec<NormalizedTemplateTask>,
}

impl TemplateInput {
    pub fn normalize(&self) -> AppResult<NormalizedTemplateInput> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(AppError::validation("模板名称不能为空"));
        }
        if name.chars().count() > MAX_TEMPLATE_NAME_LENGTH {
            return Err(AppError::validation(format!(
                "模板名称不能超过 {MAX_TEMPLATE_NAME_LENGTH} 个字符"
            )));
        }
        if self.tasks.len() > MAX_TEMPLATE_TASKS {
            return Err(AppError::validation(format!(
                "模板最多包含 {MAX_TEMPLATE_TASKS} 个任务"
            )));
        }

        let description = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if let Some(text) = &description {
            if text.chars().count() > 500 {
                return Err(AppError::validation("模板说明不能超过 500 个字符"));
            }
        }

        let mut tasks = Vec::with_capacity(self.tasks.len());
        for (index, task) in self.tasks.iter().enumerate() {
            let title = task.title.trim();
            if title.is_empty() {
                return Err(AppError::validation(format!(
                    "第 {} 个任务的标题不能为空",
                    index + 1
                )));
            }
            if title.chars().count() > MAX_TASK_TITLE_LENGTH {
                return Err(AppError::validation(format!(
                    "第 {} 个任务的标题不能超过 {MAX_TASK_TITLE_LENGTH} 个字符",
                    index + 1
                )));
            }
            let task_description = task
                .description
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            if let Some(text) = &task_description {
                if text.chars().count() > MAX_TASK_DESCRIPTION_LENGTH {
                    return Err(AppError::validation(format!(
                        "第 {} 个任务的描述过长",
                        index + 1
                    )));
                }
            }
            let priority = match &task.priority {
                Some(value) if !value.is_empty() => TaskPriority::parse(value)?,
                _ => TaskPriority::Normal,
            };
            tasks.push(NormalizedTemplateTask {
                title: title.to_string(),
                description: task_description,
                priority,
            });
        }

        Ok(NormalizedTemplateInput {
            name: name.to_string(),
            description,
            tasks,
        })
    }
}

/// 内置模板的 id 固定以 `builtin-` 开头，不允许删除或改名。
pub fn is_builtin_identifier(id: &str) -> bool {
    id.starts_with("builtin-")
}
