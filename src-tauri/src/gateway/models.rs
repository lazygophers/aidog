use serde::{Deserialize, Serialize};

/// 支持的 AI 协议类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Protocol {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "glm")]
    GLM,
    #[serde(rename = "kimi")]
    Kimi,
    #[serde(rename = "minimax")]
    MiniMax,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "claude_code")]
    ClaudeCode,
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
        for m in [&self.default, &self.sonnet, &self.opus, &self.haiku, &self.gpt] {
            if let Some(s) = m {
                if !v.contains(s) {
                    v.push(s.clone());
                }
            }
        }
        v
    }
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
}

#[derive(Debug, Deserialize)]
pub struct CreateGroup {
    pub name: String,
    pub path: String,
    pub routing_mode: RoutingMode,
    pub auto_from_platform: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroup {
    pub id: String,
    pub name: Option<String>,
    pub path: Option<String>,
    pub routing_mode: Option<RoutingMode>,
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
}

#[derive(Debug, Deserialize)]
pub struct CreateModelMapping {
    pub group_id: String,
    pub source_model: String,
    pub target_platform_id: String,
    pub target_model: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateModelMapping {
    pub id: String,
    pub source_model: Option<String>,
    pub target_platform_id: Option<String>,
    pub target_model: Option<String>,
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
    pub request_headers: String,
    pub request_body: String,
    pub response_body: String,
    pub status_code: i32,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub created_at: String,
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
