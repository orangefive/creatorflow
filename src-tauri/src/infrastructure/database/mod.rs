pub mod migrations;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::errors::{AppError, AppResult};

/// SQLite 连接封装：集中管理连接、PRAGMA 与迁移。
/// 连接位于 `Mutex` 中，所有使用都是短事务；长时间文件复制不得持有此锁。
pub struct Database {
    path: PathBuf,
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: impl Into<PathBuf>) -> AppResult<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| AppError::io_context("无法创建应用数据目录", error))?;
        }

        let mut connection = Connection::open(&path)
            .map_err(|error| AppError::db_context("无法打开数据库", error))?;

        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA busy_timeout = 5000;",
            )
            .map_err(|error| AppError::db_context("无法初始化数据库连接", error))?;

        migrations::run(&mut connection).map_err(|error| {
            tracing::error!(detail = ?error.detail, "数据库迁移失败");
            AppError::new(
                crate::errors::ErrorCode::MigrationFailure,
                "数据库结构升级失败，应用已停止写入以避免数据损坏。请备份数据目录后重试。",
            )
            .with_detail(error.detail.unwrap_or_else(|| error.message.clone()))
        })?;

        Ok(Self {
            path,
            connection: Mutex::new(connection),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 在数据库互斥锁内执行闭包。闭包必须保持短小，不做文件复制等耗时操作。
    pub fn with_conn<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut guard = self
            .connection
            .lock()
            .map_err(|_| AppError::internal("数据库连接状态异常，请重启应用"))?;
        operation(&mut guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn open_creates_database_and_applies_migrations() {
        let temp = TempDir::new().unwrap();
        let database = Database::open(temp.path().join("creatorflow.db")).unwrap();

        let tables: Vec<String> = database
            .with_conn(|connection| {
                let mut statement = connection
                    .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")?;
                let rows = statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .unwrap();

        for expected in [
            "works",
            "sections",
            "assets",
            "import_jobs",
            "app_settings",
            "activity_logs",
            "schema_migrations",
        ] {
            assert!(
                tables.iter().any(|table| table == expected),
                "缺少表 {expected}"
            );
        }
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let temp = TempDir::new().unwrap();
        let database = Database::open(temp.path().join("creatorflow.db")).unwrap();

        let result = database.with_conn(|connection| {
            connection
                .execute(
                    "INSERT INTO sections (id, work_id, key, name, relative_path, sort_order, created_at)
                     VALUES ('s1', 'missing-work', 'documents', '文档', '01_文档', 1, '2026-01-01T00:00:00.000Z')",
                    [],
                )
                .map_err(AppError::from)?;
            Ok(())
        });

        assert!(result.is_err(), "外键约束应当阻止孤立分区");
    }
}
