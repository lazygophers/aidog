//! CLI 代理 provider 模型（cpa-standalone-module s1）。
//!
//! 独立于 platform 表的 CLI 代理上游 provider。wire_protocol 为入站协议标识
//! （anthropic/openai/glm_coding 等，对应 Protocol serde 形式）；models 为 JSON 数组；
//! extra 为原始 JSON 串（仿 platform.extra）。

use serde::{Deserialize, Serialize};

/// CLI 代理 provider 主行。对应 `cli_proxy_provider` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliProxyProvider {
    pub id: u64,
    pub name: String,
    /// 入站协议标识（anthropic/openai/glm_coding 等）
    pub wire_protocol: String,
    pub base_url: String,
    pub api_key: String,
    /// 模型列表（DB 存 JSON 数组字符串）
    pub models: Vec<String>,
    /// 原始 JSON 串（空串视作 "{}"，仿 platform.extra）
    pub extra: String,
    /// active / disabled
    pub status: String,
    /// 归属分组 id；NULL = 未分配（s2 路由层消费）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 创建入参。id / created_at / updated_at 由 create_provider 写入时填。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCliProxyProvider {
    pub name: String,
    pub wire_protocol: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub extra: String,
    #[serde(default = "default_active_status")]
    pub status: String,
    #[serde(default)]
    pub group_id: Option<i64>,
}

/// 更新入参。所有字段全量覆写（无部分更新，对齐 mcp.rs upsert idiom）。
pub type UpdateCliProxyProvider = CreateCliProxyProvider;

fn default_active_status() -> String {
    "active".to_string()
}

/// 解析 models 列（出错回退空数组）
pub fn parse_cli_proxy_models(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse cli_proxy models failed, using empty list");
        Vec::new()
    })
}

/// 序列化 models 列（出错回退 "[]"）
pub fn serialize_cli_proxy_models(models: &[String]) -> String {
    serde_json::to_string(models).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize cli_proxy models failed, persisting empty array");
        "[]".to_string()
    })
}

#[cfg(test)]
mod serde_tests {
    use super::*;

    #[test]
    fn provider_serde_roundtrip() {
        let p = CliProxyProvider {
            id: 7,
            name: "p1".into(),
            wire_protocol: "anthropic".into(),
            base_url: "https://api.x.com/v1".into(),
            api_key: "sk-x".into(),
            models: vec!["claude-sonnet-4".into(), "claude-opus-4".into()],
            extra: "{\"k\":\"v\"}".into(),
            status: "active".into(),
            group_id: Some(3),
            created_at: 1000,
            updated_at: 2000,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: CliProxyProvider = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, 7);
        assert_eq!(back.models, p.models);
        assert_eq!(back.group_id, Some(3));
        // snake_case 字段名（对齐 DB 列名，跨边界一致性）
        assert!(json.contains("\"wire_protocol\""));
        assert!(json.contains("\"base_url\""));
        assert!(json.contains("\"group_id\""));
    }

    #[test]
    fn create_input_defaults() {
        // 仅必填字段 → 默认值生效
        let json = r#"{"name":"p","wire_protocol":"openai","base_url":"u"}"#;
        let c: CreateCliProxyProvider = serde_json::from_str(json).unwrap();
        assert_eq!(c.api_key, "");
        assert!(c.models.is_empty());
        assert_eq!(c.status, "active");
        assert!(c.group_id.is_none());
    }

    #[test]
    fn parse_models_corrupt_returns_empty() {
        assert!(parse_cli_proxy_models("not json").is_empty());
        assert_eq!(parse_cli_proxy_models("[\"a\",\"b\"]"), vec!["a".to_string(), "b".to_string()]);
    }
}
