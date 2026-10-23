use serde::{Deserialize, Serialize};

use crate::domain::asset::MediaKind;
use crate::errors::{AppError, AppResult};

/// 第一版固定的四个顶层分区。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKey {
    Documents,
    Audio,
    Video,
    Images,
}

impl SectionKey {
    pub const ALL: [SectionKey; 4] = [Self::Documents, Self::Audio, Self::Video, Self::Images];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Documents => "documents",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Images => "images",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "documents" => Ok(Self::Documents),
            "audio" => Ok(Self::Audio),
            "video" => Ok(Self::Video),
            "images" => Ok(Self::Images),
            other => Err(AppError::validation(format!("未知的分区：{other}"))),
        }
    }

    /// 磁盘上的分区目录名。
    pub fn folder_name(self) -> &'static str {
        match self {
            Self::Documents => "01_文档",
            Self::Audio => "02_音频",
            Self::Video => "03_视频",
            Self::Images => "04_图片",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Documents => "文档",
            Self::Audio => "音频",
            Self::Video => "视频",
            Self::Images => "图片",
        }
    }

    pub fn sort_order(self) -> i64 {
        match self {
            Self::Documents => 1,
            Self::Audio => 2,
            Self::Video => 3,
            Self::Images => 4,
        }
    }

    pub fn media_kind(self) -> MediaKind {
        match self {
            Self::Documents => MediaKind::Document,
            Self::Audio => MediaKind::Audio,
            Self::Video => MediaKind::Video,
            Self::Images => MediaKind::Image,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub work_id: String,
    pub key: SectionKey,
    pub name: String,
    pub relative_path: String,
    pub sort_order: i64,
    pub created_at: String,
    pub asset_count: i64,
}

/// 依据扩展名给出分区建议；无法识别时返回 None，由用户显式选择。
pub fn suggest_section_for_extension(extension: &str) -> Option<SectionKey> {
    let normalized = extension.trim_start_matches('.').to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    let section = match normalized.as_str() {
        "md" | "markdown" | "txt" | "doc" | "docx" | "pdf" | "srt" | "ass" | "rtf" | "csv"
        | "xlsx" | "xls" => SectionKey::Documents,
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "wma" | "aif" | "aiff" => {
            SectionKey::Audio
        }
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" | "wmv" | "flv" | "prproj" | "drp"
        | "veg" | "aep" => SectionKey::Video,
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "svg" | "tif" | "tiff" | "psd"
        | "heic" => SectionKey::Images,
        _ => return None,
    };
    Some(section)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_keys_are_stable() {
        assert_eq!(SectionKey::ALL.len(), 4);
        for key in SectionKey::ALL {
            assert_eq!(SectionKey::parse(key.as_str()).unwrap(), key);
            assert!(!key.folder_name().is_empty());
            assert!(key.sort_order() > 0);
        }
        assert_eq!(SectionKey::Documents.folder_name(), "01_文档");
        assert_eq!(SectionKey::Images.sort_order(), 4);
    }

    #[test]
    fn unknown_section_key_is_rejected() {
        assert!(SectionKey::parse("other").is_err());
        assert!(SectionKey::parse("Documents").is_err());
    }

    #[test]
    fn extension_mapping_covers_common_media() {
        assert_eq!(
            suggest_section_for_extension("mp4"),
            Some(SectionKey::Video)
        );
        assert_eq!(
            suggest_section_for_extension(".MP3"),
            Some(SectionKey::Audio)
        );
        assert_eq!(
            suggest_section_for_extension("png"),
            Some(SectionKey::Images)
        );
        assert_eq!(
            suggest_section_for_extension("docx"),
            Some(SectionKey::Documents)
        );
        assert_eq!(suggest_section_for_extension("exe"), None);
        assert_eq!(suggest_section_for_extension(""), None);
    }

    #[test]
    fn section_media_kind_matches_semantics() {
        assert_eq!(SectionKey::Documents.media_kind(), MediaKind::Document);
        assert_eq!(SectionKey::Audio.media_kind(), MediaKind::Audio);
        assert_eq!(SectionKey::Video.media_kind(), MediaKind::Video);
        assert_eq!(SectionKey::Images.media_kind(), MediaKind::Image);
    }
}
