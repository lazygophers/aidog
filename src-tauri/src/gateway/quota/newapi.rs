//! New API (中转平台) 两步余额查询。
//!   Step 1: GET {instance}/api/usage/token/ (用 api_key) → unlimited_quota?
//!   Step 2a (unlimited): GET {balance_base_url}/api/user/self (用 balance_api_key) → 用户余额
//!   Step 2b (limited):   直接用 token 的 total_available ÷ 500000 = USD
//! 内部单位 ÷ 500000 = USD

use std::sync::Arc;

use crate::gateway::db::Db;

use super::http::{
    err_quota, err_quota_platform, now_millis, parse_f64_field, quota_get_json, BalanceInfo,
    PlatformQuota, QUOTA_PLATFORM_ID,
};

/// 从 base_url 去掉版本后缀（/v1, /v2 等）获取实例根 URL
fn newapi_instance_root(base_url: &str) -> String {
    let url = base_url.trim_end_matches('/');
    if let Some(pos) = url.rfind('/') {
        let last = &url[pos + 1..];
        if last.len() > 1 && last.starts_with('v') && last[1..].chars().all(|c| c.is_ascii_digit()) {
            return url[..pos].to_string();
        }
    }
    url.to_string()
}

/// 从 platform.extra JSON 解析 New API 余额配置
/// Returns (balance_base_url, balance_api_key)
pub fn parse_newapi_extra(extra: &str) -> Option<(String, String)> {
    if extra.trim().is_empty() { return None; }
    let obj: serde_json::Value = serde_json::from_str(extra).ok()?;
    let newapi = obj.get("newapi")?;
    let base_url = newapi.get("balance_base_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let key = newapi.get("balance_api_key").and_then(|v| v.as_str())?.to_string();
    if key.is_empty() { return None; }
    Some((base_url, key))
}

/// Step 1: 用 api_key 查询 token 使用情况
/// GET {instance_root}/api/usage/token/
/// Response: { data: { unlimited_quota, total_granted, total_used, total_available } }
async fn query_token_usage(db: Option<&Arc<Db>>, base_url: &str, api_key: &str) -> Result<(bool, f64, f64, f64), String> {
    let root = newapi_instance_root(base_url);
    let url = format!("{}/api/usage/token/", root);
    let body = quota_get_json(db, &url,
        &[("Authorization", format!("Bearer {api_key}"))]).await?;
    let data = body.get("data").ok_or("Missing data field")?;
    let unlimited = data.get("unlimited_quota").and_then(|v| v.as_bool()).unwrap_or(false);
    let total_granted = parse_f64_field(data, "total_granted").unwrap_or(0.0);
    let total_used = parse_f64_field(data, "total_used").unwrap_or(0.0);
    let total_available = parse_f64_field(data, "total_available").unwrap_or(0.0);
    Ok((unlimited, total_granted, total_used, total_available))
}

/// Step 2a: unlimited token → 查用户余额 GET /api/user/self
async fn query_newapi_user_balance(db: Option<&Arc<Db>>, balance_base_url: &str, balance_api_key: &str) -> PlatformQuota {
    let url = format!("{}/api/user/self", balance_base_url.trim_end_matches('/'));
    let body = match quota_get_json(db, &url, &[
        ("Authorization", format!("Bearer {balance_api_key}")),
        ("Content-Type", "application/json".to_string()),
    ]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("newapi", &e),
    };
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        let msg = body.get("message").and_then(|v| v.as_str()).unwrap_or("Query failed");
        return err_quota_platform("newapi", msg);
    }
    let data = match body.get("data") {
        Some(d) => d,
        None => return err_quota_platform("newapi", "Missing data field"),
    };

    let user_id = data.get("id").and_then(|v| {
        v.as_i64().map(|i| i.to_string()).or_else(|| v.as_str().map(String::from))
    });

    // quota ÷ 500000 = USD
    let quota = parse_f64_field(data, "quota").unwrap_or(0.0);
    let used_quota = parse_f64_field(data, "used_quota").unwrap_or(0.0);
    let remaining = quota / 500000.0;
    let used = used_quota / 500000.0;
    let total = remaining + used;

    PlatformQuota {
        success: true, error: None, queried_at: now_millis(),
        balance: Some(BalanceInfo {
            remaining,
            total: Some(total),
            used: Some(used),
            currency: "USD".into(),
            is_valid: remaining > 0.0,
        }),
        coding_plan: None,
        newapi_user_id: user_id,
    }
}

/// New API 余额查询入口
/// base_url: 平台 OpenAI base_url (如 https://instance.com/v1)
/// api_key:  平台主 API key (用于 token usage 查询)
/// extra:    platform.extra JSON (含 balance_base_url + balance_api_key)
pub async fn query_quota_newapi(db: Option<&Arc<Db>>, base_url: &str, api_key: &str, extra: &str, platform_id: i64) -> PlatformQuota {
    QUOTA_PLATFORM_ID.scope(platform_id, query_quota_newapi_inner(db, base_url, api_key, extra)).await
}

async fn query_quota_newapi_inner(db: Option<&Arc<Db>>, base_url: &str, api_key: &str, extra: &str) -> PlatformQuota {
    if api_key.trim().is_empty() {
        return err_quota("New API: api_key required for token usage query");
    }

    // Step 1: 查询 token 使用情况
    let (unlimited, total_granted, total_used, total_available) = match query_token_usage(db, base_url, api_key).await {
        Ok(info) => info,
        Err(e) => return err_quota_platform("newapi", &format!("Token usage: {e}")),
    };

    if unlimited {
        // Step 2a: 不限额 → 查用户余额
        match parse_newapi_extra(extra) {
            Some((balance_base_url, balance_api_key)) => {
                if balance_base_url.is_empty() {
                    return err_quota_platform("newapi", "New API: unlimited token requires balance_base_url");
                }
                query_newapi_user_balance(db, &balance_base_url, &balance_api_key).await
            }
            None => err_quota_platform("newapi", "New API: unlimited token requires balance_api_key in config"),
        }
    } else {
        // Step 2b: 有限额 → 直接用 token 配额
        let remaining = total_available / 500000.0;
        let used = total_used / 500000.0;
        let total = total_granted / 500000.0;
        PlatformQuota {
            success: true, error: None, queried_at: now_millis(),
            balance: Some(BalanceInfo {
                remaining,
                total: if total > 0.0 { Some(total) } else { None },
                used: if used > 0.0 { Some(used) } else { None },
                currency: "USD".into(),
                is_valid: remaining > 0.0,
            }),
            coding_plan: None,
            newapi_user_id: None,
        }
    }
}
