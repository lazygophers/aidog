//! LiteLLM price table sync: fetch, parse, upsert into model_price table.

use super::db::Db;
use super::models::PriceSyncResult;
use std::sync::Arc;

/// LiteLLM public price table URL
const LITELLM_PRICE_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// Fetch and parse the LiteLLM price table, then upsert all entries.
///
/// 后台周期同步的每轮入口：建独立 trace_id span（非请求触发），本轮所有日志
/// 自动带 price_sync{trace_id=xxxxxxxx} 前缀，可按 id grep 出完整一轮同步。
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn sync_litellm_prices(db: &Db) -> Result<PriceSyncResult, String> {
    tracing::info!("litellm price sync started");
    let db_arc = Arc::new(db.clone());
    let json_str = match fetch_price_table(Some(&db_arc)).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "litellm price sync: fetch failed");
            return Err(e);
        }
    };
    let table: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&json_str) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "litellm price sync: parse json failed");
            return Err(format!("parse litellm json: {e}"));
        }
    };

    let mut added = 0u32;
    let mut updated = 0u32;
    let unchanged = 0u32;
    let mut failed = 0u32;
    let total = table.len() as u32;

    for (model_name, price_data) in &table {
        if model_name.is_empty() {
            continue;
        }
        // Only process chat models (skip image_generation, etc.)
        let mode = price_data.get("mode").and_then(|v| v.as_str()).unwrap_or("");
        if mode != "chat" && mode != "completion" && mode != "responses" {
            continue;
        }

        let price_json = match serde_json::to_string(price_data) {
            Ok(s) => s,
            Err(_) => { failed += 1; continue; }
        };

        // Check if data changed
        let existing = super::db::get_model_price(db, model_name).await.ok().flatten();
        let is_new = existing.is_none() || existing.as_ref().map(|e| e.source.as_str()) != Some("litellm");

        match super::db::upsert_model_price(db, model_name, "litellm", &price_json).await {
            Ok(()) => {
                if is_new { added += 1; } else { updated += 1; }
            }
            Err(_) => { failed += 1; }
        }
    }

    // Update last_sync_at in settings
    let sync_settings = get_sync_settings(db).await;
    let updated_settings = super::models::PriceSyncSettings {
        last_sync_at: super::db::now(),
        ..sync_settings
    };
    save_sync_settings(db, &updated_settings).await;

    tracing::info!(added, updated, unchanged, failed, total, "litellm price sync completed");
    Ok(PriceSyncResult { added, updated, unchanged, failed, total })
}

async fn fetch_price_table(db: Option<&Arc<Db>>) -> Result<String, String> {
    let client = match db {
        Some(db) => super::http_client::build_http_client_system(db, 30, 10).await,
        None => reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("build http client: {e}"))?,
    };

    let resp = client.get(LITELLM_PRICE_URL)
        .send()
        .await
        .map_err(|e| format!("fetch litellm prices: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        tracing::warn!(%status, "litellm price fetch: non-success status");
        return Err(format!("litellm returned status {status}"));
    }

    resp.text().await.map_err(|e| {
        tracing::warn!(error = %e, "litellm price fetch: read response body failed");
        format!("read litellm response: {e}")
    })
}

/// Read sync settings from DB
pub async fn get_sync_settings(db: &Db) -> super::models::PriceSyncSettings {
    super::db::get_setting(db, "pricing", "sync")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Save sync settings to DB
pub async fn save_sync_settings(db: &Db, settings: &super::models::PriceSyncSettings) {
    let value = match serde_json::to_value(settings) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "save price sync settings: serialize failed");
            return;
        }
    };
    if let Err(e) = super::db::set_setting(db, super::models::SetSettingInput {
        scope: "pricing".into(),
        key: "sync".into(),
        value,
    })
    .await
    {
        tracing::warn!(error = %e, "save price sync settings: db write failed");
    }
}

/// Check if auto sync is due and run it if needed.
/// Called periodically from the proxy loop or on startup.
#[allow(dead_code)]
pub async fn maybe_auto_sync(db: &Db) -> Result<Option<PriceSyncResult>, String> {
    let settings = get_sync_settings(db).await;
    if !settings.auto_sync_enabled {
        return Ok(None);
    }
    let now = super::db::now();
    let interval_ms = (settings.sync_interval_secs as i64) * 1000;
    if settings.last_sync_at > 0 && (now - settings.last_sync_at) < interval_ms {
        return Ok(None);
    }
    let result = sync_litellm_prices(db).await?;
    Ok(Some(result))
}
