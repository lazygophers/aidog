//! 代理日志模型：完整日志行 / 平台用量统计 / 测试结果 / 列表摘要 / 筛选 / 日志设置。

use super::{default_true, ProxyAttempt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLog {
    pub id: String,
    pub group_key: String,
    /// 用户请求的原始模型
    pub model: String,
    /// 实际发送给上游的模型（可能因路由映射而不同）
    pub actual_model: String,
    /// 用户请求的协议（固定 anthropic）
    pub source_protocol: String,
    /// 实际请求上游的协议
    pub target_protocol: String,
    /// 路由到的目标平台 ID
    pub platform_id: u64,
    /// 原始请求头（用户发给代理的）
    pub request_headers: String,
    /// 原始请求体（用户发给代理的）
    pub request_body: String,
    /// 代理转发给上游的请求头
    pub upstream_request_headers: String,
    /// 代理转发给上游的请求体（协议转换后）
    pub upstream_request_body: String,
    /// 上游返回的响应体（非流式完整 JSON，流式为 "[stream]"）
    pub response_body: String,
    /// 用户请求的完整 URL
    #[serde(default)]
    pub request_url: String,
    /// 上游请求的完整 URL
    #[serde(default)]
    pub upstream_request_url: String,
    /// 上游返回的响应头
    #[serde(default)]
    pub upstream_response_headers: String,
    /// 上游 HTTP 状态码
    #[serde(default)]
    pub upstream_status_code: i32,
    /// 代理返回给用户的响应头
    #[serde(default)]
    pub user_response_headers: String,
    /// 代理返回给用户的响应体（非流式含模型名替换，流式为 "[stream]"）
    #[serde(default)]
    pub user_response_body: String,
    pub status_code: i32,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_tokens: i32,
    /// 预估花费（$），基于 model_price 定价
    #[serde(default)]
    pub est_cost: f64,
    /// 是否为流式（SSE）请求；流式日志的 body 为聚合的真实 SSE 内容（非 "[stream]" 哨兵）
    #[serde(default)]
    pub is_stream: bool,
    /// 每次平台尝试快照（JSON 数组列）；单平台一次成功时长度 1
    #[serde(default)]
    pub attempts: Vec<ProxyAttempt>,
    /// 重试次数 = attempts.len()-1（0 表示一次成功，无重试）
    #[serde(default)]
    pub retry_count: i32,
    /// 被中间件拦截时的规则标识（rule_type/规则名/id 拼接，空表示未被拦截）。C2 入站 block 写入。
    #[serde(default)]
    pub blocked_by: String,
    /// 拦截原因（命中模式 / 规则描述等人读说明，空表示未被拦截）。
    #[serde(default)]
    pub blocked_reason: String,
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
    /// 经 CLI 代理上游（cli_proxy_provider 表）路由时记录的 provider id；走传统 platform 路由为 None。
    #[serde(default)]
    pub cli_proxy_provider_id: Option<i64>,
}

/// 平台使用统计（从 proxy_logs 聚合）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformUsageStats {
    pub total_requests: i64,
    pub success_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_tokens: i64,
    pub cache_rate: f64,
    /// 最近 N 次请求中失败的次数（用于可用性判断）
    pub recent_failures: i64,
    /// 最近 N 次请求的总数
    pub recent_total: i64,
    /// 累计预估花费（$），基于 est_cost 聚合
    #[serde(default)]
    pub total_cost: f64,
    /// 今日（本地 00:00 起）token 总量（input + output），按 eff_pid 聚合
    #[serde(default)]
    pub today_tokens: i64,
    /// 今日（本地 00:00 起）预估花费（$），基于 est_cost 聚合
    #[serde(default)]
    pub today_cost: f64,
}

/// 平台「最近一次测试结果」（来自 proxy_log 中 source_protocol='test' 的最新一条）。
/// 供 PlatformCard 常驻徽章消费：ok/fail + 耗时 + 时间。
#[derive(Debug, Clone, Serialize)]
pub struct LastTestResult {
    /// status_code ∈ [200, 300) → true
    pub success: bool,
    pub status_code: i32,
    pub duration_ms: i32,
    /// proxy_log.created_at（毫秒 epoch）
    pub created_at: i64,
    /// 失败时取 response_body 截断 ~200 字符；成功为空串（徽章 title 短摘要用）
    pub error: String,
    /// 测试响应正文（成功/失败均带），截断 ~4000 字符；供前端 JSON 解析结构化展示。
    pub response_body: String,
}

/// Summary row for list view (excludes large body fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLogSummary {
    pub id: String,
    pub group_key: String,
    pub model: String,
    pub actual_model: String,
    pub source_protocol: String,
    pub target_protocol: String,
    pub platform_id: u64,
    pub status_code: i32,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_tokens: i32,
    /// 是否为流式（SSE）请求；列表展示流式标记
    #[serde(default)]
    pub is_stream: bool,
    /// 重试次数（retry_count>0 时列表显示重试徽标）
    #[serde(default)]
    pub retry_count: i32,
    pub created_at: i64,
}

/// 日志列表筛选条件
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProxyLogFilter {
    pub platform_id: Option<u64>,
    pub group_key: Option<String>,
    /// None=全部; Some(200)=仅成功; Some(-1)=仅失败
    pub status: Option<i32>,
    pub time_start: Option<i64>,
    pub time_end: Option<i64>,
    pub model: Option<String>,
    /// "original" = 按 model 列; "actual" = 按 actual_model 列
    pub model_type: Option<String>,
    /// 路径片段：对 request_url 做 LIKE %v% 模糊匹配
    #[serde(default)]
    pub path: Option<String>,
}

/// Proxy logging settings stored in settings table (scope=proxy, key=logging)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLogSettings {
    /// Master switch: whether to log proxy requests at all
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether to record user-side raw data (request headers + request body +
    /// response headers + response body to client). 关闭后这些列入库即清空，只留解析后元数据。
    #[serde(default)]
    pub log_user_request: bool,

    /// Whether to record upstream-side raw data (upstream request headers + body +
    /// upstream response headers + upstream response body). 关闭后这些列入库即清空，只留解析后元数据。
    #[serde(default)]
    pub log_upstream_request: bool,

    /// Days to retain user request data (headers, body); 0 = keep forever
    #[serde(default = "default_user_req_retention")]
    pub user_request_retention_days: u32,

    /// Days to retain upstream request data (headers, body); 0 = keep forever
    #[serde(default = "default_upstream_req_retention")]
    pub upstream_request_retention_days: u32,

    /// Days to retain entire log record; 0 = keep forever
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_user_req_retention() -> u32 { 7 }
fn default_upstream_req_retention() -> u32 { 7 }
fn default_retention_days() -> u32 { 90 }

impl Default for ProxyLogSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            log_user_request: false,
            log_upstream_request: false,
            user_request_retention_days: default_user_req_retention(),
            upstream_request_retention_days: default_upstream_req_retention(),
            retention_days: default_retention_days(),
        }
    }
}
