use serde::{Deserialize, Serialize};

use crate::domain::section::SectionKey;
use crate::errors::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Queued,
    Copying,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

impl ImportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Copying => "copying",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "queued" => Ok(Self::Queued),
            "copying" => Ok(Self::Copying),
            "verifying" => Ok(Self::Verifying),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(AppError::validation(format!("未知的导入状态：{other}"))),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn is_running(self) -> bool {
        matches!(self, Self::Queued | Self::Copying | Self::Verifying)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJob {
    pub id: String,
    pub work_id: String,
    pub section_id: String,
    pub source_path: String,
    /// 由 `source_path` 推导，便于界面直接展示。
    pub file_name: String,
    pub target_relative_path: Option<String>,
    pub temporary_relative_path: Option<String>,
    pub source_size_bytes: Option<i64>,
    pub bytes_copied: i64,
    pub status: ImportStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub asset_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub source_path: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub suggested_section_key: Option<SectionKey>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupRecoveryReport {
    pub examined_jobs: i64,
    pub completed_jobs: i64,
    pub failed_jobs: i64,
    pub cleaned_temporary_files: i64,
    pub messages: Vec<String>,
}

/// 事件负载：与前端 `ImportProgressEvent` 对齐。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgressEvent {
    pub job_id: String,
    pub work_id: String,
    pub status: ImportStatus,
    pub bytes_copied: i64,
    pub total_bytes: Option<i64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub target_relative_path: Option<String>,
}

impl From<&ImportJob> for ImportProgressEvent {
    fn from(job: &ImportJob) -> Self {
        Self {
            job_id: job.id.clone(),
            work_id: job.work_id.clone(),
            status: job.status,
            bytes_copied: job.bytes_copied,
            total_bytes: job.source_size_bytes,
            error_code: job.error_code.clone(),
            error_message: job.error_message.clone(),
            target_relative_path: job.target_relative_path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_terminal_and_running_flags() {
        assert!(ImportStatus::Completed.is_terminal());
        assert!(ImportStatus::Failed.is_terminal());
        assert!(ImportStatus::Cancelled.is_terminal());
        assert!(!ImportStatus::Copying.is_terminal());

        assert!(ImportStatus::Queued.is_running());
        assert!(ImportStatus::Verifying.is_running());
        assert!(!ImportStatus::Completed.is_running());
    }

    #[test]
    fn status_roundtrip() {
        for status in [
            ImportStatus::Queued,
            ImportStatus::Copying,
            ImportStatus::Verifying,
            ImportStatus::Completed,
            ImportStatus::Failed,
            ImportStatus::Cancelled,
        ] {
            assert_eq!(ImportStatus::parse(status.as_str()).unwrap(), status);
        }
        assert!(ImportStatus::parse("done").is_err());
    }
}
