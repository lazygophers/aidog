//! MCP 工具 provider 设置的数据形状与读取。
//!
//! 设置键：scope = `"mcp_tools"`，key = `"settings"`。写入走通用设置命令；内核 `mcp`
//! 子命令与网关按这份配置取搜索 provider（生图 provider 已移除，票 08）。

use aidog_db::{self as db, Db};

/// 搜索设置（定义在 `aidog_search`，供网关 / HTTP MCP / kernel 共用而不成环）。
pub use aidog_search::SearchProviderConfig;

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
    // 生图 provider 配置已移除（生图-模型类型 票 08）：MCP 生图统一走调用方分组的
    // image 槽位链。存量 DB 值含 `image` 键 → serde 默认忽略未知字段，反序列化不炸。
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

crate::tauri_command! {
    /// Tools 页设置读取（票 57-01）：与 MCP 工具调用同走 [`load_mcp_tools_settings`]——
    /// 读时迁移旧形状并回写，返回值永远是新形状（无记录 → 默认值，不报错）。
    /// 写路径仍走通用 `settings_set` 读-改-写；key 字段经管理接口由 `secret_fields`
    /// 按 `keys` 后缀脱敏，桌面 IPC 不脱敏（与 `settings_get` 同口径）。
    pub async fn mcp_tools_settings_get() -> McpToolsSettings {
        load_mcp_tools_settings(aidog_ctx::db()).await
    }
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
    fn legacy_video_and_image_keys_are_ignored() {
        // 旧配置残留 video / image 键：serde 忽略未知字段，不报错（票 08：image 不迁移）。
        let s: McpToolsSettings = serde_json::from_value(json!({
            "video": {"base_url": "https://v/v1", "api_key": "k", "model": "m"},
            "image": {"base_url": "https://i/v1", "api_key": "k", "model": "m"}
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

    #[tokio::test]
    async fn settings_get_view_for_tools_page_migrates_legacy_keys_and_is_idempotent() {
        // 票 57-01：`mcp_tools_settings_get` 命令体 = 本函数。Tools 页打开旧形状后保存，
        // 旧 key 必须经迁移视图保留；再次读取不改库（幂等）。
        let (db, _d) = aidog_db::test_support::three_file_db().await;
        put(&db, json!({"search": {"provider": "brave", "keys": ["k1"]}})).await;
        let view = load_mcp_tools_settings(&db).await;
        assert_eq!(view.search.services["brave"].keys, vec!["k1".to_string()], "旧 key 迁入 services");
        let stored = db::get_setting(&db, "mcp_tools", "settings").await.unwrap().unwrap();
        let again = load_mcp_tools_settings(&db).await;
        assert_eq!(view, again, "命令返回值幂等");
        assert_eq!(
            stored,
            db::get_setting(&db, "mcp_tools", "settings").await.unwrap().unwrap(),
            "第二次读不再写库"
        );
    }
}
