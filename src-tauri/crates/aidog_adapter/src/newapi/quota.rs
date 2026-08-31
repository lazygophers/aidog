//! New API (中转平台) 余额查询。
//! 执行已统一走 registry quota 脚本（quota-scripts spec T4，`registry/platforms/newapi`，
//! 两步：token usage → unlimited 时查用户余额，`newapi_user_id` 随结果透传）；本文件
//! 保留 extra 解析纯函数供前端表单 / 导入侧复用。旧 Rust 查询实现已随 T4 移除。

use std::sync::Arc;

use aidog_db::Db;

use crate::quota::http::{err_quota, PlatformQuota};

/// 从 platform.extra JSON 解析 New API 余额配置
/// Returns (balance_base_url, balance_api_key)
pub fn parse_newapi_extra(extra: &str) -> Option<(String, String)> {
    let n = aidog_db::models::PlatformExtra::parse(extra).newapi?;
    if n.balance_api_key.is_empty() {
        return None;
    }
    Some((n.balance_base_url, n.balance_api_key))
}

/// New API 余额查询入口
/// base_url: 平台 OpenAI base_url (如 https://instance.com/v1)
/// api_key:  平台主 API key (用于 token usage 查询)
/// extra:    platform.extra JSON (含 balance_base_url + balance_api_key)
///
/// 统一脚本路径（`quota::run_quota_script`）：物化列 → 自定义脚本 → registry 选中
/// （或首条）变体；registry 缺脚本（理论上不可能，bundled 编译期内置）→ Unsupported err。
pub async fn query_quota_newapi(db: Option<&Arc<Db>>, base_url: &str, api_key: &str, extra: &str, platform_id: i64) -> PlatformQuota {
    crate::quota::run_quota_script(db, "newapi", base_url, api_key, extra, platform_id)
        .await
        .unwrap_or_else(|| err_quota(&format!("Unsupported base_url for quota query: {base_url}")))
}

#[cfg(test)]
#[path = "test_quota.rs"]
mod test_quota;
