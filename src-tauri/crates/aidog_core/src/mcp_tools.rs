//! MCP 工具 provider 设置（magpie 票 10 落 schema，票 11 填 provider 实现，票 12 落 UI）。
//!
//! 设置键：scope = `"mcp_tools"`，key = `"settings"`。本模块只定数据形状与读写；
//! 不带 `tauri_command!`（命令与前端表单留票 12），内核 `mcp` 子命令（票 11）按这份
//! 配置取搜索 / 生图 / 生视频 provider。

use aidog_db::{self as db, Db};

/// 搜索 provider 配置。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
pub struct SearchProviderConfig {
    /// provider id（词表与逐家实现留票 11；候选清单见 spec D5a：tavily / brave / exa /
    /// firecrawl / you.com / deepseek / 豆包 / 腾讯元宝 / searxng）。
    #[serde(default)]
    pub provider: String,
    /// API keys，多 key 轮换（D5a：多 key 数组 + 轮换；至少 1 个才算配置了搜索）。
    #[serde(default)]
    pub keys: Vec<String>,
}

impl SearchProviderConfig {
    /// 是否已配置到可用（有 provider 且至少一个非空 key）。
    pub fn is_configured(&self) -> bool {
        !self.provider.is_empty() && self.keys.iter().any(|k| !k.trim().is_empty())
    }
}

/// 生图 / 生视频 provider 配置：OpenAI images 兼容端点（D5：走用户配置的平台，
/// `base_url` 含版本前缀，最终 URL = base_url + 端点路径，禁额外拼接）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
pub struct MediaProviderConfig {
    /// 兼容端点 base_url（含版本前缀，如 `https://api.openai.com/v1`）。
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    /// 模型名（provider 侧的请求名）。
    #[serde(default)]
    pub model: String,
}

impl MediaProviderConfig {
    /// 是否已配置到可用。
    pub fn is_configured(&self) -> bool {
        !self.base_url.trim().is_empty()
            && !self.api_key.trim().is_empty()
            && !self.model.trim().is_empty()
    }
}

/// `mcp_tools/settings` 设置整体。字段缺省 = 未配置，对应工具回「未配置 provider」。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
pub struct McpToolsSettings {
    #[serde(default)]
    pub search: SearchProviderConfig,
    /// 生图 provider；None = generate_image 未配置。
    #[serde(default)]
    pub image: Option<MediaProviderConfig>,
    /// 生视频 provider；None = generate_video 未配置（首版未实现，D5 复活条件）。
    #[serde(default)]
    pub video: Option<MediaProviderConfig>,
}

/// 读设置。无记录 / 解析失败 → 默认值（全未配置），不 panic。
pub async fn load_mcp_tools_settings(db: &Db) -> McpToolsSettings {
    match db::get_setting(db, "mcp_tools", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "parse mcp_tools settings failed, using defaults");
            McpToolsSettings::default()
        }),
        _ => McpToolsSettings::default(),
    }
}

/// 写设置。
pub async fn save_mcp_tools_settings(db: &Db, s: &McpToolsSettings) -> Result<(), String> {
    let value =
        serde_json::to_value(s).map_err(|e| format!("serialize mcp_tools settings: {e}"))?;
    db::set_setting(
        db,
        crate::gateway::models::SetSettingInput {
            scope: "mcp_tools".to_string(),
            key: "settings".to_string(),
            value,
        },
    )
    .await
}

#[cfg(test)]
mod test_mcp_tools {
    use super::*;

    #[test]
    fn default_roundtrips_and_partial_json_parses() {
        let d = McpToolsSettings::default();
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(serde_json::from_value::<McpToolsSettings>(v).unwrap(), d);

        // 票 11 落库的真实形状：只填 search，image/video 缺省。
        let partial: McpToolsSettings = serde_json::from_value(serde_json::json!({
            "search": {"provider": "tavily", "keys": ["k1", "k2"]}
        }))
        .unwrap();
        assert_eq!(partial.search.provider, "tavily");
        assert_eq!(partial.search.keys.len(), 2);
        assert!(partial.search.is_configured());
        assert_eq!(partial.image, None);
        assert!(!partial.video.is_some_and(|c| c.is_configured()));
    }

    #[test]
    fn is_configured_rejects_blank_values() {
        let mut s = SearchProviderConfig::default();
        assert!(!s.is_configured());
        s.provider = "brave".into();
        s.keys = vec!["  ".into()];
        assert!(!s.is_configured(), "纯空白 key 不算配置");

        let m = MediaProviderConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "k".into(),
            model: String::new(),
        };
        assert!(!m.is_configured(), "缺 model 不算配置");
    }
}
