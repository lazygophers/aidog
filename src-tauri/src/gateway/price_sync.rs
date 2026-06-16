//! GitHub models.json 同步：拉取 data/models.json（Python 聚合的唯一信源），解析，upsert 入 model_price。
//!
//! 数据源 = raw.githubusercontent.com/lazygophers/aidog/master/data/models.json
//! schema 见 scripts/pricing/schema.py（ModelsFile / ModelEntry / PlatformPricing）。

use super::db::Db;
use super::models::PriceSyncResult;
use std::sync::Arc;

/// 唯一信源：仓内 data/models.json 的 GitHub raw URL（master 分支）。
const MODELS_JSON_URL: &str =
    "https://raw.githubusercontent.com/lazygophers/aidog/master/data/models.json";

/// Fetch + parse data/models.json，upsert 全部模型（source="github"）。
///
/// 后台周期同步的每轮入口：建独立 trace_id span（非请求触发），本轮所有日志
/// 自动带 price_sync{trace_id=xxxxxxxx} 前缀，可按 id grep 出完整一轮同步。
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn sync_github_prices(db: &Db) -> Result<PriceSyncResult, String> {
    tracing::info!("github models.json sync started");
    let db_arc = Arc::new(db.clone());
    let json_str = match fetch_models_json(Some(&db_arc)).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "github price sync: fetch failed");
            return Err(e);
        }
    };
    let root: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "github price sync: parse json failed");
            return Err(format!("parse models.json: {e}"));
        }
    };
    let models = match root.get("models").and_then(|m| m.as_object()) {
        Some(m) => m,
        None => {
            tracing::error!("github price sync: missing top-level `models` object");
            return Err("models.json: missing `models` object".into());
        }
    };

    let mut added = 0u32;
    let mut updated = 0u32;
    let unchanged = 0u32;
    let mut failed = 0u32;
    let total = models.len() as u32;

    for (model_name, entry) in models {
        if model_name.is_empty() {
            continue;
        }
        // price_data = 完整 entry JSON（resolve_price 解析 input/output/cache_read + pricing[platform] + default_platform）
        let price_json = match serde_json::to_string(entry) {
            Ok(s) => s,
            Err(_) => { failed += 1; continue; }
        };
        let max_in = entry.get("max_input_tokens").and_then(|v| v.as_i64());
        let max_out = entry.get("max_output_tokens").and_then(|v| v.as_i64());
        let ctx = entry.get("context_window").and_then(|v| v.as_i64());

        let existing = super::db::get_model_price(db, model_name).await.ok().flatten();
        let is_new = existing.is_none() || existing.as_ref().map(|e| e.source.as_str()) != Some("github");

        match super::db::upsert_model_price(db, model_name, "github", &price_json, max_in, max_out, ctx).await {
            Ok(()) => {
                if is_new { added += 1; } else { updated += 1; }
            }
            Err(e) => {
                tracing::warn!(model = %model_name, error = %e, "upsert model price failed");
                failed += 1;
            }
        }
    }

    // Update last_sync_at in settings
    let sync_settings = get_sync_settings(db).await;
    let updated_settings = super::models::PriceSyncSettings {
        last_sync_at: super::db::now(),
        ..sync_settings
    };
    save_sync_settings(db, &updated_settings).await;

    tracing::info!(added, updated, unchanged, failed, total, "github models.json sync completed");
    Ok(PriceSyncResult { added, updated, unchanged, failed, total })
}

async fn fetch_models_json(db: Option<&Arc<Db>>) -> Result<String, String> {
    let client = match db {
        Some(db) => super::http_client::build_http_client_system(db, 30, 10).await,
        None => reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("build http client: {e}"))?,
    };

    let resp = client.get(MODELS_JSON_URL)
        .send()
        .await
        .map_err(|e| format!("fetch models.json: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        tracing::warn!(%status, "models.json fetch: non-success status");
        return Err(format!("models.json returned status {status}"));
    }

    resp.text().await.map_err(|e| {
        tracing::warn!(error = %e, "models.json fetch: read response body failed");
        format!("read models.json response: {e}")
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
    let result = sync_github_prices(db).await?;
    Ok(Some(result))
}
