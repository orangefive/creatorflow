use serde::{Deserialize, Serialize};

use crate::errors::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Document,
    Audio,
    Video,
    Image,
    Other,
}

impl MediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Image => "image",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "document" => Ok(Self::Document),
            "audio" => Ok(Self::Audio),
            "video" => Ok(Self::Video),
            "image" => Ok(Self::Image),
            "other" => Ok(Self::Other),
            other => Err(AppError::validation(format!("未知的素材类型：{other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetStatus {
    Importing,
    Ready,
    Missing,
    Failed,
    Trashed,
}

impl AssetStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Importing => "importing",
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::Failed => "failed",
            Self::Trashed => "trashed",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "importing" => Ok(Self::Importing),
            "ready" => Ok(Self::Ready),
            "missing" => Ok(Self::Missing),
            "failed" => Ok(Self::Failed),
            "trashed" => Ok(Self::Trashed),
            other => Err(AppError::validation(format!("未知的素材状态：{other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub work_id: String,
    pub section_id: String,
    pub display_name: String,
    pub original_name: String,
    pub relative_path: String,
    pub source_path: Option<String>,
    pub extension: Option<String>,
    pub media_kind: MediaKind,
    pub size_bytes: i64,
    pub status: AssetStatus,
    pub is_favorite: bool,
    pub created_at: String,
    pub updated_at: String,
    /// 由服务层基于作品根目录 + 相对路径解析，仅用于展示与打开文件。
    pub absolute_path: String,
}

/// MIME 探测采用扩展名映射（MVP 不引入第三方嗅探库）。
pub fn mime_for_extension(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "pdf" => "application/pdf",
        "txt" | "md" | "markdown" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_kind_roundtrip() {
        for kind in [
            MediaKind::Document,
            MediaKind::Audio,
            MediaKind::Video,
            MediaKind::Image,
            MediaKind::Other,
        ] {
            assert_eq!(MediaKind::parse(kind.as_str()).unwrap(), kind);
        }
    }

    #[test]
    fn asset_status_roundtrip() {
        for status in [
            AssetStatus::Importing,
            AssetStatus::Ready,
            AssetStatus::Missing,
            AssetStatus::Failed,
            AssetStatus::Trashed,
        ] {
            assert_eq!(AssetStatus::parse(status.as_str()).unwrap(), status);
        }
    }

    #[test]
    fn mime_detection_for_previewable_images() {
        assert_eq!(mime_for_extension("PNG"), "image/png");
        assert_eq!(mime_for_extension("jpg"), "image/jpeg");
        assert_eq!(mime_for_extension("unknown"), "application/octet-stream");
    }
}
