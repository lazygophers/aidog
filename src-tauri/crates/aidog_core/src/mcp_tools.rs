//! MCP 工具 provider 设置的数据形状与读取。
//!
//! 设置键：scope = `"mcp_tools"`，key = `"settings"`。写入走通用设置命令；内核 `mcp`
//! 子命令与网关按这份配置取搜索 / 生图 provider。

use aidog_db::{self as db, Db};

/// 搜索设置（定义在 `aidog_search`，供网关 / HTTP MCP / kernel 共用而不成环）。
pub use aidog_search::SearchProviderConfig;

/// 生图 provider 配置：OpenAI images 兼容端点（D5：走用户配置的平台，
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

/// 抓网页（web_fetch）设置：「启用」开关（缺省 true）+ 返回格式（缺省 markdown；旧配置无该键
/// → markdown，属行为变更但为拍板结果；非法值整个 settings 解析失败回落全默认值）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct FetchConfig {
    #[serde(default = "default_fetch_enabled")]
    pub enabled: bool,
    /// 返回格式：markdown / html / text。
    #[serde(default)]
    pub format: aidog_search::FetchFormat,
}

fn default_fetch_enabled() -> bool {
    true
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self { enabled: default_fetch_enabled(), format: Default::default() }
    }
}

/// `mcp_tools/settings` 设置整体。字段缺省 = 未配置，对应工具回「未配置 provider」。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
pub struct McpToolsSettings {
    #[serde(default)]
    pub search: SearchProviderConfig,
    /// 抓网页；缺省启用。
    #[serde(default)]
    pub fetch: FetchConfig,
    /// 生图 provider；None = generate_image 未配置。
    #[serde(default)]
    pub image: Option<MediaProviderConfig>,
}

/// 读设置。无记录 / 解析失败 → 默认值，不 panic。
/// 旧搜索形状（`provider` / `keys` / `engines`）在读取时一次迁移并回写（幂等，见 [`aidog_search::migrate_search_value`]）。
pub async fn load_mcp_tools_settings(db: &Db) -> McpToolsSettings {
    let Ok(Some(mut raw)) = db::get_setting(db, "mcp_tools", "settings").await else {
        return McpToolsSettings::default();
    };
    if let Some(search) = raw.get_mut("search")
        && aidog_search::migrate_search_value(search)
    {
        let input = crate::gateway::models::SetSettingInput {
            scope: "mcp_tools".to_string(),
            key: "settings".to_string(),
            value: raw.clone(),
        };
        if let Err(e) = db::set_setting(db, input).await {
            tracing::warn!(error = %e, "write back migrated mcp_tools settings failed");
        }
    }
    serde_json::from_value(raw).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse mcp_tools settings failed, using defaults");
        McpToolsSettings::default()
    })
}

/// 写设置（仅测试夹具用，生产写入走设置命令）。
#[cfg(test)]
pub(crate) async fn save_mcp_tools_settings(db: &Db, s: &McpToolsSettings) -> Result<(), String> {
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
    use serde_json::json;

    #[test]
    fn default_roundtrips_and_partial_json_parses() {
        let d = McpToolsSettings::default();
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(serde_json::from_value::<McpToolsSettings>(v).unwrap(), d);

        // 新形状：只填 search.sources / services，image 缺省。
        let partial: McpToolsSettings = serde_json::from_value(json!({
            "search": {"sources": ["tavily"], "services": {"tavily": {"keys": ["k1", "k2"]}}}
        }))
        .unwrap();
        assert_eq!(partial.search.sources, vec!["tavily".to_string()]);
        assert_eq!(partial.search.services["tavily"].keys.len(), 2);
        assert_eq!(partial.search.mode, aidog_search::SearchMode::Sequential, "缺省 mode = sequential");
        assert!(partial.search.enabled);
        assert_eq!(partial.image, None);
    }

    #[test]
    fn fetch_defaults_enabled_and_old_config_without_key_is_enabled() {
        assert!(McpToolsSettings::default().fetch.enabled);
        let old: McpToolsSettings = serde_json::from_value(json!({"search": {"sources": ["tavily"]}})).unwrap();
        assert!(old.fetch.enabled, "旧配置无 fetch 键 = 启用");
        let empty_fetch: McpToolsSettings = serde_json::from_value(json!({"fetch": {}})).unwrap();
        assert!(empty_fetch.fetch.enabled);
        let off: McpToolsSettings = serde_json::from_value(json!({"fetch": {"enabled": false}})).unwrap();
        assert!(!off.fetch.enabled);
    }

    #[test]
    fn media_is_configured_rejects_blank_values() {
        let m = MediaProviderConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "k".into(),
            model: String::new(),
        };
        assert!(!m.is_configured(), "缺 model 不算配置");
    }

    #[test]
    fn legacy_video_key_is_ignored() {
        // 旧配置残留 video 键：serde 忽略未知字段，不报错。
        let s: McpToolsSettings = serde_json::from_value(json!({
            "video": {"base_url": "https://v/v1", "api_key": "k", "model": "m"}
        }))
        .unwrap();
        assert_eq!(s, McpToolsSettings::default());
    }

    async fn put(db: &Db, v: serde_json::Value) {
        db::set_setting(
            db,
            crate::gateway::models::SetSettingInput { scope: "mcp_tools".into(), key: "settings".into(), value: v },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn load_migrates_old_shape_writes_back_and_is_idempotent() {
        let (db, _d) = aidog_db::test_support::three_file_db().await;
        put(&db, json!({
            "search": {"provider": "brave", "keys": ["k1", " "], "enabled": false, "engines": {"bing": false}},
            "image": {"base_url": "https://i/v1", "api_key": "k", "model": "m"}
        }))
        .await;
        let first = load_mcp_tools_settings(&db).await;
        assert!(first.search.enabled, "旧 enabled=false 但有引擎可用 → 迁移后 enabled=true（保持实际效果）");
        assert!(!first.search.sources.contains(&"brave".to_string()), "key 服务不进列表");
        assert!(!first.search.sources.contains(&"bing".to_string()), "旧 engines 关掉的不进");
        assert_eq!(first.search.services["brave"].keys, vec!["k1".to_string()], "key 保留");
        assert_eq!(first.image.as_ref().unwrap().model, "m", "image 不受影响");
        // 已回写：库里已是新形状
        let stored = db::get_setting(&db, "mcp_tools", "settings").await.unwrap().unwrap();
        assert!(stored["search"].get("sources").is_some() && stored["search"].get("provider").is_none(), "{stored}");
        // 再读结果不变，库内容不变
        let second = load_mcp_tools_settings(&db).await;
        assert_eq!(first, second);
        assert_eq!(stored, db::get_setting(&db, "mcp_tools", "settings").await.unwrap().unwrap());
    }

    #[tokio::test]
    async fn load_without_record_or_with_corrupt_json_falls_back_to_default() {
        let (db, _d) = aidog_db::test_support::three_file_db().await;
        assert_eq!(load_mcp_tools_settings(&db).await, McpToolsSettings::default());
        put(&db, json!("not an object")).await;
        assert_eq!(load_mcp_tools_settings(&db).await, McpToolsSettings::default());
        put(&db, json!({"search": "garbage"})).await;
        assert_eq!(load_mcp_tools_settings(&db).await, McpToolsSettings::default());
    }
}
