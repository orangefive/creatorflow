//! 缩略图缓存管理：统计占用、容量上限与清理。
//!
//! 缓存目录中的文件都可以重新生成，因此清理只会删除这个目录内的 `.jpg` 缩略图，
//! 不触碰作品目录、素材索引或用户的其他文件。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use serde::Serialize;

use crate::domain::time::now_iso;
use crate::errors::AppResult;
use crate::infrastructure::database::Database;
use crate::infrastructure::filesystem;
use crate::repositories::settings as settings_repo;

const KEY_CACHE_LIMIT: &str = "thumbnail_cache_limit_bytes";
pub const DEFAULT_CACHE_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
pub const MIN_CACHE_LIMIT: u64 = 256 * 1024 * 1024;
pub const MAX_CACHE_LIMIT: u64 = 20 * 1024 * 1024 * 1024;

/// 刚生成不久的文件可能仍被写入，清理时跳过，避免半成品。
const MIN_FILE_AGE: Duration = Duration::from_secs(60);
/// 每生成这么多次缩略图检查一次容量，避免每次写盘都扫描目录。
const ENFORCE_INTERVAL: u64 = 25;
static WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub directory: String,
    pub total_bytes: u64,
    pub file_count: u64,
    pub limit_bytes: u64,
    pub min_limit_bytes: u64,
    pub max_limit_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheCleanupResult {
    pub removed_files: u64,
    pub freed_bytes: u64,
    pub remaining_bytes: u64,
    pub skipped_files: u64,
    pub failures: u64,
}

struct CacheFile {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

/// 读取容量上限；缺失或非法时使用默认值，越界则裁剪到允许范围。
pub fn limit_bytes(database: &Database) -> AppResult<u64> {
    let raw = database.with_conn(|connection| settings_repo::get(connection, KEY_CACHE_LIMIT))?;
    let value = raw
        .and_then(|json| serde_json::from_str::<u64>(&json).ok())
        .unwrap_or(DEFAULT_CACHE_LIMIT);
    Ok(value.clamp(MIN_CACHE_LIMIT, MAX_CACHE_LIMIT))
}

/// 设置容量上限，返回实际生效值（已裁剪到 256 MiB—20 GiB）。
pub fn set_limit(database: &Database, bytes: u64) -> AppResult<u64> {
    let clamped = bytes.clamp(MIN_CACHE_LIMIT, MAX_CACHE_LIMIT);
    let json = serde_json::to_string(&clamped)?;
    database.with_conn(|connection| {
        settings_repo::set(connection, KEY_CACHE_LIMIT, &json, &now_iso())
    })?;
    Ok(clamped)
}

fn collect_files(root: &Path) -> Vec<CacheFile> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(directory) = stack.pop() {
        let Ok(read_dir) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                stack.push(path);
                continue;
            }
            let is_thumbnail = path
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg"))
                .unwrap_or(false);
            if !is_thumbnail {
                continue;
            }
            files.push(CacheFile {
                path,
                size: metadata.len(),
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }

    files
}

fn usage(root: &Path) -> (u64, u64) {
    collect_files(root)
        .iter()
        .fold((0_u64, 0_u64), |(bytes, count), file| {
            (bytes + file.size, count + 1)
        })
}

pub fn stats(database: &Database, cache_directory: &Path) -> AppResult<CacheStats> {
    let (total_bytes, file_count) = usage(cache_directory);
    Ok(CacheStats {
        directory: filesystem::path_to_string(cache_directory),
        total_bytes,
        file_count,
        limit_bytes: limit_bytes(database)?,
        min_limit_bytes: MIN_CACHE_LIMIT,
        max_limit_bytes: MAX_CACHE_LIMIT,
    })
}

/// 清理全部可再生成的缩略图缓存。
pub fn clear_all(cache_directory: &Path) -> AppResult<CacheCleanupResult> {
    if !cache_directory.exists() {
        return Ok(CacheCleanupResult {
            removed_files: 0,
            freed_bytes: 0,
            remaining_bytes: 0,
            skipped_files: 0,
            failures: 0,
        });
    }

    let files = collect_files(cache_directory);
    let mut result = CacheCleanupResult {
        removed_files: 0,
        freed_bytes: 0,
        remaining_bytes: 0,
        skipped_files: 0,
        failures: 0,
    };

    for file in files {
        match fs::remove_file(&file.path) {
            Ok(()) => {
                result.removed_files += 1;
                result.freed_bytes += file.size;
            }
            Err(error) => {
                if error.kind() == std::io::ErrorKind::NotFound {
                    result.removed_files += 1;
                } else {
                    // 正在生成中的文件会因占用而失败，跳过即可
                    result.failures += 1;
                    tracing::debug!(path = %file.path.display(), error = %error, "删除缩略图失败");
                }
            }
        }
    }

    // 顺手清理空的作品缓存目录
    if let Ok(read_dir) = fs::read_dir(cache_directory) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir(&path);
            }
        }
    }

    result.remaining_bytes = usage(cache_directory).0;
    tracing::info!(
        removed = result.removed_files,
        freed = result.freed_bytes,
        "缩略图缓存已清理"
    );
    Ok(result)
}

/// 超过容量上限时按最久未使用顺序删除旧缩略图。
pub fn enforce_limit(database: &Database, cache_directory: &Path) -> AppResult<CacheCleanupResult> {
    let limit = limit_bytes(database)?;
    Ok(enforce_with_limit(cache_directory, limit))
}

fn enforce_with_limit(cache_directory: &Path, limit: u64) -> CacheCleanupResult {
    let mut files = collect_files(cache_directory);
    let total: u64 = files.iter().map(|file| file.size).sum();

    let mut result = CacheCleanupResult {
        removed_files: 0,
        freed_bytes: 0,
        remaining_bytes: total,
        skipped_files: 0,
        failures: 0,
    };
    if total <= limit {
        return result;
    }

    files.sort_by_key(|file| file.modified);
    let now = SystemTime::now();
    let mut remaining = total;

    for file in files {
        if remaining <= limit {
            break;
        }
        let age = now.duration_since(file.modified).unwrap_or(Duration::ZERO);
        if age < MIN_FILE_AGE {
            result.skipped_files += 1;
            continue;
        }
        match fs::remove_file(&file.path) {
            Ok(()) => {
                remaining = remaining.saturating_sub(file.size);
                result.removed_files += 1;
                result.freed_bytes += file.size;
            }
            Err(error) => {
                result.failures += 1;
                tracing::debug!(path = %file.path.display(), error = %error, "清理缩略图失败");
            }
        }
    }

    result.remaining_bytes = remaining;
    if result.removed_files > 0 {
        tracing::info!(
            removed = result.removed_files,
            freed = result.freed_bytes,
            limit,
            "缩略图缓存超出上限，已按最久未使用清理"
        );
    }
    result
}

/// 生成缩略图后调用：每 N 次检查一次容量，异常只记录日志。
pub fn maybe_enforce(database: &Database, cache_directory: &Path) {
    let count = WRITE_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    if count % ENFORCE_INTERVAL != 0 {
        return;
    }
    if let Err(error) = enforce_limit(database, cache_directory) {
        tracing::warn!(message = %error.message, "缩略图缓存容量检查失败");
    }
}

/// 判断路径是否位于缓存目录内（清理前的最后一道校验）。
pub fn is_inside(cache_directory: &Path, candidate: &Path) -> bool {
    let Ok(root) = fs::canonicalize(cache_directory) else {
        return false;
    };
    let Ok(target) = fs::canonicalize(candidate) else {
        return false;
    };
    target.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_thumbnail(root: &Path, work: &str, name: &str, bytes: usize) -> PathBuf {
        let directory = root.join(work);
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join(name);
        fs::write(&path, vec![0_u8; bytes]).unwrap();
        path
    }

    #[test]
    fn reports_usage_and_file_count() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("thumbnails");
        write_thumbnail(&root, "w1", "a-1.jpg", 1024);
        write_thumbnail(&root, "w2", "b-1.jpg", 2048);
        fs::write(root.join("w1").join("ignore.txt"), b"keep").unwrap();

        let (bytes, count) = usage(&root);
        assert_eq!(count, 2);
        assert_eq!(bytes, 3072);
    }

    #[test]
    fn clear_all_removes_only_thumbnails() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("thumbnails");
        let keep = write_thumbnail(&root, "w1", "keep.txt", 16);
        let thumb = write_thumbnail(&root, "w1", "a-1.jpg", 1024);

        let result = clear_all(&root).unwrap();
        assert_eq!(result.removed_files, 1);
        assert_eq!(result.freed_bytes, 1024);
        assert!(!thumb.exists());
        assert!(keep.exists(), "非缩略图文件不应被删除");
    }

    #[test]
    fn enforces_limit_by_removing_oldest_files() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("thumbnails");

        for index in 0..3 {
            let path = write_thumbnail(&root, "w1", &format!("a-{index}.jpg"), 4096);
            let stamp = SystemTime::now() - Duration::from_secs(600 - index * 60);
            set_modified(&path, stamp);
        }

        let result = enforce_with_limit(&root, 5000);
        assert_eq!(result.removed_files, 2, "应删除两份最旧缓存才能降到上限内");
        assert!(result.remaining_bytes <= 5000);
        // 最新的一份应保留
        assert!(root.join("w1").join("a-2.jpg").is_file());
        assert!(!root.join("w1").join("a-0.jpg").exists());
    }

    #[test]
    fn skips_recent_files_when_enforcing() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("thumbnails");
        write_thumbnail(&root, "w1", "fresh.jpg", 8192);

        let result = enforce_with_limit(&root, 1024);
        assert_eq!(result.removed_files, 0, "刚写入的文件应跳过");
        assert_eq!(result.skipped_files, 1);
        assert!(root.join("w1").join("fresh.jpg").is_file());
    }

    #[test]
    fn clamps_configured_limit() {
        let temp = TempDir::new().unwrap();
        let database = Database::open(temp.path().join("creatorflow.db")).unwrap();

        assert_eq!(limit_bytes(&database).unwrap(), DEFAULT_CACHE_LIMIT);
        assert_eq!(
            set_limit(&database, 1024).unwrap(),
            MIN_CACHE_LIMIT,
            "低于下限应被裁剪"
        );
        assert_eq!(
            set_limit(&database, u64::MAX).unwrap(),
            MAX_CACHE_LIMIT,
            "高于上限应被裁剪"
        );
    }

    #[test]
    fn rejects_paths_outside_cache_directory() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("thumbnails");
        fs::create_dir_all(&root).unwrap();
        let outside = temp.path().join("outside.jpg");
        fs::write(&outside, b"x").unwrap();

        assert!(!is_inside(&root, &outside));
        assert!(is_inside(&root, &root));
    }

    fn set_modified(path: &Path, time: SystemTime) {
        let file = fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }
}
