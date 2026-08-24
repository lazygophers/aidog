//! 中间件规则引擎模型（统一引擎，ADR 0003）。
//!
//! 一条规则 = Condition Tree（嵌套 ALL/ANY，叶子 target+field+match_type+pattern）
//! 加 Action Chain（有序动作，block/classify 终止一切）加 Applies To 过滤器。
//! Applies To 三维 platforms/groups/models 各自空 = 不限；规则按 priority 累加执行。
//! 旧 8 类 RuleType、三级就近覆盖 scope、空 pattern 兜底全部废弃；旧模型残留行
//! 由 migration 翻译或标记 failed（Failed Rule，前端引导手删）。

use super::default_true;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[cfg(test)]
#[path = "test_middleware.rs"]
mod test_middleware;

/// 条件匹配目标。请求侧/响应侧 target 决定规则求值阶段（同规则内叶子必须同阶段）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// 请求 body（field 为空 = 聚合全文；否则 JSON path 定位字段）
    RequestBody,
    /// 请求 header（field = header 名）
    RequestHeaders,
    /// 上游响应 body（field 语义同 request_body）
    ResponseBody,
    /// 上游响应 header
    ResponseHeaders,
    /// 上游状态码（字符串匹配，如 regex ^5）
    Status,
    /// 请求 model 字段
    Model,
}

impl Target {
    pub fn as_str(&self) -> &'static str {
        match self {
            Target::RequestBody => "request_body",
            Target::RequestHeaders => "request_headers",
            Target::ResponseBody => "response_body",
            Target::ResponseHeaders => "response_headers",
            Target::Status => "status",
            Target::Model => "model",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        Some(match s {
            "request_body" => Target::RequestBody,
            "request_headers" => Target::RequestHeaders,
            "response_body" => Target::ResponseBody,
            "response_headers" => Target::ResponseHeaders,
            "status" => Target::Status,
            "model" => Target::Model,
            _ => return None,
        })
    }

    /// 是否响应侧（出站阶段求值）。
    pub fn is_response_side(&self) -> bool {
        matches!(self, Target::ResponseBody | Target::ResponseHeaders | Target::Status)
    }
}

/// 匹配方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
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
            _ => MatchType::Contains,
        }
    }
}

/// 条件树叶子。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ConditionLeaf {
    pub target: Target,
    /// 空 = 目标整体文本；request_body/response_body 支持 JSON path（如 messages.0.content）
    #[serde(default)]
    pub field: String,
    pub match_type: MatchType,
    pub pattern: String,
}

/// 条件树节点：嵌套 ALL/ANY 组或叶子。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConditionNode {
    All { children: Vec<ConditionNode> },
    Any { children: Vec<ConditionNode> },
    Leaf(ConditionLeaf),
}

/// 动作种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// 脱敏遮罩（params.replacement / params.fields）
    Mask,
    /// 拦截（终止性；请求侧 4xx，流式首块前断流）
    Block,
    /// 仅告警记日志
    Warn,
    /// 注入（params.inject_mode/system_append|body_set、target、value）
    Inject,
    /// 改写命中片段为 replacement
    Override,
    /// 错误分类（终止性；category/retryable/override_status/override_body 喂重试编排）
    Classify,
}

impl ActionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionKind::Mask => "mask",
            ActionKind::Block => "block",
            ActionKind::Warn => "warn",
            ActionKind::Inject => "inject",
            ActionKind::Override => "override",
            ActionKind::Classify => "classify",
        }
    }

    /// 终止性动作：停止本链及后续规则。
    pub fn is_terminal(&self) -> bool {
        matches!(self, ActionKind::Block | ActionKind::Classify)
    }
}

/// 动作参数（一个平铺结构按需取用，各 kind 只读自己关心的字段；默认值安全）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ActionParams {
    /// mask/override 的替换文本
    #[serde(default = "default_replacement")]
    pub replacement: String,
    /// mask 的字段限定（request_body：messages/system；空 = 全部）
    #[serde(default)]
    pub fields: Vec<String>,
    /// inject：system_append | body_set
    #[serde(default)]
    pub inject_mode: String,
    /// inject：body_set 的 JSON key / header_set 的 header 名
    #[serde(default)]
    pub target: String,
    /// inject 的注入值
    #[serde(default)]
    pub value: String,
    /// classify：分类类别（人读/审计）
    #[serde(default)]
    pub category: String,
    /// classify：false = 立即返回不换候选；缺省 true（可重试）
    #[serde(default = "default_true")]
    pub retryable: bool,
    /// classify：覆写回客户端状态码
    #[serde(default)]
    pub override_status: Option<u16>,
    /// classify：覆写回客户端响应体
    #[serde(default)]
    pub override_body: Option<String>,
}

impl Default for ActionParams {
    fn default() -> Self {
        Self {
            replacement: default_replacement(),
            fields: Vec::new(),
            inject_mode: String::new(),
            target: String::new(),
            value: String::new(),
            category: String::new(),
            retryable: true,
            override_status: None,
            override_body: None,
        }
    }
}

fn default_replacement() -> String {
    "****".to_string()
}

/// 动作链一步。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ActionStep {
    pub kind: ActionKind,
    #[serde(default)]
    pub params: ActionParams,
}

/// 规则应用范围过滤器：三维各自空 = 不限；多值 = 命中任一；三维间 AND。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct AppliesTo {
    #[serde(default)]
    #[ts(type = "number[]")]
    pub platforms: Vec<i64>,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub models: Vec<String>,
}

/// 单条中间件规则（对应 `middleware_rule` 表一行）。
///
/// `conditions` / `actions` / `applies_to` 在模型层是强类型结构（DB TEXT JSON 列）。
/// `failed = true` 表示旧模型残留无法翻译（Failed Rule，前端引导手删，引擎不执行）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct MiddlewareRule {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub conditions: ConditionNode,
    #[serde(default)]
    pub actions: Vec<ActionStep>,
    #[serde(default)]
    pub applies_to: AppliesTo,
    #[serde(default)]
    #[ts(type = "number")]
    pub priority: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_builtin: bool,
    /// 旧模型残留无法翻译 → 前端展示失败态引导手删；引擎跳过。
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    #[ts(type = "number")]
    pub created_at: i64,
    #[serde(default)]
    #[ts(type = "number")]
    pub updated_at: i64,
}

/// 创建规则入参（前端不传 id/时间戳）。
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct CreateMiddlewareRule {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub conditions: ConditionNode,
    #[serde(default)]
    pub actions: Vec<ActionStep>,
    #[serde(default)]
    pub applies_to: AppliesTo,
    #[serde(default)]
    #[ts(type = "number")]
    pub priority: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_builtin: bool,
}

/// 更新规则入参（全量覆盖，id 必填；Failed Rule 允许删除不允许编辑）。
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct UpdateMiddlewareRule {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub conditions: ConditionNode,
    #[serde(default)]
    pub actions: Vec<ActionStep>,
    #[serde(default)]
    pub applies_to: AppliesTo,
    #[serde(default)]
    #[ts(type = "number")]
    pub priority: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 规则校验：条件树内所有叶子必须同阶段（请求侧/响应侧），混阶段拒绝。
/// 返回 Err 描述冲突，Ok(()) 通过。
pub fn validate_rule_phases(node: &ConditionNode) -> Result<(), String> {
    fn walk(node: &ConditionNode, phase: &mut Option<bool>) -> Result<(), String> {
        match node {
            ConditionNode::All { children } | ConditionNode::Any { children } => {
                for c in children {
                    walk(c, phase)?;
                }
                Ok(())
            }
            ConditionNode::Leaf(leaf) => {
                let p = leaf.target.is_response_side();
                if let Some(prev) = *phase
                    && prev != p
                {
                    return Err(format!(
                        "mixed-phase conditions not allowed: leaf target '{}' on opposite side",
                        leaf.target.as_str()
                    ));
                }
                *phase = Some(p);
                Ok(())
            }
        }
    }
    let mut phase = None;
    walk(node, &mut phase)
}

/// 中间件总设置（settings KV：scope="middleware" key="settings"）。
/// 统一引擎后仅剩总开关；旧 8 类 type_toggles 子开关已废（票 02）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct MiddlewareSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for MiddlewareSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}
