//! 回收站操作。
//!
//! 删除作品与素材时，真实文件不直接抹掉，而是交给系统回收站，
//! 用户仍可从回收站还原。数据库里的记录只做软删除。

use std::path::Path;

use crate::errors::{AppError, AppResult};

/// 把文件或目录移入系统回收站。
///
/// - 路径已不存在时视为已完成（文件可能被用户手动移走）。
/// - 返回错误时调用方必须中止后续的数据库变更，避免出现「记录没了、文件还在」。
pub fn move_to_recycle_bin(path: &Path) -> AppResult<()> {
    if !path.exists() {
        tracing::debug!(path = %path.display(), "路径不存在，跳过回收站操作");
        return Ok(());
    }
    if !path.is_absolute() {
        return Err(AppError::internal("回收站操作需要绝对路径")
            .with_detail(path.to_string_lossy().to_string()));
    }

    platform::move_to_recycle_bin(path)
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::UI::Shell::{
        SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
        FOF_WANTNUKEWARNING, FO_DELETE, SHFILEOPSTRUCTW,
    };

    use crate::errors::{AppError, AppResult};
    use crate::infrastructure::filesystem;

    pub fn move_to_recycle_bin(path: &Path) -> AppResult<()> {
        // SHFileOperationW 不接受 \\?\ 前缀，这里统一还原成普通路径
        let target = filesystem::path_to_string(path);
        let mut wide: Vec<u16> = OsStr::new(&target).encode_wide().collect();
        if wide.contains(&0) {
            return Err(AppError::internal("路径包含非法字符，无法移入回收站"));
        }
        // pFrom 需要双 null 结尾，可放多个路径；这里只处理单个
        wide.push(0);
        wide.push(0);

        // FOF_WANTNUKEWARNING 会在「文件过大、回收站装不下」时弹出系统提示，
        // 避免在用户不知情的情况下永久删除。
        let flags = FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI;
        let mut operation = SHFILEOPSTRUCTW {
            hwnd: std::ptr::null_mut(),
            wFunc: FO_DELETE,
            pFrom: wide.as_ptr(),
            pTo: std::ptr::null(),
            fFlags: (flags | FOF_WANTNUKEWARNING) as u16,
            fAnyOperationsAborted: 0,
            hNameMappings: std::ptr::null_mut(),
            lpszProgressTitle: std::ptr::null(),
        };

        // SAFETY: 结构体字段均指向本次调用期间有效的缓冲区，符合 Win32 约定。
        let code = unsafe { SHFileOperationW(&mut operation) };

        if operation.fAnyOperationsAborted != 0 {
            return Err(AppError::cancelled("已取消移入回收站"));
        }
        if code != 0 {
            let detail = describe_code(code);
            tracing::warn!(code, path = %target, "移入回收站失败");
            return Err(AppError::internal("无法将文件移入回收站")
                .with_detail(format!("{detail}（错误码 {code}）")));
        }
        if path.exists() {
            return Err(AppError::internal("文件仍在原位置，回收站操作未生效"));
        }
        Ok(())
    }

    /// SHFileOperation 的常见返回码，用于日志与错误详情。
    fn describe_code(code: i32) -> &'static str {
        match code {
            0x71 => "源文件与目标文件相同",
            0x74 => "不能删除根目录",
            0x75 => "操作被取消",
            0x77 | 0x78 => "目标被占用或路径过深",
            0x7A => "文件名无效",
            0x7C | 0x7E => "目标路径无效",
            0x81 => "文件正在被其他程序使用",
            0x20 => "共享冲突，文件可能正被打开",
            0x21 => "文件被占用",
            _ => "系统拒绝删除该文件",
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn moves_real_file_into_recycle_bin() {
            let directory = tempfile::TempDir::new().unwrap();
            let file = directory.path().join("待删除.txt");
            std::fs::write(&file, b"recycle me").unwrap();

            crate::infrastructure::recycle_bin::move_to_recycle_bin(&file).expect("应能移入回收站");
            assert!(!file.exists(), "文件应已离开原位置");
        }

        #[test]
        fn missing_path_is_treated_as_done() {
            let directory = tempfile::TempDir::new().unwrap();
            let file = directory.path().join("不存在.txt");
            assert!(crate::infrastructure::recycle_bin::move_to_recycle_bin(&file).is_ok());
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    use crate::errors::{AppError, AppResult};

    pub fn move_to_recycle_bin(_path: &Path) -> AppResult<()> {
        Err(AppError::internal("当前系统不支持回收站"))
    }
}
