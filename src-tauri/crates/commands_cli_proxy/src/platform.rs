//! CLI 代理 platform 建行命令（cpa-standalone-module s3）。
//!
//! 建 platform 表一行 platform_type=cli-proxy，extra 序列化 `{"cli_proxy_provider_id": <id>}`。
//! 路由层（candidates.rs::read_cli_proxy_provider_id）按此 key 拉 provider 注入 wire 字段。
//! 对齐 test_candidates.rs::mk_cli_proxy_platform idiom。

use aidog_core::gateway::{
    db::{self, Db},
    models::{CreatePlatform, Platform, Protocol},
};
use tauri::State;

/// 建 cli-proxy platform 行。extra 存 cli_proxy_provider_id 指向 provider 表。
/// base_url/api_key 留空（由路由层从 provider 注入）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn create_cli_proxy_platform(
    db: State<'_, Db>,
    provider_id: u64,
    name: Option<String>,
    group_id: Option<i64>,
) -> Result<Platform, String> {
    tracing::debug!(
        command = "create_cli_proxy_platform",
        provider_id,
        "command invoked"
    );
    // 校验 provider 存在（fail fast，避免建出 orphan platform）
    let provider = db::get_cli_proxy_provider(&db, provider_id)
        .await?
        .ok_or_else(|| format!("cli_proxy_provider {provider_id} 不存在"))?;

    // extra JSON：cli_proxy_provider_id 是路由层 read 的 key（见 candidates.rs:65-72）。
    let extra = serde_json::json!({ "cli_proxy_provider_id": provider_id }).to_string();
    // name 缺省 = provider.name；platform 行的展示名（路由不读，仅 UI）。
    let platform_name = name.unwrap_or_else(|| provider.name.clone());

    let input = CreatePlatform {
        name: platform_name,
        platform_type: Protocol::CliProxy,
        base_url: String::new(),
        api_key: String::new(),
        extra,
        models: None,
        available_models: None,
        endpoints: None,
        manual_budgets: None,
        auto_group: Some(true),
        // group_id 若提供，加入既有分组（plain membership）。
        join_group_ids: group_id.map(|g| vec![g as u64]),
        default_level_priority: None,
        expires_at: None,
    };
    let p = db::create_platform(&db, input).await?;
    tracing::info!(platform_id = p.id, provider_id, "cli-proxy platform created");
    Ok(p)
}
