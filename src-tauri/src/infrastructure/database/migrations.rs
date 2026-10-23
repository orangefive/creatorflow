use rusqlite::{params, Connection};

use crate::domain::time::now_iso;
use crate::errors::{AppError, AppResult};

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

/// 有序迁移列表：只能追加，不能修改已发布的条目。
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "0001_initial",
    sql: include_str!("../../../migrations/0001_initial.sql"),
}];

pub fn latest_version() -> i64 {
    MIGRATIONS
        .last()
        .map(|migration| migration.version)
        .unwrap_or(0)
}

/// 逐条在事务中应用未执行的迁移；任一失败则整体回滚并返回错误。
pub fn run(connection: &mut Connection) -> AppResult<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version    INTEGER PRIMARY KEY,
                name       TEXT NOT NULL,
                applied_at TEXT NOT NULL
            );",
        )
        .map_err(|error| AppError::db_context("无法初始化迁移记录表", error))?;

    for migration in MIGRATIONS {
        let applied: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1",
                params![migration.version],
                |row| row.get(0),
            )
            .map_err(|error| AppError::db_context("无法读取迁移状态", error))?;

        if applied > 0 {
            continue;
        }

        let transaction = connection
            .transaction()
            .map_err(|error| AppError::db_context("无法开启迁移事务", error))?;
        transaction
            .execute_batch(migration.sql)
            .map_err(|error| AppError::db_context("执行迁移脚本失败", error))?;
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![migration.version, migration.name, now_iso()],
            )
            .map_err(|error| AppError::db_context("写入迁移记录失败", error))?;
        transaction
            .commit()
            .map_err(|error| AppError::db_context("提交迁移事务失败", error))?;

        tracing::info!(
            version = migration.version,
            name = migration.name,
            "已应用数据库迁移"
        );
    }

    Ok(())
}
