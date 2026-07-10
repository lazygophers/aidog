//! 中间件规则引擎模型（C1 基座）。
//!
//! 8 类请求/响应中间件规则的公共数据模型。表 `middleware_rule` 单表存储，
//! 枚举全部 snake_case serde，与 src/services/api.ts 字面量联合类型一一对齐
//! （契约见 .trellis/tasks/06-13-request-response-middleware/design.md）。
//! 实际执行（入站/出站 apply）由 C2/C3 在 proxy.rs 落地；本文件只定义模型。

use super::default_true;
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "test_middleware.rs"]
mod test_middleware;

/// 规则类型（8 类中间件能力）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    /// 请求字段过滤（model 白/黑名单等）
    RequestFilter,
    /// 敏感词拦截（pattern 即词）
    SensitiveWord,
    /// 脱敏（字段值替换）
    Redaction,
    /// 内容过滤
    ContentFilter,
    /// 动态注入（system/header/body）
    DynamicInjection,
    /// 响应覆写（成功体改写）
    ResponseOverride,
    /// 矫正器（SSE/JSON/编码/字段缺省修复）
    Rectifier,
    /// 错误分类规则（重试/熔断/覆写状态码）
    ErrorRule,
}

impl RuleType {
    /// DB TEXT 列值（与 serde snake_case 一致）。
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleType::RequestFilter => "request_filter",
            RuleType::SensitiveWord => "sensitive_word",
            RuleType::Redaction => "redaction",
            RuleType::ContentFilter => "content_filter",
            RuleType::DynamicInjection => "dynamic_injection",
            RuleType::ResponseOverride => "response_override",
            RuleType::Rectifier => "rectifier",
            RuleType::ErrorRule => "error_rule",
        }
    }

    /// 从 DB TEXT 值解析；未知值返回 None（fail-open：调用方跳过该行）。
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "request_filter" => Some(RuleType::RequestFilter),
            "sensitive_word" => Some(RuleType::SensitiveWord),
            "redaction" => Some(RuleType::Redaction),
            "content_filter" => Some(RuleType::ContentFilter),
            "dynamic_injection" => Some(RuleType::DynamicInjection),
            "response_override" => Some(RuleType::ResponseOverride),
            "rectifier" => Some(RuleType::Rectifier),
            "error_rule" => Some(RuleType::ErrorRule),
            _ => None,
        }
    }
}

/// 规则作用域（三级，就近覆盖语义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleScope {
    /// 全局：所有请求
    Global,
    /// 分组：scope_ref = group_key
    Group,
    /// 平台：scope_ref = platform_id(字符串)
    Platform,
}

impl RuleScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleScope::Global => "global",
            RuleScope::Group => "group",
            RuleScope::Platform => "platform",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "group" => RuleScope::Group,
            "platform" => RuleScope::Platform,
            // 未知/空 → global（最安全的兜底层）
            _ => RuleScope::Global,
        }
    }
}

/// 匹配方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    /// 正则（regex crate，无回溯抗 ReDoS）
    Regex,
    /// 子串包含
    Contains,
    /// 完全相等
    Exact,
}

impl MatchType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchType::Regex => "regex",
            MatchType::Contains => "contains",
            MatchType::Exact => "exact",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "regex" => MatchType::Regex,
            "exact" => MatchType::Exact,
            // 默认 contains（与表 DEFAULT 一致）
            _ => MatchType::Contains,
        }
    }
}

/// 命中动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    /// 脱敏遮罩
    Mask,
    /// 拦截（立即返回 4xx）
    Block,
    /// 仅告警
    Warn,
    /// 注入
    Inject,
    /// 覆写
    Override,
    /// 分类（error_rule）
    Classify,
}

impl RuleAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleAction::Mask => "mask",
            RuleAction::Block => "block",
            RuleAction::Warn => "warn",
            RuleAction::Inject => "inject",
            RuleAction::Override => "override",
            RuleAction::Classify => "classify",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "mask" => RuleAction::Mask,
            "block" => RuleAction::Block,
            "inject" => RuleAction::Inject,
            "override" => RuleAction::Override,
            "classify" => RuleAction::Classify,
            // 默认 warn（与表 DEFAULT 一致，最弱副作用）
            _ => RuleAction::Warn,
        }
    }
}

/// 单条中间件规则（对应 `middleware_rule` 表一行）。
///
/// `config` 是 type-specific JSON 字符串（设计文档列出每类形状），
/// 引擎层不强解析，由各执行器（C2/C3）按需 `serde_json::from_str`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareRule {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub rule_type: RuleType,
    #[serde(default = "default_rule_scope")]
    pub scope: RuleScope,
    /// group_key | platform_id(字符串) | ''(global)
    #[serde(default)]
    pub scope_ref: String,
    #[serde(default = "default_match_type")]
    pub match_type: MatchType,
    #[serde(default)]
    pub pattern: String,
    #[serde(default = "default_rule_action")]
    pub action: RuleAction,
    /// type-specific JSON（默认 "{}"）
    #[serde(default = "default_config_json")]
    pub config: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_builtin: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

fn default_rule_scope() -> RuleScope { RuleScope::Global }
fn default_match_type() -> MatchType { MatchType::Contains }
fn default_rule_action() -> RuleAction { RuleAction::Warn }
fn default_config_json() -> String { "{}".to_string() }

/// 创建规则入参（前端不传 id/时间戳）。
#[derive(Debug, Clone, Deserialize)]
pub struct CreateMiddlewareRule {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub rule_type: RuleType,
    #[serde(default = "default_rule_scope")]
    pub scope: RuleScope,
    #[serde(default)]
    pub scope_ref: String,
    #[serde(default = "default_match_type")]
    pub match_type: MatchType,
    #[serde(default)]
    pub pattern: String,
    #[serde(default = "default_rule_action")]
    pub action: RuleAction,
    #[serde(default = "default_config_json")]
    pub config: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_builtin: bool,
}

/// 更新规则入参（全量覆盖，id 必填）。
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMiddlewareRule {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub rule_type: RuleType,
    #[serde(default = "default_rule_scope")]
    pub scope: RuleScope,
    #[serde(default)]
    pub scope_ref: String,
    #[serde(default = "default_match_type")]
    pub match_type: MatchType,
    #[serde(default)]
    pub pattern: String,
    #[serde(default = "default_rule_action")]
    pub action: RuleAction,
    #[serde(default = "default_config_json")]
    pub config: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_builtin: bool,
}

/// 中间件总设置（settings KV：scope="middleware" key="settings"）。
///
/// `enabled` 为总开关（OFF = 全旁路）；`type_toggles` 按 rule_type 子开关
/// （缺省视为 true，即默认所有类型启用）。
/// 注：熔断器已移出中间件层，归 group 功能块独立 task 实现，本结构不含 breaker。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// key = rule_type snake_case 字面量；缺省键视为 true。
    #[serde(default)]
    pub type_toggles: std::collections::HashMap<String, bool>,
}

impl Default for MiddlewareSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            type_toggles: std::collections::HashMap::new(),
        }
    }
}

impl MiddlewareSettings {
    /// 指定 rule_type 是否启用：总开关关 → 全 false；否则查 type_toggles，缺省 true。
    /// C2/C3 执行层判定调用。
    pub fn type_enabled(&self, rule_type: RuleType) -> bool {
        if !self.enabled {
            return false;
        }
        self.type_toggles
            .get(rule_type.as_str())
            .copied()
            .unwrap_or(true)
    }
}
