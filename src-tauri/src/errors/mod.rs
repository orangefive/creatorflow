use std::fmt;
use std::io;

use serde::ser::{Serialize, SerializeStruct, Serializer};

/// 稳定错误码：前端可据此做分支处理，界面只展示 `message`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidInput,
    NotFound,
    Conflict,
    PermissionDenied,
    DiskFull,
    IoFailure,
    DatabaseFailure,
    MigrationFailure,
    Cancelled,
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::PermissionDenied => "permission_denied",
            Self::DiskFull => "disk_full",
            Self::IoFailure => "io_failure",
            Self::DatabaseFailure => "database_failure",
            Self::MigrationFailure => "migration_failure",
            Self::Cancelled => "cancelled",
            Self::Internal => "internal",
        }
    }
}

/// 应用统一错误：`message` 面向用户（中文），`detail` 面向日志。
#[derive(Debug, Clone)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn cancelled(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Cancelled, message)
    }

    pub fn io_context(context: &str, error: io::Error) -> Self {
        let code = match error.kind() {
            io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
            io::ErrorKind::NotFound => ErrorCode::NotFound,
            io::ErrorKind::AlreadyExists => ErrorCode::Conflict,
            io::ErrorKind::StorageFull => ErrorCode::DiskFull,
            io::ErrorKind::InvalidInput => ErrorCode::InvalidInput,
            _ => ErrorCode::IoFailure,
        };
        let hint = match error.kind() {
            io::ErrorKind::PermissionDenied => "没有访问该文件或目录的权限",
            io::ErrorKind::NotFound => "文件或目录不存在",
            io::ErrorKind::AlreadyExists => "目标位置已存在同名文件或目录",
            io::ErrorKind::StorageFull => "磁盘空间不足，无法完成写入",
            io::ErrorKind::InvalidInput => "文件路径无效",
            _ => "文件操作失败",
        };
        Self {
            code,
            message: format!("{context}：{hint}"),
            detail: Some(error.to_string()),
        }
    }

    pub fn db_context(context: &str, error: rusqlite::Error) -> Self {
        Self {
            code: ErrorCode::DatabaseFailure,
            message: format!("{context}：数据库操作失败，请查看日志"),
            detail: Some(error.to_string()),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::io_context("文件操作失败", error)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        Self::db_context("数据库操作失败", error)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::internal("数据序列化失败").with_detail(error.to_string())
    }
}

/// 序列化到前端时统一形状：`{ code, message, detail }`，并在此处记录日志，
/// 避免命令层遗漏日志导致错误被静默吞掉。
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        tracing::error!(
            code = self.code.as_str(),
            message = %self.message,
            detail = self.detail.as_deref().unwrap_or(""),
            "命令执行失败"
        );
        let mut state = serializer.serialize_struct("AppError", 3)?;
        state.serialize_field("code", self.code.as_str())?;
        state.serialize_field("message", &self.message)?;
        state.serialize_field("detail", &self.detail)?;
        state.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
