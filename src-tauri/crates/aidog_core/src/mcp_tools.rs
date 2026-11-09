//! MCP 工具 provider 设置（magpie 票 10 落 schema，票 11 填 provider 实现，票 12 落 UI）。
//!
//! 设置键：scope = `"mcp_tools"`，key = `"settings"`。本模块只定数据形状与读写；
//! 不带 `tauri_command!`（命令与前端表单留票 12），内核 `mcp` 子命令（票 11）按这份
//! 配置取搜索 / 生图 / 生视频 provider。

use aidog_db::{self as db, Db};

/// 搜索 provider id 词表（spec D5a 的 9 家）。
pub const SEARCH_PROVIDERS: [&str; 9] = [
    "tavily",
    "brave",
    "exa",
    "firecrawl",
    "youcom",
    "deepseek",
    "doubao",
    "tencent_yuanbao",
    "searxng",
];

/// 搜索设置（定义在 `aidog_search`，供网关 / HTTP MCP / kernel 共用而不成环）。
pub use aidog_search::SearchProviderConfig;

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

    #[test]
    fn old_shape_search_json_keeps_keys_and_defaults_engines() {
        // 引擎开关上线前落库的真实形状：无 engines 键，所有旧字段（含 keys）原样读出。
        let s: McpToolsSettings = serde_json::from_value(serde_json::json!({
            "search": {"provider": "brave", "keys": ["k1"], "enabled": true,
                       "searxng_base_urls": ["http://s"]},
            "image": {"base_url": "https://i/v1", "api_key": "k", "model": "m"}
        }))
        .unwrap();
        assert_eq!(s.search.keys, vec!["k1".to_string()]);
        assert_eq!(s.search.searxng_base_urls, vec!["http://s".to_string()]);
        assert!(s.search.engines.is_empty(), "无 engines 键 = 全取默认开关");
        // 新字段往返
        let mut n = s.clone();
        n.search.engines.insert("bing".into(), false);
        let back: McpToolsSettings = serde_json::from_value(serde_json::to_value(&n).unwrap()).unwrap();
        assert_eq!(back.search.engines.get("bing"), Some(&false));
    }

    #[test]
    fn enabled_defaults_true_and_searxng_urls_parse() {
        // 缺省 enabled = true：配置了 provider 就该默认可用，开关只在显式禁用时为 false。
        let s: SearchProviderConfig = serde_json::from_value(serde_json::json!({
            "provider": "searxng", "keys": ["k"],
            "searxng_base_urls": ["http://a", "http://b"]
        }))
        .unwrap();
        assert!(s.enabled, "缺省 enabled 必须是 true");
        assert_eq!(s.searxng_base_urls, vec!["http://a".to_string(), "http://b".to_string()]);

        let off: SearchProviderConfig = serde_json::from_value(serde_json::json!({
            "provider": "tavily", "keys": ["k"], "enabled": false
        }))
        .unwrap();
        assert!(!off.enabled, "显式 enabled=false 必须保留");
    }

    #[test]
    fn search_providers_word_list_matches_spec_d5a() {
        // spec D5a 的 9 家；票 11 实现按此词表分发。
        assert_eq!(
            SEARCH_PROVIDERS,
            ["tavily", "brave", "exa", "firecrawl", "youcom", "deepseek", "doubao", "tencent_yuanbao", "searxng"]
        );
    }
}
