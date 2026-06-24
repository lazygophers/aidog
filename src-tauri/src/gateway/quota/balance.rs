//! 余额查询: DeepSeek / StepFun / SiliconFlow / OpenRouter / Novita。
//!
//! 每平台拆为 HTTP 壳 (`query_X_balance`) + 纯解析 (`parse_X_balance`)。
//! 纯解析吃 JSON body 出 PlatformQuota，不触网，可直接单测。

use std::sync::Arc;

use crate::gateway::db::Db;

use super::http::{
    err_quota_platform, now_millis, parse_f64_field, quota_get_json, BalanceInfo, PlatformQuota,
};

fn balance_ok(balance: BalanceInfo) -> PlatformQuota {
    PlatformQuota {
        success: true,
        error: None,
        queried_at: now_millis(),
        balance: Some(balance),
        coding_plan: None,
        newapi_user_id: None,
    }
}

// ── 余额查询: DeepSeek ───────────────────────────────────
// GET https://api.deepseek.com/user/balance

pub(crate) async fn query_deepseek_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(
        db,
        "https://api.deepseek.com/user/balance",
        &[("Authorization", format!("Bearer {api_key}"))],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("deepseek", &e),
    };
    parse_deepseek_balance(&body)
}

pub(crate) fn parse_deepseek_balance(body: &serde_json::Value) -> PlatformQuota {
    let is_available = body
        .get("is_available")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let mut remaining = 0.0_f64;
    if let Some(infos) = body.get("balance_infos").and_then(|v| v.as_array()) {
        for info in infos {
            remaining += parse_f64_field(info, "total_balance").unwrap_or(0.0);
        }
    }
    balance_ok(BalanceInfo {
        remaining,
        total: None,
        used: None,
        currency: "CNY".into(),
        is_valid: is_available,
    })
}

// ── 余额查询: StepFun ────────────────────────────────────
// GET https://api.stepfun.com/v1/accounts

pub(crate) async fn query_stepfun_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(
        db,
        "https://api.stepfun.com/v1/accounts",
        &[("Authorization", format!("Bearer {api_key}"))],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("stepfun", &e),
    };
    parse_stepfun_balance(&body)
}

pub(crate) fn parse_stepfun_balance(body: &serde_json::Value) -> PlatformQuota {
    let balance = parse_f64_field(body, "balance").unwrap_or(0.0);
    balance_ok(BalanceInfo {
        remaining: balance,
        total: None,
        used: None,
        currency: "CNY".into(),
        is_valid: true,
    })
}

// ── 余额查询: SiliconFlow ────────────────────────────────
// GET https://api.siliconflow.cn/v1/user/info

pub(crate) async fn query_siliconflow_balance(
    db: Option<&Arc<Db>>,
    api_key: &str,
    is_cn: bool,
) -> PlatformQuota {
    let domain = if is_cn {
        "api.siliconflow.cn"
    } else {
        "api.siliconflow.com"
    };
    let url = format!("https://{domain}/v1/user/info");
    let body = match quota_get_json(db, &url, &[("Authorization", format!("Bearer {api_key}"))]).await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("siliconflow", &e),
    };
    parse_siliconflow_balance(&body, is_cn)
}

pub(crate) fn parse_siliconflow_balance(body: &serde_json::Value, is_cn: bool) -> PlatformQuota {
    let data = match body.get("data") {
        Some(d) => d,
        None => return err_quota_platform("siliconflow", "Missing data field"),
    };
    let total = parse_f64_field(data, "totalBalance").unwrap_or(0.0);
    let unit = if is_cn { "CNY" } else { "USD" };
    balance_ok(BalanceInfo {
        remaining: total,
        total: None,
        used: None,
        currency: unit.into(),
        is_valid: true,
    })
}

// ── 余额查询: OpenRouter ─────────────────────────────────
// GET https://openrouter.ai/api/v1/credits

pub(crate) async fn query_openrouter_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(
        db,
        "https://openrouter.ai/api/v1/credits",
        &[("Authorization", format!("Bearer {api_key}"))],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("openrouter", &e),
    };
    parse_openrouter_balance(&body)
}

pub(crate) fn parse_openrouter_balance(body: &serde_json::Value) -> PlatformQuota {
    let data = body.get("data").unwrap_or(body);
    let total_credits = parse_f64_field(data, "total_credits").unwrap_or(0.0);
    let total_usage = parse_f64_field(data, "total_usage").unwrap_or(0.0);
    let remaining = total_credits - total_usage;
    balance_ok(BalanceInfo {
        remaining,
        total: Some(total_credits),
        used: Some(total_usage),
        currency: "USD".into(),
        is_valid: remaining > 0.0,
    })
}

// ── 余额查询: Novita AI ──────────────────────────────────
// GET https://api.novita.ai/v3/user/balance

pub(crate) async fn query_novita_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(
        db,
        "https://api.novita.ai/v3/user/balance",
        &[("Authorization", format!("Bearer {api_key}"))],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("novita", &e),
    };
    parse_novita_balance(&body)
}

pub(crate) fn parse_novita_balance(body: &serde_json::Value) -> PlatformQuota {
    // Novita 金额单位 0.0001 USD
    let available = parse_f64_field(body, "availableBalance").unwrap_or(0.0) / 10000.0;
    balance_ok(BalanceInfo {
        remaining: available,
        total: None,
        used: None,
        currency: "USD".into(),
        is_valid: available > 0.0,
    })
}

#[cfg(test)]
#[path = "test_balance.rs"]
mod test_balance;
