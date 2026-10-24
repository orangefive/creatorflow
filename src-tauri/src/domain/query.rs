use serde::{Deserialize, Serialize};

use crate::domain::asset::{Asset, AssetStatus, MediaKind};
use crate::domain::section::SectionKey;
use crate::errors::{AppError, AppResult};

pub const DEFAULT_PAGE_SIZE: i64 = 50;
pub const MAX_PAGE_SIZE: i64 = 200;
const MAX_KEYWORD_LENGTH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetSortKey {
    Name,
    CreatedAt,
    Size,
    Duration,
}

impl AssetSortKey {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::CreatedAt => "created_at",
            Self::Size => "size",
            Self::Duration => "duration",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}

/// 前端传入的素材查询参数。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetQuery {
    pub work_id: Option<String>,
    pub section_key: Option<SectionKey>,
    pub media_kind: Option<MediaKind>,
    pub status: Option<AssetStatus>,
    pub tag_id: Option<String>,
    pub keyword: Option<String>,
    pub sort_key: Option<AssetSortKey>,
    pub sort_direction: Option<SortDirection>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// 校验并补全默认值后的查询参数，repository 只接受该结构。
#[derive(Debug, Clone)]
pub struct NormalizedAssetQuery {
    pub work_id: Option<String>,
    pub section_key: Option<SectionKey>,
    pub media_kind: Option<MediaKind>,
    pub status: Option<AssetStatus>,
    pub tag_id: Option<String>,
    pub keyword: Option<String>,
    pub sort_key: AssetSortKey,
    pub descending: bool,
    pub limit: i64,
    pub offset: i64,
}

impl AssetQuery {
    pub fn normalize(&self) -> AppResult<NormalizedAssetQuery> {
        let keyword = self
            .keyword
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        if let Some(keyword) = &keyword {
            if keyword.chars().count() > MAX_KEYWORD_LENGTH {
                return Err(AppError::validation(format!(
                    "搜索关键词不能超过 {MAX_KEYWORD_LENGTH} 个字符"
                )));
            }
        }

        let work_id = self
            .work_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        let tag_id = self
            .tag_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        let limit = self
            .limit
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = self.offset.unwrap_or(0).max(0);

        Ok(NormalizedAssetQuery {
            work_id,
            section_key: self.section_key,
            media_kind: self.media_kind,
            status: self.status,
            tag_id,
            keyword,
            sort_key: self.sort_key.unwrap_or(AssetSortKey::CreatedAt),
            descending: !matches!(self.sort_direction, Some(SortDirection::Asc)),
            limit,
            offset,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPage {
    pub items: Vec<Asset>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_query() -> AssetQuery {
        AssetQuery {
            work_id: None,
            section_key: None,
            media_kind: None,
            status: None,
            tag_id: None,
            keyword: None,
            sort_key: None,
            sort_direction: None,
            limit: None,
            offset: None,
        }
    }

    #[test]
    fn applies_defaults() {
        let query = base_query().normalize().unwrap();
        assert_eq!(query.limit, DEFAULT_PAGE_SIZE);
        assert_eq!(query.offset, 0);
        assert_eq!(query.sort_key, AssetSortKey::CreatedAt);
        assert!(query.descending);
        assert!(query.keyword.is_none());
    }

    #[test]
    fn clamps_page_size_and_offset() {
        let mut input = base_query();
        input.limit = Some(10_000);
        input.offset = Some(-5);
        let query = input.normalize().unwrap();
        assert_eq!(query.limit, MAX_PAGE_SIZE);
        assert_eq!(query.offset, 0);
    }

    #[test]
    fn trims_keyword_and_sorts_ascending_when_requested() {
        let mut input = base_query();
        input.keyword = Some("  脚本  ".to_string());
        input.sort_direction = Some(SortDirection::Asc);
        input.sort_key = Some(AssetSortKey::Name);
        let query = input.normalize().unwrap();
        assert_eq!(query.keyword.as_deref(), Some("脚本"));
        assert!(!query.descending);
        assert_eq!(query.sort_key, AssetSortKey::Name);
    }

    #[test]
    fn rejects_overlong_keyword() {
        let mut input = base_query();
        input.keyword = Some("长".repeat(MAX_KEYWORD_LENGTH + 1));
        assert!(input.normalize().is_err());
    }
}
