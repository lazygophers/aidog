use serde::{Deserialize, Serialize};

/// 支持的 AI 协议类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Anthropic,
    OpenAI,
    GLM,
    Kimi,
}

/// 路由模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingMode {
    LoadBalance,
    Failover,
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
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlatform {
    pub id: String,
    pub name: Option<String>,
    pub protocol: Option<Protocol>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub extra: Option<String>,
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
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateGroup {
    pub name: String,
    pub path: String,
    pub routing_mode: RoutingMode,
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
