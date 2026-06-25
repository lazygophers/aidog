//! 平台模型：尝试快照 / 模型槽位 / 客户端类型 / 端点 / 平台主体 / 熔断覆盖 / 增改入参。

use super::{ManualBudget, PlatformStatus, Protocol};
use serde::{Deserialize, Serialize};

/// proxy_log.attempts JSON 数组元素：每次平台尝试的快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyAttempt {
    pub platform_id: u64,
    pub platform_name: String,
    /// 上游返回的 HTTP 状态码；连接失败 / 超时为 0
    pub status_code: i32,
    /// 错误描述（连接失败 / 超时 / 上游错误体摘要）；成功为空串
    #[serde(default)]
    pub error: String,
    pub duration_ms: i64,
    /// 本次尝试发起时间（毫秒 unix 时间戳）
    pub ts: i64,
}

/// 序列化 attempts 列（出错回退空数组）
pub fn serialize_attempts(items: &[ProxyAttempt]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string())
}

/// 解析 attempts 列（出错回退空数组）
pub fn parse_attempts(s: &str) -> Vec<ProxyAttempt> {
    serde_json::from_str(s).unwrap_or_default()
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

/// 容错反序列化 client_type：未知字符串回退为 ClientType::Default，
/// 而非让整个 endpoints 数组解析失败。
fn deserialize_client_type_lenient<'de, D>(deserializer: D) -> Result<ClientType, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(serde_json::from_value(serde_json::Value::String(s)).unwrap_or_default())
}

/// 平台协议端点：同一平台可支持多种协议，每种协议对应不同的 base_url
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEndpoint {
    pub protocol: Protocol,
    pub base_url: String,
    /// 模拟的客户端类型（用于通过上游客户端校验）。
    /// 用 `deserialize_with` 容错：DB 中历史遗留 / 未知 client_type 字符串
    /// （如旧数据里的 "anthropic"）回退为 Default，避免单个未知值导致整个
    /// endpoints 数组反序列化失败 → 空 Vec → 前端 Protocol Endpoints 丢失。
    #[serde(default, deserialize_with = "deserialize_client_type_lenient")]
    pub client_type: ClientType,
    /// 是否为 Coding Plan（针对支持编程代理订阅的平台，如 Kimi Code Plan）
    #[serde(default)]
    pub coding_plan: bool,
}

// ─── Platform ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub id: u64,
    pub name: String,
    pub platform_type: Protocol,
    pub base_url: String,
    pub api_key: String,
    /// JSON 额外配置
    pub extra: String,
    /// 平台模型配置
    pub models: PlatformModels,
    /// 从 API 获取到的可用模型列表
    pub available_models: Vec<String>,
    /// 额外协议端点：每种协议对应不同的 base_url
    #[serde(default)]
    pub endpoints: Vec<PlatformEndpoint>,
    /// 旧布尔启用位，保留向后兼容（旧读者 / 旧前端）。写入端从 status 同步：
    /// `status==Enabled → true`，否则 false。新逻辑（router 过滤 / 前端三态）走 status。
    pub enabled: bool,
    /// 三态状态：enabled / disabled(用户手动) / auto_disabled(401/403 自动)
    #[serde(default)]
    pub status: PlatformStatus,
    /// auto_disabled 下次试探时间（毫秒 unix 时间戳）；退避用，0 = 立即可试探
    #[serde(default)]
    pub auto_disabled_until: i64,
    /// 连续自动禁用次数（指数退避指数）；恢复 enabled 时清零
    #[serde(default)]
    pub auto_disable_strikes: i64,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
    /// 预估剩余余额（按量计费平台，请求驱动增量自减；系统维护，前端只读）
    #[serde(default)]
    pub est_balance_remaining: f64,
    /// 预估 coding plan JSON（含 tiers est_utilization + 方案 B 拟合系数/样本；系统维护，前端只读）
    #[serde(default)]
    pub est_coding_plan: String,
    /// 上次真实 quota 查询毫秒戳（校准基准；系统维护，前端只读）
    #[serde(default)]
    pub last_real_query_at: i64,
    /// 自上次真查以来的预估次数（校准计数；系统维护，前端只读）
    #[serde(default)]
    pub estimate_count: i64,
    /// 是否在 tray 中展示此平台
    #[serde(default)]
    pub show_in_tray: bool,
    /// tray 展示类型: "balance" | "coding"
    #[serde(default)]
    pub tray_display: String,
    /// 排序权重（越小越靠前），0 = 按 created_at 排序
    #[serde(default)]
    pub sort_order: i64,
    /// 手动预算限额列表（仅无上游 quota 自动支持平台；请求驱动扣减 + 耗尽阻断）
    #[serde(default)]
    pub manual_budgets: Vec<ManualBudget>,
    /// 余额使用速率配色级别（非 DB 列；`platform_list` 按动态窗口日速率算 days_remaining 后填充）。
    /// "red"|"yellow"|"green"|"neutral"，前端列表页余额只消费此 level 不重算阈值（usage_color 唯一源）。
    /// 缺省空串 → 前端退中性。`skip_deserializing` 避免从前端入参反序列化。
    #[serde(default, skip_deserializing)]
    pub balance_level: String,
}

/// 平台级熔断阈值覆盖，存于 `platform.extra` JSON 的嵌套对象 `breaker`。
/// 每字段 0/缺省 = 继承全局 `SchedulingBreakerSettings` 默认（语义同旧顶层列）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlatformBreaker {
    #[serde(default)]
    pub failure_threshold: u32,
    #[serde(default)]
    pub open_secs: u64,
    #[serde(default)]
    pub half_open_max: u32,
}

/// 从 `extra` JSON 字符串解析 `breaker` 嵌套对象；空/非法/缺键 → 全 0（继承全局默认）。
pub fn parse_breaker(extra: &str) -> PlatformBreaker {
    if extra.trim().is_empty() {
        return PlatformBreaker::default();
    }
    serde_json::from_str::<serde_json::Value>(extra)
        .ok()
        .and_then(|v| v.get("breaker").cloned())
        .and_then(|b| serde_json::from_value(b).ok())
        .unwrap_or_default()
}

/// 把 breaker 阈值合并进 `extra` JSON 的 `breaker` 键（保留 extra 其余字段）。
/// 三值全 0 时移除 `breaker` 键（无覆盖 → 继承全局，不留空对象）。空 extra → "{}" 起步。
pub fn merge_breaker_into_extra(extra: &str, b: &PlatformBreaker) -> String {
    let mut root = serde_json::from_str::<serde_json::Value>(extra.trim())
        .ok()
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    let obj = root.as_object_mut().expect("object");
    if b.failure_threshold == 0 && b.open_secs == 0 && b.half_open_max == 0 {
        obj.remove("breaker");
    } else {
        obj.insert("breaker".to_string(), serde_json::to_value(b).unwrap_or_default());
    }
    serde_json::to_string(&root).unwrap_or_else(|_| "{}".to_string())
}

impl Platform {
    /// 解析本平台 `extra.breaker` 覆盖阈值（缺省全 0 = 继承全局默认）。
    pub fn breaker(&self) -> PlatformBreaker {
        parse_breaker(&self.extra)
    }
}

#[derive(Debug, Deserialize)]
pub struct CreatePlatform {
    pub name: String,
    pub platform_type: Protocol,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub extra: String,
    #[serde(default)]
    pub models: Option<PlatformModels>,
    #[serde(default)]
    pub available_models: Option<Vec<String>>,
    #[serde(default)]
    pub endpoints: Option<Vec<PlatformEndpoint>>,
    #[serde(default)]
    pub manual_budgets: Option<Vec<ManualBudget>>,
    /// 是否自动创建默认分组（transient 输入，不入库）：None→true 保持旧行为；
    /// false=创建时不建默认分组。该选择是创建时一次性判断，不持久化。
    #[serde(default)]
    pub auto_group: Option<bool>,
    /// 额外加入的已有分组 ID 列表（plain membership，不写 auto_from_platform）。
    #[serde(default)]
    pub join_group_ids: Option<Vec<u64>>,
    /// 自动创建的默认分组的 per-group level_priority 初值（1~10）。
    /// transient 输入，不入 platform 表；None→落库走 DEFAULT 5。
    /// 仅当平台最终归属唯一分组（auto_group 建的默认组）时由前端传入。
    #[serde(default)]
    pub default_level_priority: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlatform {
    pub id: u64,
    pub name: Option<String>,
    pub platform_type: Option<Protocol>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub extra: Option<String>,
    pub models: Option<PlatformModels>,
    pub available_models: Option<Vec<String>>,
    pub endpoints: Option<Vec<PlatformEndpoint>>,
    pub enabled: Option<bool>,
    /// 前端三态切换：显式置 enabled / disabled。
    /// 注意：禁止前端直接置 auto_disabled（仅系统 401/403 联动设置）；置 enabled 会清空退避状态。
    pub status: Option<PlatformStatus>,
    pub manual_budgets: Option<Vec<ManualBudget>>,
    /// 全量同步该平台的手动组成员关系（None=不动；Some(set)=加入 set 内、移出 set 外，
    /// auto 分组不受影响）。
    /// 注：熔断阈值覆盖现走 `extra.breaker`（随 `extra` 字段整体更新），不再有独立列。
    #[serde(default)]
    pub join_group_ids: Option<Vec<u64>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── serialize_attempts / parse_attempts ──

    #[test]
    fn attempts_roundtrip_empty() {
        let s = serialize_attempts(&[]);
        assert_eq!(s, "[]");
        let v = parse_attempts(&s);
        assert!(v.is_empty());
    }

    #[test]
    fn attempts_roundtrip_with_items() {
        let items = vec![
            ProxyAttempt { platform_id: 1, platform_name: "p1".into(), status_code: 200, error: "".into(), duration_ms: 150, ts: 0 },
            ProxyAttempt { platform_id: 2, platform_name: "p2".into(), status_code: 500, error: "err".into(), duration_ms: 300, ts: 1 },
        ];
        let s = serialize_attempts(&items);
        let parsed = parse_attempts(&s);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].platform_id, 1);
        assert_eq!(parsed[1].status_code, 500);
    }

    #[test]
    fn parse_attempts_invalid_returns_empty() {
        let v = parse_attempts("not-json");
        assert!(v.is_empty());
    }

    // ── PlatformModels::all_values ──

    #[test]
    fn platform_models_all_values_deduplicates() {
        let pm = PlatformModels {
            default: Some("gpt-4o".into()),
            sonnet: Some("claude-sonnet-4".into()),
            opus: Some("gpt-4o".into()), // duplicate
            haiku: None,
            gpt: Some("gpt-3.5".into()),
        };
        let vals = pm.all_values();
        assert_eq!(vals.len(), 3, "dedup: {:?}", vals);
        assert!(vals.contains(&"gpt-4o".to_string()));
        assert!(vals.contains(&"claude-sonnet-4".to_string()));
    }

    #[test]
    fn platform_models_all_values_empty() {
        let pm = PlatformModels::default();
        assert!(pm.all_values().is_empty());
    }

    // ── parse_breaker ──

    #[test]
    fn parse_breaker_empty_returns_default() {
        let b = parse_breaker("");
        assert_eq!(b.failure_threshold, 0);
        assert_eq!(b.open_secs, 0);
        assert_eq!(b.half_open_max, 0);
    }

    #[test]
    fn parse_breaker_with_values() {
        let extra = r#"{"breaker":{"failure_threshold":5,"open_secs":30,"half_open_max":2}}"#;
        let b = parse_breaker(extra);
        assert_eq!(b.failure_threshold, 5);
        assert_eq!(b.open_secs, 30);
        assert_eq!(b.half_open_max, 2);
    }

    #[test]
    fn parse_breaker_no_breaker_key_returns_default() {
        let extra = r#"{"other_field": "value"}"#;
        let b = parse_breaker(extra);
        assert_eq!(b.failure_threshold, 0);
    }

    #[test]
    fn parse_breaker_invalid_json_returns_default() {
        let b = parse_breaker("not-json");
        assert_eq!(b.failure_threshold, 0);
    }

    // ── merge_breaker_into_extra ──

    #[test]
    fn merge_breaker_all_zero_removes_breaker_key() {
        let extra = r#"{"proxy_enabled":true,"breaker":{"failure_threshold":3}}"#;
        let b = PlatformBreaker { failure_threshold: 0, open_secs: 0, half_open_max: 0 };
        let out = merge_breaker_into_extra(extra, &b);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.get("breaker").is_none(), "all-zero should remove breaker: {out}");
        assert_eq!(v["proxy_enabled"], serde_json::json!(true), "other fields preserved: {out}");
    }

    #[test]
    fn merge_breaker_nonzero_inserts_breaker() {
        let b = PlatformBreaker { failure_threshold: 3, open_secs: 60, half_open_max: 1 };
        let out = merge_breaker_into_extra("{}", &b);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["breaker"]["failure_threshold"], 3);
        assert_eq!(v["breaker"]["open_secs"], 60);
    }

    #[test]
    fn merge_breaker_empty_extra_starts_empty_object() {
        let b = PlatformBreaker { failure_threshold: 1, open_secs: 10, half_open_max: 0 };
        let out = merge_breaker_into_extra("", &b);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["breaker"]["failure_threshold"], 1);
    }

    // ── Platform::breaker() ──
    #[test]
    fn platform_breaker_delegates_to_parse_breaker() {
        let p = Platform {
            id: 1, name: "p".into(), platform_type: super::Protocol::OpenAI,
            base_url: "http://x".into(), api_key: "k".into(),
            extra: r#"{"breaker":{"failure_threshold":7,"open_secs":90,"half_open_max":3}}"#.into(),
            enabled: true, status: Default::default(), est_balance_remaining: 0.0,
            models: PlatformModels::default(), available_models: vec![],
            endpoints: vec![], manual_budgets: vec![],
            auto_disabled_until: 0, auto_disable_strikes: 0,
            created_at: 0, updated_at: 0, deleted_at: 0,
            est_coding_plan: "".into(), last_real_query_at: 0, estimate_count: 0,
            show_in_tray: false, tray_display: "".into(), sort_order: 0, balance_level: "".into(),
        };
        let b = p.breaker();
        assert_eq!(b.failure_threshold, 7);
    }
}
