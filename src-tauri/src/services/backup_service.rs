//! 数据库备份、校验与恢复。
//!
//! 使用 SQLite 在线备份接口生成一致性快照；WAL 模式下不能只复制主 `.db` 文件。
//! 备份只覆盖数据库（作品/素材索引、标签、配置、任务记录），不包含作品目录里的
//! 原始媒体文件，也不包含可再生成的缩略图缓存。

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::domain::time::now_iso;
use crate::errors::{AppError, AppResult};
use crate::infrastructure::database::{migrations, Database};
use crate::infrastructure::filesystem;
use crate::repositories::settings as settings_repo;

/// 清单格式版本；字段变更时递增，旧清单仍按已知字段读取。
pub const BACKUP_FORMAT_VERSION: i64 = 1;
const KEY_BACKUP_DIRECTORY: &str = "backup_directory";
const DEFAULT_DIRECTORY_NAME: &str = "backups";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub format_version: i64,
    pub app_version: String,
    pub schema_version: i64,
    pub created_at: String,
    pub database_file: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub integrity_ok: bool,
    pub foreign_keys_ok: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub created_at: Option<String>,
    pub valid: bool,
    pub problem: Option<String>,
    pub manifest: Option<BackupManifest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOverview {
    pub directory: String,
    pub database_path: String,
    pub schema_version: i64,
    pub latest_schema_version: i64,
    pub entries: Vec<BackupEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupVerification {
    pub valid: bool,
    pub integrity_ok: bool,
    pub foreign_keys_ok: bool,
    pub schema_version: i64,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    pub restored_from: String,
    pub backup_schema_version: i64,
    pub current_schema_version: i64,
    pub safety_snapshot: String,
    pub integrity_ok: bool,
    pub foreign_keys_ok: bool,
}

fn default_directory(database: &Database) -> PathBuf {
    database
        .path()
        .parent()
        .map(|parent| parent.join(DEFAULT_DIRECTORY_NAME))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DIRECTORY_NAME))
}

/// 备份目录：优先使用设置值，缺失或非法时回落到数据库同级的 `backups/`。
pub fn backup_directory(database: &Database) -> AppResult<PathBuf> {
    let configured =
        database.with_conn(|connection| settings_repo::get(connection, KEY_BACKUP_DIRECTORY))?;
    let directory = configured
        .and_then(|json| serde_json::from_str::<String>(&json).ok())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| default_directory(database));

    fs::create_dir_all(&directory)
        .map_err(|error| AppError::io_context("无法创建备份目录", error))?;
    Ok(directory)
}

/// 设置备份目录：必须是绝对路径，目录会被创建。
pub fn set_backup_directory(database: &Database, raw_path: &str) -> AppResult<String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("备份目录不能为空"));
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(AppError::validation("请选择绝对路径作为备份目录"));
    }
    fs::create_dir_all(&path).map_err(|error| AppError::io_context("无法创建备份目录", error))?;
    let display = filesystem::path_to_string(&path);
    let json = serde_json::to_string(&display)?;
    database.with_conn(|connection| {
        settings_repo::set(connection, KEY_BACKUP_DIRECTORY, &json, &now_iso())
    })?;
    Ok(display)
}

fn manifest_path(database_file: &Path) -> PathBuf {
    let mut name = database_file.as_os_str().to_os_string();
    name.push(".json");
    PathBuf::from(name)
}

fn compact_timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

fn read_manifest(database_file: &Path) -> Option<BackupManifest> {
    let raw = fs::read_to_string(manifest_path(database_file)).ok()?;
    serde_json::from_str::<BackupManifest>(&raw)
        .map_err(|error| {
            tracing::warn!(path = %database_file.display(), error = %error, "备份清单解析失败");
            error
        })
        .ok()
}

/// 校验单个备份文件：完整性、外键、结构版本与（存在时）清单哈希。
pub fn verify_file(path: &Path) -> BackupVerification {
    let invalid = |problem: String, integrity: bool, fk: bool, version: i64| BackupVerification {
        valid: false,
        integrity_ok: integrity,
        foreign_keys_ok: fk,
        schema_version: version,
        problem: Some(problem),
    };

    if !path.is_file() {
        return invalid("备份文件不存在".to_string(), false, false, 0);
    }

    let opened = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY);
    let connection = match opened {
        Ok(connection) => connection,
        Err(error) => {
            return invalid(format!("无法打开备份文件：{error}"), false, false, 0);
        }
    };

    let integrity: String =
        match connection.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0)) {
            Ok(value) => value,
            Err(error) => {
                return invalid(format!("完整性检查失败：{error}"), false, false, 0);
            }
        };
    let integrity_ok = integrity.eq_ignore_ascii_case("ok");

    let foreign_violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);
    let foreign_keys_ok = foreign_violations == 0;

    let schema_version: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(-1);

    if schema_version < 1 {
        return invalid(
            "这不是 CreatorFlow 的数据库备份（缺少迁移记录）".to_string(),
            integrity_ok,
            foreign_keys_ok,
            schema_version,
        );
    }

    let latest = migrations::latest_version();
    if schema_version > latest {
        return invalid(
            format!("备份来自更新版本的应用（结构版本 {schema_version} > 当前 {latest}）"),
            integrity_ok,
            foreign_keys_ok,
            schema_version,
        );
    }

    if !integrity_ok {
        return invalid(
            format!("数据库完整性检查未通过：{integrity}"),
            integrity_ok,
            foreign_keys_ok,
            schema_version,
        );
    }
    if !foreign_keys_ok {
        return invalid(
            format!("数据库存在 {foreign_violations} 条外键异常"),
            integrity_ok,
            foreign_keys_ok,
            schema_version,
        );
    }

    if let Some(manifest) = read_manifest(path) {
        if let Ok(actual) = filesystem::sha256_file(path) {
            if !manifest.sha256.is_empty() && manifest.sha256 != actual {
                return invalid(
                    "备份文件与清单记录的内容不一致（可能被篡改或损坏）".to_string(),
                    integrity_ok,
                    foreign_keys_ok,
                    schema_version,
                );
            }
        }
    }

    BackupVerification {
        valid: true,
        integrity_ok,
        foreign_keys_ok,
        schema_version,
        problem: None,
    }
}

fn write_manifest(
    database_file: &Path,
    app_version: &str,
    schema_version: i64,
    created_at: &str,
) -> AppResult<BackupManifest> {
    let size_bytes = filesystem::file_size(database_file)?;
    let sha256 = filesystem::sha256_file(database_file)?;
    let manifest = BackupManifest {
        format_version: BACKUP_FORMAT_VERSION,
        app_version: app_version.to_string(),
        schema_version,
        created_at: created_at.to_string(),
        database_file: database_file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        size_bytes,
        sha256,
        integrity_ok: true,
        foreign_keys_ok: true,
    };
    filesystem::write_json_atomic(&manifest_path(database_file), &manifest)?;
    Ok(manifest)
}

/// 创建备份：快照 → 校验 → 写清单。校验不通过时不删除文件，但标记为无效并返回错误。
pub fn create_backup(database: &Database, app_version: &str) -> AppResult<BackupEntry> {
    let directory = backup_directory(database)?;
    let created_at = now_iso();
    let file_name = format!("creatorflow-{}.db", compact_timestamp());
    let target = directory.join(&file_name);

    if target.exists() {
        return Err(AppError::conflict("同一秒内已创建过备份，请稍后再试"));
    }

    database.snapshot_to(&target)?;
    let verification = verify_file(&target);
    if !verification.valid {
        let problem = verification
            .problem
            .unwrap_or_else(|| "备份校验未通过".to_string());
        let _ = write_manifest(
            &target,
            app_version,
            verification.schema_version,
            &created_at,
        );
        return Err(AppError::internal(format!("备份校验未通过：{problem}")));
    }

    let manifest = write_manifest(
        &target,
        app_version,
        verification.schema_version,
        &created_at,
    )?;
    tracing::info!(path = %target.display(), schema = verification.schema_version, "数据库备份完成");

    Ok(BackupEntry {
        path: filesystem::path_to_string(&target),
        file_name,
        size_bytes: manifest.size_bytes,
        created_at: Some(created_at),
        valid: true,
        problem: None,
        manifest: Some(manifest),
    })
}

/// 列出备份目录中的数据库文件及其清单。
pub fn list_backups(database: &Database) -> AppResult<Vec<BackupEntry>> {
    let directory = backup_directory(database)?;
    let mut entries = Vec::new();

    let read_dir = match fs::read_dir(&directory) {
        Ok(read_dir) => read_dir,
        Err(error) => {
            tracing::warn!(path = %directory.display(), error = %error, "无法读取备份目录");
            return Ok(entries);
        }
    };

    for item in read_dir.flatten() {
        let path = item.path();
        if !path.is_file() {
            continue;
        }
        let is_database = path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("db"))
            .unwrap_or(false);
        if !is_database {
            continue;
        }

        let manifest = read_manifest(&path);
        let size_bytes = item.metadata().map(|meta| meta.len()).unwrap_or(0);
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let (valid, problem) = match &manifest {
            None => (false, Some("缺少备份清单，无法确认来源".to_string())),
            Some(manifest) => {
                if manifest.format_version > BACKUP_FORMAT_VERSION {
                    (false, Some("备份清单版本高于当前应用".to_string()))
                } else if !manifest.integrity_ok || !manifest.foreign_keys_ok {
                    (false, Some("清单记录校验未通过".to_string()))
                } else {
                    (true, None)
                }
            }
        };

        entries.push(BackupEntry {
            path: filesystem::path_to_string(&path),
            file_name,
            size_bytes,
            created_at: manifest.as_ref().map(|item| item.created_at.clone()),
            valid,
            problem,
            manifest,
        });
    }

    entries.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(entries)
}

pub fn overview(database: &Database) -> AppResult<BackupOverview> {
    let directory = backup_directory(database)?;
    Ok(BackupOverview {
        directory: filesystem::path_to_string(&directory),
        database_path: filesystem::path_to_string(database.path()),
        schema_version: database.schema_version()?,
        latest_schema_version: migrations::latest_version(),
        entries: list_backups(database)?,
    })
}

/// 恢复备份。
///
/// 步骤：校验输入 → 为当前数据库生成安全快照 → 复制到同目录临时文件并再次校验 →
/// 切换数据库（内部执行迁移）。任一步失败都保留当前数据库。
pub fn restore_backup(
    database: &Database,
    source: &str,
    app_version: &str,
) -> AppResult<RestoreOutcome> {
    let source_path = PathBuf::from(source);
    let verification = verify_file(&source_path);
    if !verification.valid {
        return Err(AppError::validation(format!(
            "无法恢复：{}",
            verification
                .problem
                .unwrap_or_else(|| "备份文件无效".to_string())
        )));
    }

    let directory = backup_directory(database)?;
    let safety = directory.join(format!("pre-restore-{}.db", compact_timestamp()));
    if safety.exists() {
        return Err(AppError::conflict("同一秒内已创建过快照，请稍后再试"));
    }
    database.snapshot_to(&safety)?;
    let safety_check = verify_file(&safety);
    if !safety_check.valid {
        let _ = fs::remove_file(&safety);
        return Err(AppError::internal(
            "当前数据库快照校验失败，已中止恢复以保护现有数据",
        ));
    }
    let _ = write_manifest(
        &safety,
        app_version,
        safety_check.schema_version,
        &now_iso(),
    );

    // 复制到数据库同目录，保证切换时是同卷 rename
    let staged = database.path().with_extension("db.restoring");
    let _ = fs::remove_file(&staged);
    fs::copy(&source_path, &staged)
        .map_err(|error| AppError::io_context("无法准备恢复文件", error))?;

    let staged_check = verify_file(&staged);
    if !staged_check.valid {
        let _ = fs::remove_file(&staged);
        return Err(AppError::internal("恢复文件校验失败，已保留当前数据库"));
    }

    database.replace_with(&staged)?;
    let current_version = database.schema_version()?;

    tracing::info!(
        source = %source_path.display(),
        from_schema = verification.schema_version,
        to_schema = current_version,
        "数据库恢复完成"
    );

    Ok(RestoreOutcome {
        restored_from: filesystem::path_to_string(&source_path),
        backup_schema_version: verification.schema_version,
        current_schema_version: current_version,
        safety_snapshot: filesystem::path_to_string(&safety),
        integrity_ok: true,
        foreign_keys_ok: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::settings_service;
    use tempfile::TempDir;

    fn prepare() -> (TempDir, Database) {
        let temp = TempDir::new().unwrap();
        let database = Database::open(temp.path().join("creatorflow.db")).unwrap();
        let works = temp.path().join("works");
        fs::create_dir_all(&works).unwrap();
        settings_service::set_workspace_root(&database, works.to_string_lossy().as_ref()).unwrap();
        (temp, database)
    }

    #[test]
    fn creates_verified_backup_with_manifest() {
        let (_temp, database) = prepare();
        let entry = create_backup(&database, "0.3.0").unwrap();

        assert!(entry.valid);
        assert!(Path::new(&entry.path).is_file());
        let manifest = entry.manifest.unwrap();
        assert_eq!(manifest.schema_version, migrations::latest_version());
        assert!(manifest.integrity_ok && manifest.foreign_keys_ok);
        assert!(manifest.sha256.len() == 64);

        let verification = verify_file(Path::new(&entry.path));
        assert!(verification.valid, "{:?}", verification.problem);
    }

    #[test]
    fn rejects_non_database_file() {
        let (temp, database) = prepare();
        let junk = temp.path().join("伪造.db");
        fs::write(&junk, b"not a database").unwrap();

        let verification = verify_file(&junk);
        assert!(!verification.valid);
        assert!(verification.problem.is_some());

        let error = restore_backup(&database, junk.to_string_lossy().as_ref(), "0.3.0")
            .expect_err("不应恢复无效文件");
        assert!(error.message.contains("无法恢复"));
        // 当前数据库仍然可用
        assert!(database.schema_version().unwrap() >= 1);
    }

    #[test]
    fn detects_backup_replaced_after_manifest_was_written() {
        let (temp, database) = prepare();
        let entry = create_backup(&database, "0.3.0").unwrap();
        let path = PathBuf::from(&entry.path);

        // 复制成另一份仍然合法的数据库，内容与清单记录不一致
        let replacement = temp.path().join("替换.db");
        fs::copy(&path, &replacement).unwrap();
        {
            let connection = Connection::open(&replacement).unwrap();
            connection
                .execute(
                    "INSERT INTO app_settings (key, value_json, updated_at)
                     VALUES ('tamper', '\"1\"', '2030-01-01T00:00:00.000Z')",
                    [],
                )
                .unwrap();
        }
        fs::copy(manifest_path(&path), manifest_path(&replacement)).unwrap();

        let verification = verify_file(&replacement);
        assert!(!verification.valid, "哈希不一致的备份不应通过校验");
        assert!(
            verification.problem.unwrap().contains("不一致"),
            "应命中文档哈希差异"
        );
    }

    #[test]
    fn restores_backup_and_keeps_safety_snapshot() {
        let (temp, database) = prepare();
        let work = crate::services::work_service::create_work(
            &database,
            crate::services::work_service::CreateWorkInput {
                name: "备份前".to_string(),
                description: None,
                root_directory: None,
                template_key: None,
            },
        )
        .unwrap();

        let entry = create_backup(&database, "0.3.0").unwrap();

        // 备份后新增一条作品，恢复后应消失
        crate::services::work_service::create_work(
            &database,
            crate::services::work_service::CreateWorkInput {
                name: "备份后".to_string(),
                description: None,
                root_directory: None,
                template_key: None,
            },
        )
        .unwrap();
        assert_eq!(
            crate::services::work_service::list_works(&database, true)
                .unwrap()
                .len(),
            2
        );

        let outcome = restore_backup(&database, &entry.path, "0.3.0").unwrap();
        assert!(Path::new(&outcome.safety_snapshot).is_file());

        let works = crate::services::work_service::list_works(&database, true).unwrap();
        assert_eq!(works.len(), 1);
        assert_eq!(works[0].id, work.id);

        // 恢复后的数据库仍可正常写入
        crate::services::work_service::create_work(
            &database,
            crate::services::work_service::CreateWorkInput {
                name: "恢复后".to_string(),
                description: None,
                root_directory: None,
                template_key: None,
            },
        )
        .unwrap();
        let _ = temp;
    }

    #[test]
    fn rejects_backup_from_newer_schema() {
        let (temp, database) = prepare();
        let entry = create_backup(&database, "0.3.0").unwrap();
        let path = PathBuf::from(&entry.path);

        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name, applied_at)
                     VALUES (99, 'future', '2030-01-01T00:00:00.000Z')",
                    [],
                )
                .unwrap();
        }

        let verification = verify_file(&path);
        assert!(!verification.valid);
        assert!(verification.problem.unwrap().contains("更新版本"));

        let error = restore_backup(&database, &entry.path, "0.3.0").unwrap_err();
        assert!(error.message.contains("无法恢复"));
        let _ = temp;
    }
}
