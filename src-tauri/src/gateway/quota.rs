//! 平台余额 & Coding Plan 配额查询服务
//!
//! 移植自 cc-switch，支持:
//!   - 余额查询: DeepSeek, StepFun, SiliconFlow, OpenRouter, Novita
//!   - Coding Plan: Kimi, GLM (智谱), MiniMax
//!
//! 对于无法实时获取的平台，前端可通过 proxy_logs 估算用量。

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use super::db::Db;

// 当前 quota 查询归属的平台 ID。
// query_quota / query_quota_newapi 进入时 scope 设定；quota_get_json 单点落库时读取，
// 避免沿 10 个 provider 函数链逐层透传 platform_id 签名。未设（如裸调测试）→ 0。
tokio::task_local! {
    pub(crate) static QUOTA_PLATFORM_ID: i64;
}

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
    tracing::warn!(error = %msg, "quota query failed");
    PlatformQuota { success: false, error: Some(msg.to_string()), queried_at: now_millis(), balance: None, coding_plan: None, newapi_user_id: None }
}

/// 同 err_quota，但附带平台标识，供排障定位是哪个平台查询失败。
fn err_quota_platform(platform: &str, msg: &str) -> PlatformQuota {
    tracing::warn!(platform = %platform, error = %msg, "quota query failed");
    PlatformQuota { success: false, error: Some(msg.to_string()), queried_at: now_millis(), balance: None, coding_plan: None, newapi_user_id: None }
}

async fn http_client(db: Option<&Arc<Db>>) -> reqwest::Client {
    match db {
        Some(db) => super::http_client::build_http_client_system(db, 10, 5).await,
        None => reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default(),
    }
}

/// 统一 quota 出站 GET: 记录请求 path (info 级) + 响应体 (debug 级), 返回解析后的 JSON。
/// headers 原样设置 (调用方决定是否加 Bearer 前缀)。
/// 错误前缀保持与各 func 原行为一致: Network / HTTP {status} / Parse。
/// 所有 quota 出站 HTTP 经此单点, 落 proxy_log (source_protocol="quota"), 与 fetch_models/model_test 同模式。
async fn quota_get_json(
    db: Option<&Arc<Db>>,
    url: &str,
    headers: &[(&str, String)],
) -> Result<serde_json::Value, String> {
    tracing::info!(method = "GET", url = %url, "quota outbound request");
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let created_at = super::db::now();

    let mut rb = http_client(db).await.get(url);
    for (k, v) in headers {
        rb = rb.header(*k, v);
    }
    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("Network: {e}");
            persist_quota_log(db, make_quota_log(&request_id, url, 0, &msg, start.elapsed().as_millis() as i32, created_at)).await;
            return Err(msg);
        }
    };
    let status = resp.status();
    let upstream_status = status.as_u16() as i32;
    if !status.is_success() {
        let msg = format!("HTTP {status}");
        persist_quota_log(db, make_quota_log(&request_id, url, upstream_status, &msg, start.elapsed().as_millis() as i32, created_at)).await;
        return Err(msg);
    }
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            let msg = format!("Parse: {e}");
            persist_quota_log(db, make_quota_log(&request_id, url, upstream_status, &msg, start.elapsed().as_millis() as i32, created_at)).await;
            return Err(msg);
        }
    };
    tracing::debug!(url = %url, body = %text, "quota response body");
    // 成功响应落库 (保留 body 原文); parse 失败也落库 (body 已在, 便于排查)
    persist_quota_log(db, make_quota_log(&request_id, url, upstream_status, &text, start.elapsed().as_millis() as i32, created_at)).await;
    serde_json::from_str(&text).map_err(|e| format!("Parse: {e}"))
}

/// 构造 quota 日志条目 (复用 fetch_models/model_test 标记约定, platform_id=0)。
fn make_quota_log(
    request_id: &str,
    url: &str,
    upstream_status: i32,
    body: &str,
    duration_ms: i32,
    created_at: i64,
) -> super::models::ProxyLog {
    super::models::ProxyLog {
        id: request_id.to_string(),
        group_key: "[quota]".into(),
        model: String::new(),
        actual_model: String::new(),
        source_protocol: "quota".into(),
        target_protocol: String::new(),
        platform_id: QUOTA_PLATFORM_ID.try_get().unwrap_or(0) as u64,
        request_headers: r#"{"source":"quota"}"#.into(),
        request_body: String::new(),
        upstream_request_headers: String::new(),
        upstream_request_body: String::new(),
        response_body: body.into(),
        request_url: "/quota".into(),
        upstream_request_url: url.to_string(),
        upstream_response_headers: String::new(),
        upstream_status_code: upstream_status,
        user_response_headers: r#"{"content-type":"application/json"}"#.to_string(),
        user_response_body: body.into(),
        status_code: upstream_status,
        duration_ms,
        input_tokens: 0,
        output_tokens: 0,
        cache_tokens: 0,
        est_cost: 0.0,
        is_stream: false,
        attempts: Vec::new(),
        retry_count: 0,
        blocked_by: String::new(),
        blocked_reason: String::new(),
        created_at,
        updated_at: created_at,
        deleted_at: 0,
    }
}

/// 落库 quota 日志 (仅 db 可写时; 测试传 None 跳过)。
async fn persist_quota_log(db: Option<&Arc<Db>>, log: super::models::ProxyLog) {
    if let Some(d) = db {
        if let Err(e) = super::db::upsert_proxy_log(d, log).await {
            tracing::warn!(error = %e, "persist quota log failed");
        }
    }
}

// ── 余额查询: DeepSeek ───────────────────────────────────
// GET https://api.deepseek.com/user/balance

async fn query_deepseek_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(db, "https://api.deepseek.com/user/balance",
        &[("Authorization", format!("Bearer {api_key}"))]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("deepseek", &e),
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

async fn query_stepfun_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(db, "https://api.stepfun.com/v1/accounts",
        &[("Authorization", format!("Bearer {api_key}"))]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("stepfun", &e),
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

async fn query_siliconflow_balance(db: Option<&Arc<Db>>, api_key: &str, is_cn: bool) -> PlatformQuota {
    let domain = if is_cn { "api.siliconflow.cn" } else { "api.siliconflow.com" };
    let url = format!("https://{domain}/v1/user/info");
    let body = match quota_get_json(db, &url,
        &[("Authorization", format!("Bearer {api_key}"))]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("siliconflow", &e),
    };
    let data = match body.get("data") {
        Some(d) => d,
        None => return err_quota_platform("siliconflow", "Missing data field"),
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

async fn query_openrouter_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(db, "https://openrouter.ai/api/v1/credits",
        &[("Authorization", format!("Bearer {api_key}"))]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("openrouter", &e),
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

async fn query_novita_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(db, "https://api.novita.ai/v3/user/balance",
        &[("Authorization", format!("Bearer {api_key}"))]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("novita", &e),
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

async fn query_kimi_coding_plan(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(db, "https://api.kimi.com/coding/v1/usages",
        &[("Authorization", format!("Bearer {api_key}"))]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("kimi", &e),
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

async fn query_zhipu_coding_plan(db: Option<&Arc<Db>>, base_url: &str, api_key: &str) -> PlatformQuota {
    let base = if base_url.to_lowercase().contains("bigmodel.cn") {
        "https://open.bigmodel.cn"
    } else {
        "https://api.z.ai"
    };
    let url = format!("{base}/api/monitor/usage/quota/limit");
    let body = match quota_get_json(db, &url, &[
        ("Authorization", api_key.to_string()), // 智谱不加 Bearer
        ("Content-Type", "application/json".to_string()),
    ]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("zhipu", &e),
    };
    if body.get("success").and_then(|v| v.as_bool()) == Some(false) {
        let msg = body.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown");
        return err_quota_platform("zhipu", msg);
    }
    let data = match body.get("data") {
        Some(d) => d,
        None => return err_quota_platform("zhipu", "Missing data field"),
    };
    let level = data.get("level").and_then(|v| v.as_str()).map(String::from);
    let mut tiers = Vec::new();
    if let Some(limits) = data.get("limits").and_then(|v| v.as_array()) {
        // Phase 1: 按 unit 字段分类 TOKENS_LIMIT（unit=3→5h, unit=6→weekly）
        type Entry = (Option<i64>, f64, Option<String>);
        let mut five_hour: Option<Entry> = None;
        let mut weekly: Option<Entry> = None;
        let mut unclassified: Vec<Entry> = Vec::new();

        for item in limits {
            let limit_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let pct = item.get("percentage").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let reset_ms = item.get("nextResetTime").and_then(|v| v.as_i64());
            let reset_iso = reset_ms.and_then(millis_to_iso8601);

            if limit_type.eq_ignore_ascii_case("TOKENS_LIMIT") {
                let entry = (reset_ms, pct, reset_iso);
                match item.get("unit").and_then(|v| v.as_i64()) {
                    Some(3) if five_hour.is_none() => five_hour = Some(entry),
                    Some(6) if weekly.is_none() => weekly = Some(entry),
                    _ => unclassified.push(entry),
                }
            } else if limit_type.eq_ignore_ascii_case("TIME_LIMIT") {
                // MCP 月用量（绝对量）
                let total = parse_f64_field(item, "usage").unwrap_or(0.0);
                let used = parse_f64_field(item, "currentValue").unwrap_or(0.0);
                let remaining = parse_f64_field(item, "remaining").unwrap_or(0.0);
                let utilization = if total > 0.0 { (used / total) * 100.0 } else { 0.0 };
                tiers.push(QuotaTier {
                    name: "mcp_monthly".into(),
                    utilization,
                    resets_at: reset_iso,
                    limit: if total > 0.0 { Some(total) } else { None },
                    remaining: if remaining > 0.0 { Some(remaining) } else { None },
                });
            }
        }

        // 未分类条目按 reset 升序填入空槽（兜底启发式）
        unclassified.sort_by_key(|(reset, _, _)| (reset.is_some(), reset.unwrap_or(i64::MIN)));
        for entry in unclassified {
            if five_hour.is_none() { five_hour = Some(entry); }
            else if weekly.is_none() { weekly = Some(entry); }
        }

        // 按固定顺序输出 token tiers
        if let Some((_, pct, resets_at)) = five_hour {
            tiers.insert(0, QuotaTier { name: "five_hour".into(), utilization: pct, resets_at, limit: None, remaining: None });
        }
        if let Some((_, pct, resets_at)) = weekly {
            // 插入到 five_hour 之后、mcp_monthly 之前
            let pos = tiers.iter().position(|t| t.name == "mcp_monthly").unwrap_or(tiers.len());
            tiers.insert(pos, QuotaTier { name: "weekly_limit".into(), utilization: pct, resets_at, limit: None, remaining: None });
        }
    }
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(), balance: None,
        coding_plan: Some(CodingPlanInfo { tiers, level }),
        newapi_user_id: None,
    }
}
// ── Coding Plan: MiniMax ─────────────────────────────────
// GET https://{domain}/v1/api/openplatform/coding_plan/remains
//   domain = is_cn ? "api.minimaxi.com" : "api.minimax.io"
// 响应: { base_resp:{status_code,status_msg}, model_remains:[ {model_name, ...} ] }
//   取 model_name == "general"：
//     - five_hour 桶: current_interval_* (status==1 + remaining_percent + end_time→resets_at)
//     - weekly   桶: current_weekly_*   (status==1 + remaining_percent + weekly_end_time)
//   utilization = 100 - remaining_percent；MiniMax 只暴露百分比，无绝对量 → limit/remaining = None

async fn query_minimax_coding_plan(db: Option<&Arc<Db>>, api_key: &str, is_cn: bool) -> PlatformQuota {
    let domain = if is_cn { "api.minimaxi.com" } else { "api.minimax.io" };
    let url = format!("https://{domain}/v1/api/openplatform/coding_plan/remains");
    let body = match quota_get_json(db, &url, &[
        ("Authorization", format!("Bearer {api_key}")),
        ("Content-Type", "application/json".to_string()),
    ]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("minimax", &e),
    };
    if let Some(base_resp) = body.get("base_resp") {
        let code = base_resp.get("status_code").and_then(|v| v.as_i64()).unwrap_or(-1);
        if code != 0 {
            let msg = base_resp.get("status_msg").and_then(|v| v.as_str()).unwrap_or("Unknown");
            return err_quota_platform("minimax", &format!("API error (code {code}): {msg}"));
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

/// 根据 base_url 自动检测平台并查询余额或 Coding Plan 配额。
/// platform_id 透传给落库日志（task_local scope），让 Logs 页能显示归属平台。
pub async fn query_quota(db: Option<&Arc<Db>>, base_url: &str, api_key: &str, platform_id: i64) -> PlatformQuota {
    QUOTA_PLATFORM_ID.scope(platform_id, query_quota_inner(db, base_url, api_key)).await
}

async fn query_quota_inner(db: Option<&Arc<Db>>, base_url: &str, api_key: &str) -> PlatformQuota {
    if api_key.trim().is_empty() {
        return err_quota("API key is empty");
    }
    let url = base_url.to_lowercase();

    // Coding Plan 查询 (优先检测，这些平台通常同时有 Coding Plan)
    if url.contains("api.kimi.com/coding") {
        return query_kimi_coding_plan(db, api_key).await;
    }
    if url.contains("open.bigmodel.cn") || url.contains("bigmodel.cn") {
        // GLM 可能同时返回 coding plan
        let quota = query_zhipu_coding_plan(db, base_url, api_key).await;
        return quota;
    }
    if url.contains("api.z.ai") {
        return query_zhipu_coding_plan(db, base_url, api_key).await;
    }
    if url.contains("api.minimaxi.com") {
        return query_minimax_coding_plan(db, api_key, true).await;
    }
    if url.contains("api.minimax.io") {
        return query_minimax_coding_plan(db, api_key, false).await;
    }

    // 余额查询
    if url.contains("api.deepseek.com") {
        return query_deepseek_balance(db, api_key).await;
    }
    if url.contains("api.stepfun.com") || url.contains("api.stepfun.ai") {
        return query_stepfun_balance(db, api_key).await;
    }
    if url.contains("api.siliconflow.cn") {
        return query_siliconflow_balance(db, api_key, true).await;
    }
    if url.contains("api.siliconflow.com") {
        return query_siliconflow_balance(db, api_key, false).await;
    }
    if url.contains("openrouter.ai") {
        return query_openrouter_balance(db, api_key).await;
    }
    if url.contains("api.novita.ai") {
        return query_novita_balance(db, api_key).await;
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
