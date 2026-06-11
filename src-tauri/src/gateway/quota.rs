//! 平台余额 & Coding Plan 配额查询服务
//!
//! 移植自 cc-switch，支持:
//!   - 余额查询: DeepSeek, StepFun, SiliconFlow, OpenRouter, Novita
//!   - Coding Plan: Kimi, GLM (智谱), MiniMax
//!
//! 对于无法实时获取的平台，前端可通过 proxy_logs 估算用量。

use serde::{Deserialize, Serialize};
use std::time::Duration;

// ── 公共类型 ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformQuota {
    pub success: bool,
    pub error: Option<String>,
    /// 查询时间 (unix millis)
    pub queried_at: i64,
    /// 余额信息 (按量计费平台)
    pub balance: Option<BalanceInfo>,
    /// Coding Plan 配额 (订阅制平台)
    pub coding_plan: Option<CodingPlanInfo>,
    /// New API: 从 /api/user/self 自动获取的用户 ID，前端可回填
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newapi_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInfo {
    /// 剩余金额
    pub remaining: f64,
    /// 总额度
    pub total: Option<f64>,
    /// 已使用
    pub used: Option<f64>,
    /// 货币单位
    pub currency: String,
    /// 账户是否可用
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingPlanInfo {
    /// 配额层级 (five_hour / weekly_limit)
    pub tiers: Vec<QuotaTier>,
    /// 套餐等级
    pub level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaTier {
    /// "five_hour" | "weekly_limit"
    pub name: String,
    /// 已用百分比 (0-100)
    pub utilization: f64,
    /// 重置时间 (ISO 8601)
    pub resets_at: Option<String>,
    /// 绝对配额上限（token 数）。仅 Kimi 等暴露绝对量的平台有值，用于精确预估基数。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    /// 绝对剩余量（token 数）。仅 Kimi 等暴露绝对量的平台有值。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining: Option<f64>,
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn millis_to_iso8601(ms: i64) -> Option<String> {
    let secs = ms / 1000;
    let nsecs = ((ms % 1000) * 1_000_000) as u32;
    chrono::DateTime::from_timestamp(secs, nsecs).map(|dt| dt.to_rfc3339())
}

fn parse_f64(value: &serde_json::Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

fn parse_f64_field(obj: &serde_json::Value, field: &str) -> Option<f64> {
    obj.get(field).and_then(parse_f64)
}

fn err_quota(msg: &str) -> PlatformQuota {
    PlatformQuota { success: false, error: Some(msg.to_string()), queried_at: now_millis(), balance: None, coding_plan: None, newapi_user_id: None }
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default()
}

// ── 余额查询: DeepSeek ───────────────────────────────────
// GET https://api.deepseek.com/user/balance

async fn query_deepseek_balance(api_key: &str) -> PlatformQuota {
    let resp = match http_client()
        .get("https://api.deepseek.com/user/balance")
        .header("Authorization", format!("Bearer {api_key}"))
        .send().await
    {
        Ok(r) => r,
        Err(e) => return err_quota(&format!("Network: {e}")),
    };
    let status = resp.status();
    if !status.is_success() {
        return err_quota(&format!("HTTP {status}"));
    }
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return err_quota(&format!("Parse: {e}")),
    };
    let is_available = body.get("is_available").and_then(|v| v.as_bool()).unwrap_or(true);
    let mut remaining = 0.0_f64;
    if let Some(infos) = body.get("balance_infos").and_then(|v| v.as_array()) {
        for info in infos {
            remaining += parse_f64_field(info, "total_balance").unwrap_or(0.0);
        }
    }
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(),
        balance: Some(BalanceInfo { remaining, total: None, used: None, currency: "CNY".into(), is_valid: is_available }),
        coding_plan: None, newapi_user_id: None,
    }
}

// ── 余额查询: StepFun ────────────────────────────────────
// GET https://api.stepfun.com/v1/accounts

async fn query_stepfun_balance(api_key: &str) -> PlatformQuota {
    let resp = match http_client()
        .get("https://api.stepfun.com/v1/accounts")
        .header("Authorization", format!("Bearer {api_key}"))
        .send().await
    {
        Ok(r) => r,
        Err(e) => return err_quota(&format!("Network: {e}")),
    };
    let status = resp.status();
    if !status.is_success() { return err_quota(&format!("HTTP {status}")); }
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return err_quota(&format!("Parse: {e}")),
    };
    let balance = parse_f64_field(&body, "balance").unwrap_or(0.0);
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(),
        balance: Some(BalanceInfo { remaining: balance, total: None, used: None, currency: "CNY".into(), is_valid: true }),
        coding_plan: None, newapi_user_id: None,
    }
}

// ── 余额查询: SiliconFlow ────────────────────────────────
// GET https://api.siliconflow.cn/v1/user/info

async fn query_siliconflow_balance(api_key: &str, is_cn: bool) -> PlatformQuota {
    let domain = if is_cn { "api.siliconflow.cn" } else { "api.siliconflow.com" };
    let url = format!("https://{domain}/v1/user/info");
    let resp = match http_client()
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send().await
    {
        Ok(r) => r,
        Err(e) => return err_quota(&format!("Network: {e}")),
    };
    let status = resp.status();
    if !status.is_success() { return err_quota(&format!("HTTP {status}")); }
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return err_quota(&format!("Parse: {e}")),
    };
    let data = match body.get("data") {
        Some(d) => d,
        None => return err_quota("Missing data field"),
    };
    let total = parse_f64_field(data, "totalBalance").unwrap_or(0.0);
    let unit = if is_cn { "CNY" } else { "USD" };
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(),
        balance: Some(BalanceInfo { remaining: total, total: None, used: None, currency: unit.into(), is_valid: true }),
        coding_plan: None, newapi_user_id: None,
    }
}

// ── 余额查询: OpenRouter ─────────────────────────────────
// GET https://openrouter.ai/api/v1/credits

async fn query_openrouter_balance(api_key: &str) -> PlatformQuota {
    let resp = match http_client()
        .get("https://openrouter.ai/api/v1/credits")
        .header("Authorization", format!("Bearer {api_key}"))
        .send().await
    {
        Ok(r) => r,
        Err(e) => return err_quota(&format!("Network: {e}")),
    };
    let status = resp.status();
    if !status.is_success() { return err_quota(&format!("HTTP {status}")); }
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return err_quota(&format!("Parse: {e}")),
    };
    let data = body.get("data").unwrap_or(&body);
    let total_credits = parse_f64_field(data, "total_credits").unwrap_or(0.0);
    let total_usage = parse_f64_field(data, "total_usage").unwrap_or(0.0);
    let remaining = total_credits - total_usage;
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(),
        balance: Some(BalanceInfo {
            remaining, total: Some(total_credits), used: Some(total_usage),
            currency: "USD".into(), is_valid: remaining > 0.0,
        }),
        coding_plan: None, newapi_user_id: None,
    }
}

// ── 余额查询: Novita AI ──────────────────────────────────
// GET https://api.novita.ai/v3/user/balance

async fn query_novita_balance(api_key: &str) -> PlatformQuota {
    let resp = match http_client()
        .get("https://api.novita.ai/v3/user/balance")
        .header("Authorization", format!("Bearer {api_key}"))
        .send().await
    {
        Ok(r) => r,
        Err(e) => return err_quota(&format!("Network: {e}")),
    };
    let status = resp.status();
    if !status.is_success() { return err_quota(&format!("HTTP {status}")); }
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return err_quota(&format!("Parse: {e}")),
    };
    // Novita 金额单位 0.0001 USD
    let available = parse_f64_field(&body, "availableBalance").unwrap_or(0.0) / 10000.0;
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(),
        balance: Some(BalanceInfo { remaining: available, total: None, used: None, currency: "USD".into(), is_valid: available > 0.0 }),
        coding_plan: None, newapi_user_id: None,
    }
}

// ── Coding Plan: Kimi ─────────────────────────────────────
// GET https://api.kimi.com/coding/v1/usages

async fn query_kimi_coding_plan(api_key: &str) -> PlatformQuota {
    let resp = match http_client()
        .get("https://api.kimi.com/coding/v1/usages")
        .header("Authorization", format!("Bearer {api_key}"))
        .send().await
    {
        Ok(r) => r,
        Err(e) => return err_quota(&format!("Network: {e}")),
    };
    let status = resp.status();
    if !status.is_success() { return err_quota(&format!("HTTP {status}")); }
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return err_quota(&format!("Parse: {e}")),
    };
    let mut tiers = Vec::new();
    // 5h 窗口
    if let Some(limits) = body.get("limits").and_then(|v| v.as_array()) {
        for item in limits {
            if let Some(detail) = item.get("detail") {
                let limit = parse_f64_field(detail, "limit").unwrap_or(1.0);
                let remaining = parse_f64_field(detail, "remaining").unwrap_or(0.0);
                let used = (limit - remaining).max(0.0);
                let utilization = if limit > 0.0 { (used / limit) * 100.0 } else { 0.0 };
                let resets_at = detail.get("resetTime").and_then(|v| {
                    v.as_str().map(String::from).or_else(|| v.as_i64().and_then(millis_to_iso8601))
                });
                // Kimi 暴露绝对 limit/remaining → 保留供精确预估基数
                tiers.push(QuotaTier { name: "five_hour".into(), utilization, resets_at, limit: Some(limit), remaining: Some(remaining) });
            }
        }
    }
    // 周限额
    if let Some(usage) = body.get("usage") {
        let limit = parse_f64_field(usage, "limit").unwrap_or(1.0);
        let remaining = parse_f64_field(usage, "remaining").unwrap_or(0.0);
        let used = (limit - remaining).max(0.0);
        let utilization = if limit > 0.0 { (used / limit) * 100.0 } else { 0.0 };
        let resets_at = usage.get("resetTime").and_then(|v| {
            v.as_str().map(String::from).or_else(|| v.as_i64().and_then(millis_to_iso8601))
        });
        tiers.push(QuotaTier { name: "weekly_limit".into(), utilization, resets_at, limit: Some(limit), remaining: Some(remaining) });
    }
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(), balance: None,
        coding_plan: Some(CodingPlanInfo { tiers, level: None }),
        newapi_user_id: None,
    }
}

// ── Coding Plan: GLM (智谱) ──────────────────────────────
// GET {base}/api/monitor/usage/quota/limit

async fn query_zhipu_coding_plan(base_url: &str, api_key: &str) -> PlatformQuota {
    let base = if base_url.to_lowercase().contains("bigmodel.cn") {
        "https://open.bigmodel.cn"
    } else {
        "https://api.z.ai"
    };
    let url = format!("{base}/api/monitor/usage/quota/limit");
    let resp = match http_client()
        .get(&url)
        .header("Authorization", api_key) // 智谱不加 Bearer
        .header("Content-Type", "application/json")
        .send().await
    {
        Ok(r) => r,
        Err(e) => return err_quota(&format!("Network: {e}")),
    };
    let status = resp.status();
    if !status.is_success() { return err_quota(&format!("HTTP {status}")); }
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return err_quota(&format!("Parse: {e}")),
    };
    if body.get("success").and_then(|v| v.as_bool()) == Some(false) {
        let msg = body.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown");
        return err_quota(msg);
    }
    let data = match body.get("data") {
        Some(d) => d,
        None => return err_quota("Missing data field"),
    };
    let level = data.get("level").and_then(|v| v.as_str()).map(String::from);
    // 解析 TOKENS_LIMIT 条目
    let mut raw_limits: Vec<(Option<i64>, f64, Option<String>)> = Vec::new();
    if let Some(limits) = data.get("limits").and_then(|v| v.as_array()) {
        for item in limits {
            let limit_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if !limit_type.eq_ignore_ascii_case("TOKENS_LIMIT") { continue; }
            let pct = item.get("percentage").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let reset_ms = item.get("nextResetTime").and_then(|v| v.as_i64());
            let reset_iso = reset_ms.and_then(millis_to_iso8601);
            raw_limits.push((reset_ms, pct, reset_iso));
        }
    }
    raw_limits.sort_by_key(|(reset, _, _)| (reset.is_some(), reset.unwrap_or(i64::MIN)));
    let tiers: Vec<QuotaTier> = raw_limits.into_iter().enumerate().filter_map(|(idx, (_, pct, resets_at))| {
        let name = match idx { 0 => "five_hour", 1 => "weekly_limit", _ => return None };
        // GLM 上游仅给百分比，无绝对基数 → 方案 B 拟合
        Some(QuotaTier { name: name.into(), utilization: pct, resets_at, limit: None, remaining: None })
    }).collect();
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(), balance: None,
        coding_plan: Some(CodingPlanInfo { tiers, level }),
        newapi_user_id: None,
    }
}

// ── Coding Plan: MiniMax ─────────────────────────────────
// GET https://api.minimaxi.com/v1/api/openplatform/coding_plan/remains

async fn query_minimax_coding_plan(api_key: &str, is_cn: bool) -> PlatformQuota {
    let domain = if is_cn { "api.minimaxi.com" } else { "api.minimax.io" };
    let url = format!("https://{domain}/v1/api/openplatform/coding_plan/remains");
    let resp = match http_client()
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send().await
    {
        Ok(r) => r,
        Err(e) => return err_quota(&format!("Network: {e}")),
    };
    let status = resp.status();
    if !status.is_success() { return err_quota(&format!("HTTP {status}")); }
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return err_quota(&format!("Parse: {e}")),
    };
    if let Some(base_resp) = body.get("base_resp") {
        let code = base_resp.get("status_code").and_then(|v| v.as_i64()).unwrap_or(-1);
        if code != 0 {
            let msg = base_resp.get("status_msg").and_then(|v| v.as_str()).unwrap_or("Unknown");
            return err_quota(&format!("API error (code {code}): {msg}"));
        }
    }
    let mut tiers = Vec::new();
    if let Some(model_remains) = body.get("model_remains").and_then(|v| v.as_array()) {
        let item = model_remains.iter().find(|i| {
            i.get("model_name").and_then(|v| v.as_str()).map(|s| s == "general").unwrap_or(false)
        });
        if let Some(item) = item {
            // 5h 桶
            if let Some(remain_pct) = item.get("current_interval_remaining_percent").and_then(|v| v.as_f64()) {
                let resets_at = item.get("end_time").and_then(|v| v.as_i64()).and_then(millis_to_iso8601);
                tiers.push(QuotaTier { name: "five_hour".into(), utilization: 100.0 - remain_pct, resets_at, limit: None, remaining: None });
            }
            // 周桶 (仅 status=1)
            if item.get("current_weekly_status").and_then(|v| v.as_i64()) == Some(1) {
                if let Some(remain_pct) = item.get("current_weekly_remaining_percent").and_then(|v| v.as_f64()) {
                    let resets_at = item.get("weekly_end_time").and_then(|v| v.as_i64()).and_then(millis_to_iso8601);
                    tiers.push(QuotaTier { name: "weekly_limit".into(), utilization: 100.0 - remain_pct, resets_at, limit: None, remaining: None });
                }
            }
        }
    }
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(), balance: None,
        coding_plan: Some(CodingPlanInfo { tiers, level: None }),
        newapi_user_id: None,
    }
}

// ── 公开入口 ──────────────────────────────────────────────

/// 根据 base_url 自动检测平台并查询余额或 Coding Plan 配额
pub async fn query_quota(base_url: &str, api_key: &str) -> PlatformQuota {
    if api_key.trim().is_empty() {
        return err_quota("API key is empty");
    }
    let url = base_url.to_lowercase();

    // Coding Plan 查询 (优先检测，这些平台通常同时有 Coding Plan)
    if url.contains("api.kimi.com/coding") {
        return query_kimi_coding_plan(api_key).await;
    }
    if url.contains("open.bigmodel.cn") || url.contains("bigmodel.cn") {
        // GLM 可能同时返回 coding plan
        let quota = query_zhipu_coding_plan(base_url, api_key).await;
        return quota;
    }
    if url.contains("api.z.ai") {
        return query_zhipu_coding_plan(base_url, api_key).await;
    }
    if url.contains("api.minimaxi.com") {
        return query_minimax_coding_plan(api_key, true).await;
    }
    if url.contains("api.minimax.io") {
        return query_minimax_coding_plan(api_key, false).await;
    }

    // 余额查询
    if url.contains("api.deepseek.com") {
        return query_deepseek_balance(api_key).await;
    }
    if url.contains("api.stepfun.com") || url.contains("api.stepfun.ai") {
        return query_stepfun_balance(api_key).await;
    }
    if url.contains("api.siliconflow.cn") {
        return query_siliconflow_balance(api_key, true).await;
    }
    if url.contains("api.siliconflow.com") {
        return query_siliconflow_balance(api_key, false).await;
    }
    if url.contains("openrouter.ai") {
        return query_openrouter_balance(api_key).await;
    }
    if url.contains("api.novita.ai") {
        return query_novita_balance(api_key).await;
    }

    // 不支持的平台
    err_quota("Unsupported platform for quota query")
}

// ── New API (中转平台) ──────────────────────────────────────
// 两步余额查询:
//   Step 1: GET {instance}/api/usage/token/ (用 api_key) → unlimited_quota?
//   Step 2a (unlimited): GET {balance_base_url}/api/user/self (用 balance_api_key) → 用户余额
//   Step 2b (limited):   直接用 token 的 total_available ÷ 500000 = USD
// 内部单位 ÷ 500000 = USD

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
async fn query_token_usage(base_url: &str, api_key: &str) -> Result<(bool, f64, f64, f64), String> {
    let root = newapi_instance_root(base_url);
    let url = format!("{}/api/usage/token/", root);
    let resp = http_client()
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send().await
        .map_err(|e| format!("Network: {e}"))?;
    let status = resp.status();
    if !status.is_success() { return Err(format!("HTTP {status}")); }
    let body: serde_json::Value = resp.json().await
        .map_err(|e| format!("Parse: {e}"))?;
    let data = body.get("data").ok_or("Missing data field")?;
    let unlimited = data.get("unlimited_quota").and_then(|v| v.as_bool()).unwrap_or(false);
    let total_granted = parse_f64_field(data, "total_granted").unwrap_or(0.0);
    let total_used = parse_f64_field(data, "total_used").unwrap_or(0.0);
    let total_available = parse_f64_field(data, "total_available").unwrap_or(0.0);
    Ok((unlimited, total_granted, total_used, total_available))
}

/// Step 2a: unlimited token → 查用户余额 GET /api/user/self
async fn query_newapi_user_balance(balance_base_url: &str, balance_api_key: &str) -> PlatformQuota {
    let url = format!("{}/api/user/self", balance_base_url.trim_end_matches('/'));
    let resp = match http_client()
        .get(&url)
        .header("Authorization", format!("Bearer {balance_api_key}"))
        .header("Content-Type", "application/json")
        .send().await
    {
        Ok(r) => r,
        Err(e) => return err_quota(&format!("Network: {e}")),
    };
    let status = resp.status();
    if !status.is_success() { return err_quota(&format!("HTTP {status}")); }
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return err_quota(&format!("Parse: {e}")),
    };
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        let msg = body.get("message").and_then(|v| v.as_str()).unwrap_or("Query failed");
        return err_quota(msg);
    }
    let data = match body.get("data") {
        Some(d) => d,
        None => return err_quota("Missing data field"),
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
pub async fn query_quota_newapi(base_url: &str, api_key: &str, extra: &str) -> PlatformQuota {
    if api_key.trim().is_empty() {
        return err_quota("New API: api_key required for token usage query");
    }

    // Step 1: 查询 token 使用情况
    let (unlimited, total_granted, total_used, total_available) = match query_token_usage(base_url, api_key).await {
        Ok(info) => info,
        Err(e) => return err_quota(&format!("Token usage: {e}")),
    };

    if unlimited {
        // Step 2a: 不限额 → 查用户余额
        match parse_newapi_extra(extra) {
            Some((balance_base_url, balance_api_key)) => {
                if balance_base_url.is_empty() {
                    return err_quota("New API: unlimited token requires balance_base_url");
                }
                query_newapi_user_balance(&balance_base_url, &balance_api_key).await
            }
            None => err_quota("New API: unlimited token requires balance_api_key in config"),
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
