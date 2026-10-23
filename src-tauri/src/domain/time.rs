use chrono::{SecondsFormat, Utc};

/// 统一的时间表示：UTC ISO-8601，毫秒精度，带 `Z` 后缀。
pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
