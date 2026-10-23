use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf, MAIN_SEPARATOR_STR};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::errors::{AppError, AppResult};

/// 固定缓冲区流式复制，避免把大文件整体读入内存。
pub const COPY_BUFFER_SIZE: usize = 1024 * 1024;
const MAX_UNIQUE_ATTEMPTS: u32 = 9999;

pub fn ensure_dir(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path)
        .map_err(|error| AppError::io_context(&format!("无法创建目录 {}", path.display()), error))
}

/// 将任意来源的相对路径规范化为以 `/` 分隔的安全形式，拒绝绝对路径与 `..`。
pub fn normalize_relative(relative: &str) -> AppResult<String> {
    let trimmed = relative.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("路径不能为空"));
    }
    let unified = trimmed.replace('\\', "/");
    if unified.starts_with('/') {
        return Err(AppError::validation("路径必须是作品内的相对路径"));
    }
    if unified.contains(':') {
        return Err(AppError::validation("路径不能包含盘符或冒号"));
    }

    let mut parts: Vec<&str> = Vec::new();
    for part in unified.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(AppError::validation("路径不能包含上级目录引用"));
        }
        if part.chars().any(|ch| ch.is_control()) {
            return Err(AppError::validation("路径不能包含控制字符"));
        }
        parts.push(part);
    }

    if parts.is_empty() {
        return Err(AppError::validation("路径无效"));
    }
    Ok(parts.join("/"))
}

/// 转换为便于展示与存储的路径字符串：去掉 Windows canonicalize 产生的 `\\?\` 前缀。
pub fn path_to_string(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();
    if let Some(stripped) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{stripped}");
    }
    if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        return stripped.to_string();
    }
    raw
}

/// 把相对路径解析为作品根目录下的绝对路径，并确保结果仍在根目录内。
/// 对尚不存在的文件，沿最近的已存在祖先目录校验，避免符号链接逃逸。
pub fn resolve_within(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let normalized = normalize_relative(relative)?;
    let root_canonical = root
        .canonicalize()
        .map_err(|error| AppError::io_context("无法访问作品目录", error))?;
    let joined = root_canonical.join(normalized.replace('/', MAIN_SEPARATOR_STR));

    let mut existing = joined.as_path();
    while !existing.exists() {
        existing = existing
            .parent()
            .ok_or_else(|| AppError::validation("路径无效"))?;
    }
    let canonical = existing
        .canonicalize()
        .map_err(|error| AppError::io_context("无法解析目标路径", error))?;
    if !canonical.starts_with(&root_canonical) {
        return Err(AppError::validation("路径超出作品目录范围"));
    }
    Ok(joined)
}

/// 校验单个文件名（不含目录分隔符）。
pub fn validate_file_name(name: &str) -> AppResult<String> {
    if name.trim().is_empty() {
        return Err(AppError::validation("文件名不能为空"));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(AppError::validation("文件名不能包含路径分隔符"));
    }
    if name.chars().any(|ch| ch.is_control()) {
        return Err(AppError::validation("文件名不能包含控制字符"));
    }
    if name.chars().count() > 180 {
        return Err(AppError::validation("文件名过长"));
    }
    Ok(name.to_string())
}

fn split_file_name(name: &str) -> (String, Option<String>) {
    match name.rfind('.') {
        Some(index) if index > 0 && index < name.len() - 1 => (
            name[..index].to_string(),
            Some(name[index + 1..].to_string()),
        ),
        _ => (name.to_string(), None),
    }
}

/// 生成不冲突的文件名：`名称 (1).扩展名`、`名称 (2).扩展名` ……
/// 只读取目录、不创建文件，调用方负责最终以 `create_new` 方式落盘。
pub fn unique_file_name(dir: &Path, desired_name: &str) -> AppResult<String> {
    let base = validate_file_name(desired_name)?;
    if !dir.join(&base).exists() {
        return Ok(base);
    }

    let (stem, extension) = split_file_name(&base);
    for index in 1..=MAX_UNIQUE_ATTEMPTS {
        let candidate = match &extension {
            Some(ext) => format!("{stem} ({index}).{ext}"),
            None => format!("{stem} ({index})"),
        };
        if !dir.join(&candidate).exists() {
            return Ok(candidate);
        }
    }
    Err(AppError::conflict("同名文件过多，无法生成唯一的文件名"))
}

pub fn file_size(path: &Path) -> AppResult<u64> {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|error| AppError::io_context("无法读取文件信息", error))
}

/// 流式复制到目标文件。目标必须不存在（`create_new`），从机制上杜绝覆盖。
pub fn copy_streaming<F>(
    source: &Path,
    target: &Path,
    cancel: &AtomicBool,
    mut on_progress: F,
) -> AppResult<u64>
where
    F: FnMut(u64),
{
    let mut reader = BufReader::with_capacity(
        COPY_BUFFER_SIZE,
        File::open(source)
            .map_err(|error| AppError::io_context("无法读取源文件，可能已被移动或删除", error))?,
    );
    let expected_size = reader
        .get_ref()
        .metadata()
        .map_err(|error| AppError::io_context("无法读取源文件信息", error))?
        .len();

    let target_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                AppError::conflict("目标文件已存在，已阻止覆盖").with_detail(error.to_string())
            } else {
                AppError::io_context(&format!("无法写入目标文件 {}", target.display()), error)
            }
        })?;
    let mut writer = BufWriter::with_capacity(COPY_BUFFER_SIZE, target_file);
    let mut buffer = vec![0_u8; COPY_BUFFER_SIZE];
    let mut copied: u64 = 0;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::cancelled("导入已取消"));
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| AppError::io_context("读取源文件失败", error))?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|error| AppError::io_context("写入目标文件失败，可能是磁盘空间不足", error))?;
        copied += read as u64;
        on_progress(copied);
    }

    writer
        .flush()
        .map_err(|error| AppError::io_context("写入目标文件失败", error))?;
    let written = writer
        .get_ref()
        .metadata()
        .map_err(|error| AppError::io_context("无法校验目标文件", error))?
        .len();
    if written != expected_size || copied != expected_size {
        return Err(AppError::internal(format!(
            "文件大小校验失败：源文件 {expected_size} 字节，目标文件 {written} 字节"
        )));
    }
    Ok(copied)
}

pub fn remove_file_if_exists(path: &Path) -> AppResult<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AppError::io_context(
            &format!("无法清理临时文件 {}", path.display()),
            error,
        )),
    }
}

/// 以「临时文件 + 重命名」方式写入 JSON，减少中途失败留下半截文件的风险。
pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let payload = serde_json::to_string_pretty(value)?;
    let temp_path = path.with_extension("json.tmp");
    {
        let mut file = File::create(&temp_path)
            .map_err(|error| AppError::io_context("无法写入清单文件", error))?;
        file.write_all(payload.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| AppError::io_context("无法写入清单文件", error))?;
    }
    if fs::rename(&temp_path, path).is_err() {
        // Windows 下目标已存在时 rename 会失败，退化为先删除再改名。
        if path.exists() {
            fs::remove_file(path)
                .map_err(|error| AppError::io_context("无法更新清单文件", error))?;
        }
        fs::rename(&temp_path, path)
            .map_err(|error| AppError::io_context("无法更新清单文件", error))?;
    }
    Ok(())
}

/// 递归删除目录，仅用于清理本次创建且仍为空的目录。
pub fn remove_dir_if_empty(path: &Path) -> bool {
    match fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return false;
            }
            fs::remove_dir(path).is_ok()
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tempfile::TempDir;

    #[test]
    fn normalize_relative_accepts_nested_paths() {
        assert_eq!(
            normalize_relative("01_文档/脚本/a.md").unwrap(),
            "01_文档/脚本/a.md"
        );
        assert_eq!(
            normalize_relative("01_文档\\脚本\\a.md").unwrap(),
            "01_文档/脚本/a.md"
        );
        assert_eq!(normalize_relative("./a/b.mp4").unwrap(), "a/b.mp4");
    }

    #[test]
    fn normalize_relative_rejects_traversal_and_absolute_paths() {
        assert!(normalize_relative("../outside.txt").is_err());
        assert!(normalize_relative("01_文档/../../outside.txt").is_err());
        assert!(normalize_relative("C:\\Windows\\system.ini").is_err());
        assert!(normalize_relative("/etc/passwd").is_err());
        assert!(normalize_relative("   ").is_err());
    }

    #[test]
    fn resolve_within_keeps_paths_inside_root() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("01_文档")).unwrap();

        let resolved = resolve_within(root, "01_文档/a.md").unwrap();
        assert!(resolved.starts_with(root.canonicalize().unwrap()));

        let traversal = resolve_within(root, "../escape.txt");
        assert!(traversal.is_err());
    }

    #[test]
    fn unique_file_name_appends_incrementing_suffix() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        assert_eq!(unique_file_name(dir, "脚本.md").unwrap(), "脚本.md");

        fs::write(dir.join("脚本.md"), b"a").unwrap();
        assert_eq!(unique_file_name(dir, "脚本.md").unwrap(), "脚本 (1).md");

        fs::write(dir.join("脚本 (1).md"), b"b").unwrap();
        assert_eq!(unique_file_name(dir, "脚本 (2).md").unwrap(), "脚本 (2).md");
        assert_eq!(unique_file_name(dir, "脚本.md").unwrap(), "脚本 (2).md");
    }

    #[test]
    fn unique_file_name_handles_names_without_extension() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        fs::write(dir.join("README"), b"a").unwrap();
        assert_eq!(unique_file_name(dir, "README").unwrap(), "README (1)");
    }

    #[test]
    fn copy_streaming_keeps_source_and_matches_size() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.bin");
        let payload = vec![7_u8; 3 * 1024 * 1024 + 17];
        fs::write(&source, &payload).unwrap();

        let target = temp.path().join("target.bin");
        let cancel = AtomicBool::new(false);
        let mut last_progress = 0_u64;
        let copied =
            copy_streaming(&source, &target, &cancel, |value| last_progress = value).unwrap();

        assert_eq!(copied, payload.len() as u64);
        assert_eq!(last_progress, payload.len() as u64);
        assert!(source.exists(), "源文件必须保留");
        assert_eq!(fs::read(&source).unwrap(), payload);
        assert_eq!(fs::read(&target).unwrap(), payload);
    }

    #[test]
    fn copy_streaming_refuses_to_overwrite_existing_target() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        fs::write(&source, b"new").unwrap();
        fs::write(&target, b"existing").unwrap();

        let cancel = AtomicBool::new(false);
        let error = copy_streaming(&source, &target, &cancel, |_| {}).unwrap_err();
        assert_eq!(error.code, crate::errors::ErrorCode::Conflict);
        assert_eq!(fs::read(&target).unwrap(), b"existing");
    }

    #[test]
    fn copy_streaming_honours_cancellation() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("big.bin");
        fs::write(&source, vec![1_u8; 4 * 1024 * 1024]).unwrap();
        let target = temp.path().join("copy.bin");
        let cancel = AtomicBool::new(true);

        let error = copy_streaming(&source, &target, &cancel, |_| {}).unwrap_err();
        assert_eq!(error.code, crate::errors::ErrorCode::Cancelled);
        assert!(!target.exists() || fs::metadata(&target).unwrap().len() < 4 * 1024 * 1024);
    }
}
