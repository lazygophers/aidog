//! registry 远程同步：拉 `src-tauri/defaults/registry/`（人工维护的唯一真值源）逐文件入库。
//!
//! 数据源 = jsDelivr master 主 + raw.githubusercontent 兜底，**每个文件各自两源回退**。
//! 流程是 index 驱动：先拉 `index.json`（失败即整轮放弃），再照它的清单逐文件拉
//! `platforms/<code>/platform.json` 与 `platforms/<code>/models/<model>.json`，
//! 成功的 upsert 进 `platform_preset` / `model_entry`，失败的记进 `failures` 清单并
//! **保留 DB 旧行**（best-effort，不清空、不部分覆盖），单文件失败不阻塞整轮。

use super::models::{ModelEntry, PlatformPreset, PriceSyncResult, SyncFailure};
use aidog_db::Db;
use futures::stream::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;

/// 主源：jsDelivr CDN（master 分支）。CDN 加速 + 边缘缓存，失败/非 200 回退 raw。
const REGISTRY_PRIMARY_BASE: &str =
    "https://cdn.jsdelivr.net/gh/lazygophers/aidog@master/src-tauri/defaults/registry";

/// fallback：GitHub raw（master 分支）。jsDelivr 不可达时兜底。
const REGISTRY_FALLBACK_BASE: &str =
    "https://raw.githubusercontent.com/lazygophers/aidog/master/src-tauri/defaults/registry";

/// 单轮并发拉取上限。registry 约 1000 个小文件，串行会跑到分钟级；
/// 16 路对 CDN 友好且把整轮压到十秒量级。
const FETCH_CONCURRENCY: usize = 16;

/// bundled registry（同一份人工维护信源，编译期内嵌）。DB 未同步时的只读兜底。
pub use aidog_db::registry::model_entry as bundled_model_entry;

/// 一个待拉文件：registry 内相对路径 + 它归属的平台 code。
struct Job {
    platform_code: String,
    path: String,
    is_platform: bool,
}

/// 拉取整份 registry 并 upsert 入库。
///
/// 后台周期同步的每轮入口：建独立 trace_id span（非请求触发），本轮所有日志
/// 自动带 price_sync{trace_id=xxxxxxxx} 前缀，可按 id grep 出完整一轮同步。
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn sync_registry(db: &Db) -> Result<PriceSyncResult, String> {
    sync_registry_from(db, &[REGISTRY_PRIMARY_BASE, REGISTRY_FALLBACK_BASE]).await
}

/// [`sync_registry`] 的可注入源版本（测试用本地 stub server 当 base）。
/// `bases` 按序回退，全败才算该文件失败。
async fn sync_registry_from(db: &Db, bases: &[&str]) -> Result<PriceSyncResult, String> {
    tracing::info!("registry sync started");
    let db_arc = Arc::new(db.clone());
    let client = super::http_client::build_http_client_system(&db_arc, 30, 10).await;

    // index 拉不到 = 不知道该拉哪些文件，整轮放弃（DB 全量保留）。
    let index_json = fetch_with_fallback(&client, bases, "index.json")
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "registry sync: index.json fetch failed");
            format!("index.json: {e}")
        })?;
    let index = aidog_db::registry::parse_index(&index_json)?;

    let jobs: Vec<Job> = index
        .iter()
        .flat_map(|e| {
            let platform = e.platform_file.iter().map(|p| Job {
                platform_code: e.code.clone(),
                path: p.clone(),
                is_platform: true,
            });
            let models = e.models.iter().map(|m| Job {
                platform_code: e.code.clone(),
                path: format!("{}/{m}", e.models_dir),
                is_platform: false,
            });
            platform.chain(models).collect::<Vec<_>>()
        })
        .collect();
    let total = jobs.len() as u32;

    let fetched: Vec<(Job, Result<String, String>)> = futures::stream::iter(jobs)
        .map(|job| {
            let client = &client;
            async move {
                let body = fetch_with_fallback(client, bases, &job.path).await;
                (job, body)
            }
        })
        .buffer_unordered(FETCH_CONCURRENCY)
        .collect()
        .await;

    let mut failures: Vec<SyncFailure> = Vec::new();
    let mut presets: Vec<PlatformPreset> = Vec::new();
    let mut entries: Vec<ModelEntry> = Vec::new();
    for (job, body) in fetched {
        let body = match body {
            Ok(b) => b,
            Err(error) => {
                tracing::warn!(file = %job.path, %error, "registry sync: file failed, keeping db row");
                failures.push(SyncFailure { file: job.path, error });
                continue;
            }
        };
        if job.is_platform {
            // 解析不过的 JSON 不入库：宁可保留 DB 旧的品牌字段，也不写一份读不出名字的脏行。
            match serde_json::from_str::<serde_json::Value>(&body) {
                Ok(v) if v.is_object() => presets.push(PlatformPreset {
                    code: job.platform_code,
                    preset_data: body,
                    updated_at: 0,
                }),
                _ => failures.push(SyncFailure { file: job.path, error: "invalid platform json".into() }),
            }
        } else {
            match aidog_db::model_entry_from_json(&job.platform_code, &body) {
                Some(e) => entries.push(e),
                None => failures.push(SyncFailure { file: job.path, error: "invalid model json".into() }),
            }
        }
    }

    let (mut added, mut updated, mut unchanged) = (0u32, 0u32, 0u32);

    let old_presets: HashMap<String, String> = aidog_db::select_platform_presets(db)
        .await?
        .into_iter()
        .map(|p| (p.code, p.preset_data))
        .collect();
    let presets: Vec<PlatformPreset> = presets
        .into_iter()
        .filter(|p| match old_presets.get(&p.code) {
            None => { added += 1; true }
            Some(old) if *old == p.preset_data => { unchanged += 1; false }
            Some(_) => { updated += 1; true }
        })
        .collect();

    let old_entries: HashMap<(String, String), String> = aidog_db::select_model_entries(db, None)
        .await?
        .into_iter()
        .map(|e| ((e.platform_code, e.model_id), e.price_data))
        .collect();
    let entries: Vec<ModelEntry> = entries
        .into_iter()
        .filter(|e| match old_entries.get(&(e.platform_code.clone(), e.model_id.clone())) {
            None => { added += 1; true }
            Some(old) if *old == e.price_data => { unchanged += 1; false }
            Some(_) => { updated += 1; true }
        })
        .collect();

    aidog_db::upsert_platform_presets(db, presets).await?;
    aidog_db::upsert_model_entries(db, entries).await?;

    let sync_settings = get_sync_settings(db).await;
    save_sync_settings(
        db,
        &super::models::PriceSyncSettings { last_sync_at: aidog_db::now(), ..sync_settings },
    )
    .await;

    let failed = failures.len() as u32;
    tracing::info!(added, updated, unchanged, failed, total, "registry sync completed");
    Ok(PriceSyncResult { added, updated, unchanged, failed, total, failures })
}

/// 逐源回退拉单个 registry 文件（`path` 是 registry 内相对路径）。全部源失败才返 Err。
async fn fetch_with_fallback(client: &reqwest::Client, bases: &[&str], path: &str) -> Result<String, String> {
    let mut last = "no source configured".to_string();
    for base in bases {
        match fetch_one(client, &format!("{base}/{path}")).await {
            Ok(body) => return Ok(body),
            Err(e) => {
                tracing::debug!(%path, base, error = %e, "registry fetch failed, trying next source");
                last = e;
            }
        }
    }
    Err(last)
}

async fn fetch_one(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client.get(url).send().await.map_err(|e| format!("fetch: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }
    resp.text().await.map_err(|e| format!("read body: {e}"))
}

/// Read sync settings from DB
pub async fn get_sync_settings(db: &Db) -> super::models::PriceSyncSettings {
    aidog_db::get_setting(db, "pricing", "sync")
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
    if let Err(e) = aidog_db::set_setting(db, super::models::SetSettingInput {
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
pub async fn maybe_auto_sync(db: &Db) -> Result<Option<PriceSyncResult>, String> {
    let settings = get_sync_settings(db).await;
    if !settings.auto_sync_enabled {
        return Ok(None);
    }
    let now = aidog_db::now();
    let interval_ms = (settings.sync_interval_secs as i64) * 1000;
    if settings.last_sync_at > 0 && (now - settings.last_sync_at) < interval_ms {
        return Ok(None);
    }
    let result = sync_registry(db).await?;
    Ok(Some(result))
}

#[cfg(test)]
#[path = "test_price_sync.rs"]
mod test_price_sync;
