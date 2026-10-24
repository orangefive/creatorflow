use serde::Serialize;

use crate::domain::asset::Asset;

/// 重新定位素材的检查结果。
///
/// `applied = false` 表示需要用户确认（大小或哈希与记录不一致），
/// 此时数据库记录不会被修改。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelocationResult {
    pub applied: bool,
    pub requires_confirmation: bool,
    pub size_matches: bool,
    pub hash_matches: Option<bool>,
    pub relative_path: String,
    pub message: String,
    pub asset: Option<Asset>,
}
