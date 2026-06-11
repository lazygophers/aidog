//! LiteLLM price table sync: fetch, parse, upsert into model_price table.

use super::db::Db;
use super::models::PriceSyncResult;

/// LiteLLM public price table URL
const LITELLM_PRICE_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// Fetch and parse the LiteLLM price table, then upsert all entries.
pub async fn sync_litellm_prices(db: &Db) -> Result<PriceSyncResult, String> {
    let json_str = fetch_price_table().await?;
    let table: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&json_str).map_err(|e| format!("parse litellm json: {e}"))?;

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
        let existing = super::db::get_model_price(db, model_name).ok().flatten();
        let is_new = existing.is_none() || existing.as_ref().map(|e| e.source.as_str()) != Some("litellm");

        match super::db::upsert_model_price(db, model_name, "litellm", &price_json) {
            Ok(()) => {
                if is_new { added += 1; } else { updated += 1; }
            }
            Err(_) => { failed += 1; }
        }
    }

    // Update last_sync_at in settings
    let sync_settings = get_sync_settings(db);
    let updated_settings = super::models::PriceSyncSettings {
        last_sync_at: super::db::now(),
        ..sync_settings
    };
    save_sync_settings(db, &updated_settings);

    Ok(PriceSyncResult { added, updated, unchanged, failed, total })
}

async fn fetch_price_table() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("build http client: {e}"))?;

    let resp = client.get(LITELLM_PRICE_URL)
        .send()
        .await
        .map_err(|e| format!("fetch litellm prices: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("litellm returned status {}", resp.status()));
    }

    resp.text().await.map_err(|e| format!("read litellm response: {e}"))
}

/// Read sync settings from DB
pub fn get_sync_settings(db: &Db) -> super::models::PriceSyncSettings {
    super::db::get_setting(db, "pricing", "sync")
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Save sync settings to DB
pub fn save_sync_settings(db: &Db, settings: &super::models::PriceSyncSettings) {
    let value = match serde_json::to_value(settings) {
        Ok(v) => v,
        Err(_) => return,
    };
    let _ = super::db::set_setting(db, super::models::SetSettingInput {
        scope: "pricing".into(),
        key: "sync".into(),
        value,
    });
}

/// Check if auto sync is due and run it if needed.
/// Called periodically from the proxy loop or on startup.
#[allow(dead_code)]
pub async fn maybe_auto_sync(db: &Db) -> Result<Option<PriceSyncResult>, String> {
    let settings = get_sync_settings(db);
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
