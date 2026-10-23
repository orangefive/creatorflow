use serde::{Deserialize, Serialize};

use crate::errors::{AppError, AppResult};

pub const MAX_WORK_NAME_LENGTH: usize = 80;

const INVALID_NAME_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

const RESERVED_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Idea,
    Scripting,
    Collecting,
    InProduction,
    Review,
    Published,
    Archived,
    Creating,
    Error,
}

impl WorkStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Scripting => "scripting",
            Self::Collecting => "collecting",
            Self::InProduction => "in_production",
            Self::Review => "review",
            Self::Published => "published",
            Self::Archived => "archived",
            Self::Creating => "creating",
            Self::Error => "error",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "idea" => Ok(Self::Idea),
            "scripting" => Ok(Self::Scripting),
            "collecting" => Ok(Self::Collecting),
            "in_production" => Ok(Self::InProduction),
            "review" => Ok(Self::Review),
            "published" => Ok(Self::Published),
            "archived" => Ok(Self::Archived),
            "creating" => Ok(Self::Creating),
            "error" => Ok(Self::Error),
            other => Err(AppError::validation(format!("未知的作品状态：{other}"))),
        }
    }

    /// 用户可在界面上直接切换的状态；`creating`/`error`/`archived` 由系统流程控制。
    pub fn is_user_selectable(self) -> bool {
        matches!(
            self,
            Self::Idea
                | Self::Scripting
                | Self::Collecting
                | Self::InProduction
                | Self::Review
                | Self::Published
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Work {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: WorkStatus,
    pub root_path: String,
    pub template_key: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
    pub asset_count: i64,
}

/// 作品目录识别文件，写入作品根目录，不包含密钥与素材索引。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkManifest {
    pub schema_version: u32,
    pub work_id: String,
    pub name: String,
    pub template_key: Option<String>,
    pub created_at: String,
}

impl WorkManifest {
    pub fn new(work_id: &str, name: &str, template_key: Option<&str>, created_at: &str) -> Self {
        Self {
            schema_version: 1,
            work_id: work_id.to_string(),
            name: name.to_string(),
            template_key: template_key.map(str::to_string),
            created_at: created_at.to_string(),
        }
    }
}

/// 校验作品名称：空值、首尾空格、Windows 非法字符、保留名称、长度与结尾句点。
pub fn validate_work_name(raw: &str) -> AppResult<String> {
    if raw.trim().is_empty() {
        return Err(AppError::validation("作品名称不能为空"));
    }
    if raw.trim() != raw {
        return Err(AppError::validation("作品名称首尾不能包含空格"));
    }
    if raw.chars().count() > MAX_WORK_NAME_LENGTH {
        return Err(AppError::validation(format!(
            "作品名称不能超过 {MAX_WORK_NAME_LENGTH} 个字符"
        )));
    }
    if raw.chars().any(|ch| ch.is_control()) {
        return Err(AppError::validation("作品名称不能包含控制字符"));
    }
    if raw.chars().any(|ch| INVALID_NAME_CHARS.contains(&ch)) {
        return Err(AppError::validation(
            "作品名称不能包含 \\ / : * ? \" < > | 等字符",
        ));
    }
    if raw == "." || raw == ".." {
        return Err(AppError::validation("作品名称不能是 . 或 .."));
    }
    if raw.ends_with('.') {
        return Err(AppError::validation("作品名称不能以句点结尾"));
    }
    let stem = raw
        .split('.')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if RESERVED_NAMES.contains(&stem.as_str()) {
        return Err(AppError::validation(format!(
            "“{}”是 Windows 保留名称，请换一个名称",
            stem.to_ascii_uppercase()
        )));
    }
    Ok(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_names() {
        for name in ["我的第一支视频", "Podcast Ep.01", "直播切片-2026", "a"] {
            assert!(validate_work_name(name).is_ok(), "应接受：{name}");
        }
    }

    #[test]
    fn rejects_empty_and_blank_names() {
        assert!(validate_work_name("").is_err());
        assert!(validate_work_name("   ").is_err());
    }

    #[test]
    fn rejects_surrounding_spaces() {
        let error = validate_work_name(" 视频 ").unwrap_err();
        assert!(error.message.contains("空格"));
    }

    #[test]
    fn rejects_windows_invalid_characters() {
        for name in [
            "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b",
        ] {
            assert!(validate_work_name(name).is_err(), "应拒绝：{name}");
        }
    }

    #[test]
    fn rejects_reserved_device_names() {
        assert!(validate_work_name("CON").is_err());
        assert!(validate_work_name("nul.txt").is_err());
        assert!(validate_work_name("Lpt1").is_err());
    }

    #[test]
    fn rejects_trailing_dot_and_too_long_names() {
        assert!(validate_work_name("视频.").is_err());
        assert!(validate_work_name(&"长".repeat(MAX_WORK_NAME_LENGTH + 1)).is_err());
        assert!(validate_work_name(&"长".repeat(MAX_WORK_NAME_LENGTH)).is_ok());
    }

    #[test]
    fn status_roundtrip() {
        for status in [
            WorkStatus::Idea,
            WorkStatus::Scripting,
            WorkStatus::Collecting,
            WorkStatus::InProduction,
            WorkStatus::Review,
            WorkStatus::Published,
            WorkStatus::Archived,
        ] {
            assert_eq!(WorkStatus::parse(status.as_str()).unwrap(), status);
        }
        assert!(WorkStatus::parse("unknown").is_err());
    }

    #[test]
    fn only_user_facing_statuses_are_selectable() {
        assert!(!WorkStatus::Creating.is_user_selectable());
        assert!(!WorkStatus::Error.is_user_selectable());
        assert!(!WorkStatus::Archived.is_user_selectable());
        assert!(WorkStatus::InProduction.is_user_selectable());
    }
}
