//! 分组模型：Group 主体 / 增改入参 / 平台关联 / 模型映射 / 带详情的分组。

use super::{Platform, RoutingMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    pub name: String,
    /// 分组密钥：Bearer token + 路由匹配键 + proxy_log 归属键（前端按 group_key 反查 name 显示）。
    /// UNIQUE。创建时若未提供则自动生成 `gk_<32hex>`；创建后锁定不可改。
    #[serde(default)]
    pub group_key: String,
    pub routing_mode: RoutingMode,
    /// 如果由平台自动创建，记录关联平台 ID（十进制字符串；空串表示非自动）
    pub auto_from_platform: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
    /// 超时设置（秒），0 = 继承系统设置
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    /// 入站协议（默认 anthropic）
    #[serde(default = "default_source_protocol")]
    pub source_protocol: String,
    /// 排序权重（越小越靠前），0 = 按 created_at 排序
    #[serde(default)]
    pub sort_order: i64,
    /// 分组级最大重试次数：失败后最多再换几个候选平台（0 = 不重试，只试 1 次）
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// 模型映射（内联 JSON 数组）
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
    /// 用户自定义环境变量（内联 JSON 数组）。sync 时注入 settings.{group}.json 的 env block，
    /// 但同名 ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN 被跳过（aidog 强写的 proxy 路由字段）。
    #[serde(default)]
    pub env_vars: Vec<EnvVar>,
    /// 是否为默认分组（单选）：true 时该组 config merge 写入
    /// `~/.claude/settings.json` + `~/.codex/config.toml`，使用户直接 `claude`/`codex`
    /// 不带 `-c`/`--profile` 即走此组。全局文件用 deep merge 保护用户其它字段。
    #[serde(default)]
    pub is_default: bool,
    /// JSON 扩展字段（仿 platform.extra）。当前承载 `_ui_*` UI 态（卡片折叠等），
    /// 业务键扩展同模式插入。空串 = "{}" 的轻量表示（解析时空串视作 {}）。
    #[serde(default)]
    pub extra: String,
}

fn default_source_protocol() -> String { "anthropic".to_string() }
fn default_max_retries() -> u32 { 10 }

#[derive(Debug, Deserialize)]
pub struct CreateGroup {
    pub name: String,
    /// 分组密钥；None 或空 → 自动生成 `gk_<32hex>`。创建后锁定不可改。
    #[serde(default)]
    pub group_key: Option<String>,
    pub routing_mode: RoutingMode,
    #[serde(default)]
    pub auto_from_platform: String,
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    #[serde(default = "default_source_protocol_opt")]
    pub source_protocol: Option<String>,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
    #[serde(default)]
    pub env_vars: Vec<EnvVar>,
}

fn default_source_protocol_opt() -> Option<String> { Some("anthropic".to_string()) }

#[derive(Debug, Deserialize)]
pub struct UpdateGroup {
    pub id: u64,
    pub name: Option<String>,
    pub routing_mode: Option<RoutingMode>,
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
    #[serde(default)]
    pub source_protocol: Option<String>,
    /// 分组级最大重试次数；None = 不变（保留既有值）
    #[serde(default)]
    pub max_retries: Option<u32>,
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
    #[serde(default)]
    pub env_vars: Vec<EnvVar>,
    /// 默认分组标记：本字段不参与 update_group UPDATE（默认组经 group_set_default
    /// command + db::set_default_group 单选切换）。这里保留仅为统一 struct 形态，
    /// update_group 返回的 `..existing` 透传原值，不丢失。
    #[serde(default)]
    #[allow(dead_code)]
    pub is_default: Option<bool>,
}

// ─── GroupPlatform (关联) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct GroupPlatform {
    pub id: u64,
    pub group_id: u64,
    pub platform_id: u64,
    /// 故障转移优先级（越小越优先）
    pub priority: i32,
    /// 负载均衡权重
    pub weight: i32,
    /// per-group 平台优先级（1~10，默认 5，10=最高优先；数大优先高）
    #[serde(default = "default_level_priority")]
    pub level_priority: i32,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
}

/// level_priority 默认值（5 = 中等优先）
pub fn default_level_priority() -> i32 {
    5
}

/// 把 level_priority clamp 到合法区间 [1, 10]
pub fn clamp_level_priority(v: i32) -> i32 {
    v.clamp(1, 10)
}

#[derive(Debug, Deserialize)]
pub struct SetGroupPlatforms {
    pub group_id: u64,
    /// (platform_id, priority, weight) 列表
    pub platforms: Vec<GroupPlatformInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupPlatformInput {
    pub platform_id: u64,
    pub priority: Option<i32>,
    pub weight: Option<i32>,
    /// per-group 平台优先级（1~10，None → 默认 5）
    #[serde(default)]
    pub level_priority: Option<i32>,
}

// ─── ModelMapping ──────────────────────────────────────────

/// 内联于 group.model_mappings JSON 数组的元素
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMapping {
    /// 对外模型名，如 "claude-sonnet-4-6"
    pub source_model: String,
    pub target_platform_id: u64,
    /// 实际模型名，如 "glm-4-plus"
    pub target_model: String,
    /// 超时设置（秒），0 = 继承分组设置
    #[serde(default)]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub connect_timeout_secs: u64,
}

/// 内联于 group.env_vars JSON 数组的元素（用户自定义环境变量）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_level_priority_is_5() {
        assert_eq!(default_level_priority(), 5);
    }

    #[test]
    fn clamp_level_priority_bounds() {
        assert_eq!(clamp_level_priority(0), 1);
        assert_eq!(clamp_level_priority(1), 1);
        assert_eq!(clamp_level_priority(5), 5);
        assert_eq!(clamp_level_priority(10), 10);
        assert_eq!(clamp_level_priority(11), 10);
        assert_eq!(clamp_level_priority(-5), 1);
    }

    #[test]
    fn default_source_protocol_is_anthropic() {
        assert_eq!(default_source_protocol(), "anthropic");
    }

    #[test]
    fn default_max_retries_is_10() {
        assert_eq!(default_max_retries(), 10);
    }
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
    /// per-group 平台优先级（1~10，默认 5，10=最高优先）
    #[serde(default = "default_level_priority")]
    pub level_priority: i32,
}
