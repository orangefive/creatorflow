pub mod asset_commands;
pub mod import_commands;
pub mod settings_commands;
pub mod work_commands;

use crate::errors::{AppError, AppResult};

/// 把可能较慢的文件/数据库操作放到阻塞线程池，避免占用主线程。
pub async fn blocking<T, F>(operation: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(operation).await {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(error = %error, "后台任务线程异常结束");
            Err(AppError::internal("后台任务执行失败，请查看日志").with_detail(error.to_string()))
        }
    }
}
