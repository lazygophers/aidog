use serde::{Deserialize, Serialize};

/// 支持的 AI 协议类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Protocol {
    // ── AI 请求协议（可作为 endpoint 协议）──
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "openai_responses")]
    OpenAIResponses,
    #[serde(rename = "openai_completions")]
    OpenAICompletions,
    #[serde(rename = "gemini")]
    Gemini,
    // ── 平台类型（仅作为平台主协议，不作为 endpoint 协议）──
    #[serde(rename = "glm")]
    Glm,
    #[serde(rename = "kimi")]
    Kimi,
    #[serde(rename = "minimax")]
    MiniMax,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "bailian")]
    Bailian,
}

/// 路由模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoutingMode {
    #[serde(rename = "load_balance")]
    LoadBalance,
    #[serde(rename = "failover")]
    Failover,
}

// ─── Platform Models ───────────────────────────────────────

/// 平台模型配置：5 个固定槽位
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlatformModels {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sonnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub haiku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpt: Option<String>,
}

impl PlatformModels {
    /// 返回所有已配置的模型名（去重）
    #[allow(dead_code)]
    pub fn all_values(&self) -> Vec<String> {
        let mut v = Vec::new();
        for s in [&self.default, &self.sonnet, &self.opus, &self.haiku, &self.gpt].into_iter().flatten() {
            if !v.contains(s) {
                v.push(s.clone());
            }
        }
        v
    }
}

// ─── ClientType (客户端模拟) ─────────────────────────────────

/// 可模拟的客户端类型，用于通过上游的客户端校验。
/// 参考 claude-code-hub 的客户端检测逻辑：
///   - Claude Code 家族: CLI / VSCode / SDK-TS / SDK-PY / GitHub Action
///   - Codex 家族: CLI-Rust / TUI / Desktop / VSCode
///   - IDE: Cursor / Windsurf
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum ClientType {
    #[default]
    #[serde(rename = "default")]
    Default,
    // ── Claude Code family ──
    #[serde(rename = "claude_code")]
    ClaudeCode,
    #[serde(rename = "claude_code_vscode")]
    ClaudeCodeVscode,
    #[serde(rename = "claude_code_sdk_ts")]
    ClaudeCodeSdkTs,
    #[serde(rename = "claude_code_sdk_py")]
    ClaudeCodeSdkPy,
    #[serde(rename = "claude_code_gh_action")]
    ClaudeCodeGhAction,
    // ── Codex family ──
    #[serde(rename = "codex_cli")]
    CodexCli,
    #[serde(rename = "codex_tui")]
    CodexTui,
    #[serde(rename = "codex_desktop")]
    CodexDesktop,
    #[serde(rename = "codex_vscode")]
    CodexVscode,
    // ── IDE ──
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "windsurf")]
    Windsurf,
}

impl ClientType {
    /// 根据 endpoint 协议返回推荐的默认客户端类型：
    /// - anthropic → claude_code (CLI)
    /// - openai → codex_tui
    /// - 其他 → default
    #[allow(dead_code)]
    pub fn default_for_protocol(protocol: &Protocol) -> Self {
        match protocol {
            Protocol::Anthropic => ClientType::ClaudeCode,
            Protocol::OpenAI | Protocol::OpenAIResponses | Protocol::OpenAICompletions => ClientType::CodexTui,
            _ => ClientType::Default,
        }
    }
}

// ─── Platform Endpoint ──────────────────────────────────────

/// 平台协议端点：同一平台可支持多种协议，每种协议对应不同的 base_url
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEndpoint {
    pub protocol: Protocol,
    pub base_url: String,
    /// 模拟的客户端类型（用于通过上游客户端校验）
    #[serde(default)]
    pub client_type: ClientType,
    /// 是否为 Coding Plan（针对支持编程代理订阅的平台，如 Kimi Code Plan）
    #[serde(default)]
    pub coding_plan: bool,
}

// ─── Platform ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub api_key: String,
    /// JSON 额外配置
    pub extra: Option<String>,
    /// 平台模型配置
    pub models: PlatformModels,
    /// 从 API 获取到的可用模型列表
    pub available_models: Vec<String>,
    /// 额外协议端点：每种协议对应不同的 base_url
    #[serde(default)]
    pub endpoints: Vec<PlatformEndpoint>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlatform {
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub api_key: String,
    pub extra: Option<String>,
    #[serde(default)]
    pub models: Option<PlatformModels>,
    #[serde(default)]
    pub available_models: Option<Vec<String>>,
    #[serde(default)]
    pub endpoints: Option<Vec<PlatformEndpoint>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlatform {
    pub id: String,
    pub name: Option<String>,
    pub protocol: Option<Protocol>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub extra: Option<String>,
    pub models: Option<PlatformModels>,
    pub available_models: Option<Vec<String>>,
    pub endpoints: Option<Vec<PlatformEndpoint>>,
    pub enabled: Option<bool>,
}

// ─── Group ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    /// URL path 前缀，如 "/claude"
    pub path: String,
    pub routing_mode: RoutingMode,
    /// 如果由平台自动创建，记录关联平台 ID
    pub auto_from_platform: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// 超时设置（秒），0 = 继承系统设置
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    /// 入站协议（默认 anthropic）
    #[serde(default = "default_source_protocol")]
    pub source_protocol: String,
}

fn default_source_protocol() -> String { "anthropic".to_string() }

#[derive(Debug, Deserialize)]
pub struct CreateGroup {
    pub name: String,
    pub path: String,
    pub routing_mode: RoutingMode,
    pub auto_from_platform: Option<String>,
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    #[serde(default = "default_source_protocol_opt")]
    pub source_protocol: Option<String>,
}

fn default_source_protocol_opt() -> Option<String> { Some("anthropic".to_string()) }

#[derive(Debug, Deserialize)]
pub struct UpdateGroup {
    pub id: String,
    pub name: Option<String>,
    pub path: Option<String>,
    pub routing_mode: Option<RoutingMode>,
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    #[serde(default)]
    pub source_protocol: Option<String>,
}

// ─── GroupPlatform (关联) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct GroupPlatform {
    pub group_id: String,
    pub platform_id: String,
    /// 故障转移优先级（越小越优先）
    pub priority: i32,
    /// 负载均衡权重
    pub weight: i32,
}

#[derive(Debug, Deserialize)]
pub struct SetGroupPlatforms {
    pub group_id: String,
    /// (platform_id, priority, weight) 列表
    pub platforms: Vec<GroupPlatformInput>,
}

#[derive(Debug, Deserialize)]
pub struct GroupPlatformInput {
    pub platform_id: String,
    pub priority: Option<i32>,
    pub weight: Option<i32>,
}

// ─── ModelMapping ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMapping {
    pub id: String,
    pub group_id: String,
    /// 对外模型名，如 "claude-sonnet-4-6"
    pub source_model: String,
    pub target_platform_id: String,
    /// 实际模型名，如 "glm-4-plus"
    pub target_model: String,
    /// 超时设置（秒），0 = 继承分组设置
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct CreateModelMapping {
    pub group_id: String,
    pub source_model: String,
    pub target_platform_id: String,
    pub target_model: String,
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateModelMapping {
    pub id: String,
    pub source_model: Option<String>,
    pub target_platform_id: Option<String>,
    pub target_model: Option<String>,
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
}

// ─── 辅助：带平台详情的分组 ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupDetail {
    pub group: Group,
    pub platforms: Vec<GroupPlatformDetail>,
    pub model_mappings: Vec<ModelMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupPlatformDetail {
    pub platform: Platform,
    pub priority: i32,
    pub weight: i32,
}

// ─── Settings (KV) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SettingEntry {
    pub scope: String,
    pub key: String,
    pub value: serde_json::Value,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SetSettingInput {
    pub scope: String,
    pub key: String,
    pub value: serde_json::Value,
}

// ─── ProxyLog ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLog {
    pub id: String,
    pub group_name: String,
    /// 用户请求的原始模型
    pub model: String,
    /// 实际发送给上游的模型（可能因路由映射而不同）
    pub actual_model: String,
    /// 用户请求的协议（固定 anthropic）
    pub source_protocol: String,
    /// 实际请求上游的协议
    pub target_protocol: String,
    /// 路由到的目标平台 ID
    pub platform_id: String,
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
    pub created_at: String,
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
}

/// Summary row for list view (excludes large body fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLogSummary {
    pub id: String,
    pub group_name: String,
    pub model: String,
    pub actual_model: String,
    pub source_protocol: String,
    pub target_protocol: String,
    pub status_code: i32,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_tokens: i32,
    pub created_at: String,
}

/// Proxy logging settings stored in settings table (scope=proxy, key=logging)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLogSettings {
    #[serde(default)]
    pub enabled: bool,
    /// Maximum days to retain logs; 0 = keep forever
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_retention_days() -> u32 {
    7
}

impl Default for ProxyLogSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            retention_days: default_retention_days(),
        }
    }
}

// ─── Proxy Timeout Settings ─────────────────────────────────

/// Upstream request timeout configuration (stored in settings table)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTimeoutSettings {
    /// Total request timeout in seconds (0 = no limit)
    #[serde(default)]
    pub request_timeout_secs: u64,
    /// TCP connection timeout in seconds (0 = no limit)
    #[serde(default)]
    pub connect_timeout_secs: u64,
}

impl Default for ProxyTimeoutSettings {
    fn default() -> Self {
        Self {
            request_timeout_secs: 300,  // 5 minutes
            connect_timeout_secs: 10,   // 10 seconds
        }
    }
}

// ─── Statistics ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub start: Option<String>,
    pub end: Option<String>,
    pub granularity: Option<String>,
    pub group_by: Option<String>,
    pub filter_group: Option<String>,
    pub filter_model: Option<String>,
    pub filter_protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsOverview {
    pub total_requests: i32,
    pub success_rate: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_tokens: i64,
    pub cache_rate: f64,
    pub avg_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsBucket {
    pub time_bucket: String,
    pub total_requests: i32,
    pub success_count: i32,
    pub error_count: i32,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_tokens: i64,
    pub avg_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionEntry {
    pub name: String,
    pub total_requests: i32,
    pub success_count: i32,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_tokens: i64,
    pub avg_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResult {
    pub overview: StatsOverview,
    pub buckets: Vec<StatsBucket>,
    pub dimension_data: Vec<DimensionEntry>,
}

// ─── Model Testing ────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ModelTestRequest {
    pub platform_id: String,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTestResult {
    pub success: bool,
    pub model: String,
    pub prompt_preview: String,
    pub response_preview: String,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub error: String,
}

/// Built-in test prompts — short, harmless, clearly not real requests
#[allow(dead_code)]
pub const TEST_PROMPTS: &[&str] = &[
    "Respond with only the word 'hello' in lowercase.",
    "Calculate 7 x 13 and respond with only the number.",
    "List exactly 3 primary colors, comma-separated.",
    "What is the capital of France? Answer in one word.",
    "Translate 'good morning' to Japanese. One word only.",
    "Count the letters in 'artificial'. Respond with only the number.",
    "What is 15% of 200? Answer with only the number.",
    "Name the 4th planet from the Sun. One word.",
    "What element has the symbol 'O'? One word.",
    "How many days are in a leap year? Answer with only the number.",
];
