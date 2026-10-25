use serde::{Deserialize, Serialize};

use crate::domain::section::SectionKey;
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

/// 元数据提取状态；不可提取不影响素材本身可用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataStatus {
    Pending,
    Ready,
    Partial,
    Failed,
    Unsupported,
}

impl MetadataStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "ready" => Ok(Self::Ready),
            "partial" => Ok(Self::Partial),
            "failed" => Ok(Self::Failed),
            "unsupported" => Ok(Self::Unsupported),
            other => Err(AppError::validation(format!("未知的元数据状态：{other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbnailStatus {
    Pending,
    Ready,
    Failed,
    Unsupported,
}

impl ThumbnailStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            "unsupported" => Ok(Self::Unsupported),
            other => Err(AppError::validation(format!("未知的缩略图状态：{other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub asset_count: i64,
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
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub frame_rate: Option<f64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub codec: Option<String>,
    pub sha256: Option<String>,
    pub metadata_status: MetadataStatus,
    pub metadata_error: Option<String>,
    pub thumbnail_status: ThumbnailStatus,
    pub thumbnail_relative_path: Option<String>,
    /// 缓存目录中的绝对路径，由服务层拼接，用于前端 asset 协议显示
    pub thumbnail_path: Option<String>,
    pub thumbnail_error: Option<String>,
    pub tags: Vec<Tag>,
    /// 跨作品查询时用于标识所属作品；单作品查询同样填充
    pub work_name: Option<String>,
    /// 素材所属分区（documents/audio/video/images）
    pub section_key: Option<SectionKey>,
}

impl Asset {
    pub fn has_metadata(&self) -> bool {
        self.width.is_some() || self.height.is_some() || self.duration_ms.is_some()
    }
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
    fn metadata_and_thumbnail_status_roundtrip() {
        for status in [
            MetadataStatus::Pending,
            MetadataStatus::Ready,
            MetadataStatus::Partial,
            MetadataStatus::Failed,
            MetadataStatus::Unsupported,
        ] {
            assert_eq!(MetadataStatus::parse(status.as_str()).unwrap(), status);
        }
        for status in [
            ThumbnailStatus::Pending,
            ThumbnailStatus::Ready,
            ThumbnailStatus::Failed,
            ThumbnailStatus::Unsupported,
        ] {
            assert_eq!(ThumbnailStatus::parse(status.as_str()).unwrap(), status);
        }
    }

    #[test]
    fn mime_detection_for_previewable_images() {
        assert_eq!(mime_for_extension("PNG"), "image/png");
        assert_eq!(mime_for_extension("jpg"), "image/jpeg");
        assert_eq!(mime_for_extension("unknown"), "application/octet-stream");
    }
}
