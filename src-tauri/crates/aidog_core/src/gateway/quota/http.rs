//! quota 子模块共享: 类型、工具函数、统一出站 HTTP + 日志落库。

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use crate::gateway::db::Db;

// 当前 quota 查询归属的平台 ID。
// query_quota / query_quota_newapi 进入时 scope 设定；quota_get_json 单点落库时读取，
// 避免沿 10 个 provider 函数链逐层透传 platform_id 签名。未设（如裸调测试）→ 0。
tokio::task_local! {
    pub(crate) static QUOTA_PLATFORM_ID: i64;
    // cli_proxy_test 透传的 provider 归属 ID。scope 内有值 → make_quota_log 填
    // ProxyLog.cli_proxy_provider_id；未设（platform_query_quota / cold_start 等路径）→ None。
    pub(crate) static QUOTA_CLI_PROXY_PROVIDER_ID: i64;
}

/// 在 cli_proxy_provider_id task_local scope 内执行 fut。
/// cli_proxy_test 调 query_quota 前用此包裹，provider_id 透传至 make_quota_log 落库。
/// scope() 本身是 RAII——future 结束即释放，无 leak。其他路径不调此 = try_get 返 None = NULL。
pub async fn with_cli_proxy_provider_id<R>(
    pid: i64,
    fut: impl std::future::Future<Output = R>,
) -> R {
    QUOTA_CLI_PROXY_PROVIDER_ID.scope(pid, fut).await
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

pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub(crate) fn millis_to_iso8601(ms: i64) -> Option<String> {
    let secs = ms / 1000;
    let nsecs = ((ms % 1000) * 1_000_000) as u32;
    chrono::DateTime::from_timestamp(secs, nsecs).map(|dt| dt.to_rfc3339())
}

pub(crate) fn parse_f64(value: &serde_json::Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

pub(crate) fn parse_f64_field(obj: &serde_json::Value, field: &str) -> Option<f64> {
    obj.get(field).and_then(parse_f64)
}

pub(crate) fn err_quota(msg: &str) -> PlatformQuota {
    tracing::warn!(error = %msg, "quota query failed");
    PlatformQuota { success: false, error: Some(msg.to_string()), queried_at: now_millis(), balance: None, coding_plan: None, newapi_user_id: None }
}

/// 同 err_quota，但附带平台标识，供排障定位是哪个平台查询失败。
pub(crate) fn err_quota_platform(platform: &str, msg: &str) -> PlatformQuota {
    tracing::warn!(platform = %platform, error = %msg, "quota query failed");
    PlatformQuota { success: false, error: Some(msg.to_string()), queried_at: now_millis(), balance: None, coding_plan: None, newapi_user_id: None }
}

async fn http_client(db: Option<&Arc<Db>>) -> reqwest::Client {
    match db {
        Some(db) => crate::gateway::http_client::build_http_client_system(db, 10, 5).await,
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
pub(crate) async fn quota_get_json(
    db: Option<&Arc<Db>>,
    url: &str,
    headers: &[(&str, String)],
) -> Result<serde_json::Value, String> {
    tracing::info!(method = "GET", url = %url, "quota outbound request");
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let created_at = crate::gateway::db::now();

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
    tracing::debug!(url = %url, body = %crate::gateway::log_util::log_body_preview(&text), "quota response body");
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
) -> crate::gateway::models::ProxyLog {
    crate::gateway::models::ProxyLog {
        id: request_id.to_string(),
        group_key: "[quota]".into(),
        model: String::new(),
        actual_model: String::new(),
        source_protocol: "quota".into(),
        target_protocol: String::new(),
        platform_id: QUOTA_PLATFORM_ID.try_get().unwrap_or(0) as u64,
        cli_proxy_provider_id: QUOTA_CLI_PROXY_PROVIDER_ID.try_get().ok(),
        request_headers: r#"{"source":"quota"}"#.into(),
        request_body: String::new(),
        upstream_request_headers: String::new(),
        upstream_request_body: String::new(),
        response_body: body.into(),
        // quota 是 aidog 主动拉余额，无独立用户侧 URL；记完整上游 URL（非占位 path）便于日志可读。
        request_url: url.to_string(),
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
async fn persist_quota_log(db: Option<&Arc<Db>>, log: crate::gateway::models::ProxyLog) {
    if let Some(d) = db
        && let Err(e) = crate::gateway::db::upsert_proxy_log(d, log).await {
            tracing::warn!(error = %e, "persist quota log failed");
        }
}

#[cfg(test)]
#[path = "test_http.rs"]
mod test_http;
