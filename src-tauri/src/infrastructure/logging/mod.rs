use std::fs;
use std::path::Path;

use crate::errors::{AppError, AppResult};

/// 初始化结构化日志：同时输出到控制台与按天轮转的文件。
pub fn init(log_dir: &Path) -> AppResult<()> {
    fs::create_dir_all(log_dir).map_err(|error| AppError::io_context("无法创建日志目录", error))?;

    let file_appender = tracing_appender::rolling::daily(log_dir, "creatorflow.log");
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "warn,creatorflow_lib=info".into());

    // 测试或多窗口场景可能重复初始化，失败时忽略即可（已有全局订阅者）。
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(file_appender)
        .with_ansi(false)
        .try_init();

    Ok(())
}
