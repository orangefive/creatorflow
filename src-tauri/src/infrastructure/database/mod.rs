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

/// 打开连接并应用 PRAGMA 与迁移。恢复流程换库后也走这里，保证行为一致。
fn open_with_migrations(path: &Path) -> AppResult<Connection> {
    let mut connection =
        Connection::open(path).map_err(|error| AppError::db_context("无法打开数据库", error))?;

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

    Ok(connection)
}

/// WAL/SHM 伴生文件路径。
fn wal_paths(path: &Path) -> (PathBuf, PathBuf) {
    (path.with_extension("db-wal"), path.with_extension("db-shm"))
}

impl Database {
    pub fn open(path: impl Into<PathBuf>) -> AppResult<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| AppError::io_context("无法创建应用数据目录", error))?;
        }

        let connection = open_with_migrations(&path)?;

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

    /// 当前已应用的最高迁移版本。
    pub fn schema_version(&self) -> AppResult<i64> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                    [],
                    |row| row.get(0),
                )
                .map_err(|error| AppError::db_context("无法读取数据库结构版本", error))
        })
    }

    /// 使用 SQLite 在线备份接口生成一致性快照。
    ///
    /// 不能简单复制 `.db` 文件：WAL 模式下部分已提交数据可能仍在 `-wal` 中。
    pub fn snapshot_to(&self, target: &Path) -> AppResult<()> {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| AppError::io_context("无法创建备份目录", error))?;
        }
        if target.exists() {
            return Err(AppError::conflict("备份目标已存在，请勿覆盖"));
        }

        self.with_conn(|source| {
            let mut destination = Connection::open(target)
                .map_err(|error| AppError::db_context("无法创建备份文件", error))?;
            {
                let backup = rusqlite::backup::Backup::new(source, &mut destination)
                    .map_err(|error| AppError::db_context("无法启动数据库备份", error))?;
                backup
                    .run_to_completion(64, std::time::Duration::from_millis(20), None)
                    .map_err(|error| AppError::db_context("数据库备份中断", error))?;
            }
            destination
                .execute_batch("PRAGMA journal_mode = DELETE;")
                .ok();
            Ok(())
        })?;

        tracing::info!(path = %target.display(), "已生成数据库快照");
        Ok(())
    }

    /// 用校验过的候选文件替换当前数据库。
    ///
    /// 全程持有连接锁，形成写入屏障：先 checkpoint 并关闭旧连接，再把旧文件挪到
    /// `.pre-restore`，候选文件改名到位后重新打开并执行迁移。任何一步失败都尝试回滚
    /// 到旧文件，避免留下不可用的数据库。
    pub fn replace_with(&self, candidate: &Path) -> AppResult<()> {
        let mut guard = self
            .connection
            .lock()
            .map_err(|_| AppError::internal("数据库连接状态异常，请重启应用"))?;

        let placeholder = Connection::open_in_memory()
            .map_err(|error| AppError::db_context("无法准备数据库切换", error))?;
        let old_connection = std::mem::replace(&mut *guard, placeholder);
        let _ = old_connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        drop(old_connection);

        let current = self.path.clone();
        let safety = current.with_extension("db.pre-restore");
        let (wal, shm) = wal_paths(&current);
        let _ = fs::remove_file(&safety);
        let _ = fs::remove_file(&wal);
        let _ = fs::remove_file(&shm);

        if current.exists() {
            fs::rename(&current, &safety)
                .map_err(|error| AppError::io_context("无法为当前数据库创建回滚副本", error))?;
        }

        if let Err(error) = fs::rename(candidate, &current) {
            // 候选文件没能就位，把旧库放回去
            if safety.exists() {
                let _ = fs::rename(&safety, &current);
            }
            *guard = open_with_migrations(&current).map_err(|restore_error| {
                AppError::internal("恢复失败，且重新打开原数据库时出错，请重启应用")
                    .with_detail(format!("{restore_error:?}"))
            })?;
            return Err(AppError::io_context("无法替换数据库文件", error));
        }

        match open_with_migrations(&current) {
            Ok(connection) => {
                *guard = connection;
                let _ = fs::remove_file(&safety);
                tracing::info!(path = %current.display(), "数据库已切换为恢复后的文件");
                Ok(())
            }
            Err(error) => {
                // 打开/迁移失败：回滚到切换前的数据库
                let broken = current.with_extension("db.restore-failed");
                let _ = fs::remove_file(&broken);
                let _ = fs::rename(&current, &broken);
                if safety.exists() {
                    let _ = fs::rename(&safety, &current);
                }
                *guard = open_with_migrations(&current).map_err(|rollback_error| {
                    AppError::internal("恢复失败，且回滚原数据库时出错，请从备份目录手动恢复")
                        .with_detail(format!("{rollback_error:?}"))
                })?;
                Err(
                    AppError::internal("恢复后的数据库无法打开，已回滚到原数据库")
                        .with_detail(error.detail.unwrap_or(error.message)),
                )
            }
        }
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
