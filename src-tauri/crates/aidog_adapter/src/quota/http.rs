//! quota 子模块共享: 类型、工具函数、脚本出站 HTTP 单点 + 日志落库。
//! （quota-scripts T5：旧 per-provider 出站 `quota_get_json` 已随各平台 Rust 查询实现删除，
//! 出站 HTTP 仅剩脚本路径 `quota_script_request`。）

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use aidog_db::Db;

// 当前 quota 查询归属的平台 ID。
// query_quota / query_quota_for / 特化入口进入时 scope 设定；make_quota_log 落库时读取，
// 免沿调用链逐层透传 platform_id 签名。未设（如裸调测试）→ 0。
tokio::task_local! {
    pub static QUOTA_PLATFORM_ID: i64;
    // cli_proxy_test 透传的 provider 归属 ID。scope 内有值 → make_quota_log 填
    // ProxyLog.cli_proxy_provider_id；未设（platform_query_quota / cold_start 等路径）→ None。
    pub static QUOTA_CLI_PROXY_PROVIDER_ID: i64;
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

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn err_quota(msg: &str) -> PlatformQuota {
    tracing::warn!(error = %msg, "quota query failed");
    PlatformQuota { success: false, error: Some(msg.to_string()), queried_at: now_millis(), balance: None, coding_plan: None, newapi_user_id: None }
}

/// 系统代理感知的 client 构建器（aidog_core 启动时注入 build_http_client_system；
/// 未注入（裸测试）回落直连）。
pub type QuotaClientBuilder = std::sync::Arc<
    dyn Fn(&Arc<Db>) -> std::pin::Pin<Box<dyn std::future::Future<Output = reqwest::Client> + Send>>
        + Send
        + Sync,
>;
static CLIENT_BUILDER: std::sync::OnceLock<QuotaClientBuilder> = std::sync::OnceLock::new();

/// 注入 client 构建器（幂等，重复调用保留首个）。
pub fn set_client_builder(f: QuotaClientBuilder) {
    let _ = CLIENT_BUILDER.set(f);
}

pub(super) async fn http_client(db: Option<&Arc<Db>>) -> reqwest::Client {
    match (db, CLIENT_BUILDER.get()) {
        (Some(db), Some(f)) => f(db).await,
        _ => reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default(),
    }
}

/// JS 自定义查询脚本出站单点（get/post 统一）: 走注入的系统代理 client（由 script.rs
/// eval 前 build 好传入），错误/成功均落 proxy_log（group_key="[quota:script]"）。
/// 错误文案维持 script 既有格式（裸 reqwest 错误 /
/// `HTTP {status}: {body}` / `JSON parse: {e}`，脚本侧 try/catch 依赖）。
pub(super) async fn quota_script_request(
    db: Option<&Arc<Db>>,
    client: reqwest::Client,
    method: reqwest::Method,
    url: &str,
    body: Option<String>,
    headers: Vec<(String, String)>,
) -> Result<serde_json::Value, String> {
    tracing::info!(method = %method, url = %url, "quota script outbound request");
    let mut req = client.request(method, url);
    for (k, v) in &headers {
        req = req.header(k, v);
    }
    if let Some(b) = body {
        req = req.body(b);
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string();
            persist_quota_log(db, make_quota_log_for_script(url, 0, &msg)).await;
            return Err(msg);
        }
    };
    let status = resp.status().as_u16();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            let msg = e.to_string();
            persist_quota_log(db, make_quota_log_for_script(url, status, &msg)).await;
            return Err(msg);
        }
    };
    if !(200..300).contains(&status) {
        let msg = format!("HTTP {status}: {}", text.chars().take(500).collect::<String>());
        persist_quota_log(db, make_quota_log_for_script(url, status, &msg)).await;
        return Err(msg);
    }
    // 成功响应落库 (保留 body 原文); parse 失败也落库 (body 已在, 便于排查)
    persist_quota_log(db, make_quota_log_for_script(url, status, &text)).await;
    serde_json::from_str(&text).map_err(|e| format!("JSON parse: {e}"))
}

/// JS 自定义查询脚本出站请求日志（与 make_quota_log 同型，duration/created 简化）。
/// quota_script_request 单点调用（成功/失败均落）。
pub fn make_quota_log_for_script(url: &str, upstream_status: u16, body: &str) -> aidog_db::models::ProxyLog {
    let created_at = now_millis();
    let mut log = make_quota_log("", url, upstream_status as i32, body, 0, created_at);
    log.group_key = "[quota:script]".into();
    log
}

/// 构造 quota 日志条目 (复用 fetch_models/model_test 标记约定, platform_id=0)。
fn make_quota_log(
    request_id: &str,
    url: &str,
    upstream_status: i32,
    body: &str,
    duration_ms: i32,
    created_at: i64,
) -> aidog_db::models::ProxyLog {
    aidog_db::models::ProxyLog {
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
        done: true,
        // quota 拉取不经出站 body 构造 seam，无字段留痕（票 10）。
        field_trace: String::new(),
    }
}

/// 落库 quota 日志 (仅 db 可写时; 测试传 None 跳过)。
async fn persist_quota_log(db: Option<&Arc<Db>>, log: aidog_db::models::ProxyLog) {
    if let Some(d) = db
        && let Err(e) = aidog_logs::upsert_proxy_log(d, log).await {
            tracing::warn!(error = %e, "persist quota log failed");
        }
}

