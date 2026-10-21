//! registry 灌种：编译期内置（`src-tauri/defaults/registry/`，include_str! 打包）upsert 进
//! `platform_preset` / `model_entry` 镜像表。2026-10-21 起远程同步已删（数据真值源 =
//! 构建内打包，不允许运行时更新）：本模块在内核启动时跑一次，内容未变的行跳过，
//! 清单里已不存在的旧行 prune 掉（内置清单即全量真值，无部分失败顾虑）。

use super::models::{ModelEntry, PlatformPreset, PriceSyncResult, SyncFailure};
use aidog_db::Db;
use std::collections::{HashMap, HashSet};

/// 启动灌种：bundled → diff 镜像表 → upsert 变更行 + prune 失效行 → 刷新 preset 缓存。
/// 有实际变更时 emit "registry-updated"（前端模型信息页 / defaults docPromise 失效重拉）。
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn seed_registry_from_bundled(db: &Db) -> Result<PriceSyncResult, String> {
    tracing::info!("registry seed from bundled started");

    let mut failures: Vec<SyncFailure> = Vec::new();
    let presets: Vec<PlatformPreset> = aidog_db::registry::bundled_platform_files()
        .iter()
        .map(|(code, json)| PlatformPreset {
            code: (*code).to_string(),
            preset_data: (*json).to_string(),
            updated_at: 0,
        })
        .collect();
    let entries: Vec<ModelEntry> = aidog_db::registry::bundled_model_files()
        .iter()
        .filter_map(|(code, _file, json)| aidog_db::model_entry_from_json(code, json))
        .collect();
    // include_str! 的源文件过得了 check-registry 门禁才会进构建；这里解析失败只可能
    // 是形状漂移（门禁没拦住的 bug），逐条记 failures 不静默吞。
    let bundled_model_total = aidog_db::registry::bundled_model_files().len();
    if entries.len() != bundled_model_total {
        failures.push(SyncFailure {
            file: "bundled model files".into(),
            error: format!(
                "{} of {} bundled model files parsed",
                entries.len(),
                bundled_model_total
            ),
        });
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
            None => {
                added += 1;
                true
            }
            Some(old) if *old == p.preset_data => {
                unchanged += 1;
                false
            }
            Some(_) => {
                updated += 1;
                true
            }
        })
        .collect();

    let old_entries: HashMap<(String, String), String> = aidog_db::select_model_entries(db, None)
        .await?
        .into_iter()
        .map(|e| ((e.platform_code, e.model_id), e.price_data))
        .collect();
    // 内置清单是镜像表的真值；键集在 entries 被过滤消费前收齐（prune 不删仍在清单里的行）。
    let bundled_model_keys: HashSet<(String, String)> = entries
        .iter()
        .map(|e| (e.platform_code.clone(), e.model_id.clone()))
        .collect();
    let entries: Vec<ModelEntry> = entries
        .into_iter()
        .filter(
            |e| match old_entries.get(&(e.platform_code.clone(), e.model_id.clone())) {
                None => {
                    added += 1;
                    true
                }
                Some(old) if *old == e.price_data => {
                    unchanged += 1;
                    false
                }
                Some(_) => {
                    updated += 1;
                    true
                }
            },
        )
        .collect();

    let bundled_preset_codes: HashSet<String> = presets_all_codes();
    failures.extend(write_failures(
        "platform_preset",
        aidog_db::upsert_platform_presets(db, presets).await,
    ));
    failures.extend(write_failures(
        "model_entry",
        aidog_db::upsert_model_entries_best_effort(db, entries).await,
    ));
    // 内置清单即全量：镜像表里不在清单中的行 = 上一版二进制的残留，prune（计数在删前收）。
    let pruned: u32 = (old_entries.keys().filter(|k| !bundled_model_keys.contains(*k)).count()
        + old_presets.keys().filter(|c| !bundled_preset_codes.contains(*c)).count())
        as u32;
    if let Err(error) = aidog_db::prune_model_entries(db, &bundled_model_keys).await {
        failures.push(SyncFailure { file: "model_entry".into(), error });
    }
    if let Err(error) = aidog_db::prune_platform_presets(db, &bundled_preset_codes).await {
        failures.push(SyncFailure { file: "platform_preset".into(), error });
    }
    // preset 写入作废了热路径缓存，这里把新值装回去（路由/计费与前端读同一份）。
    if let Err(e) = aidog_db::refresh_presets_cache(db).await {
        tracing::warn!(error = %e, "registry seed: refresh presets cache failed");
    }

    // 诊断戳：灌种完成的 registry 版本（bundled index last_updated）。
    let mut settings = get_sync_settings(db).await;
    settings.registry_last_updated = aidog_db::registry::presets()
        .get("last_updated")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(settings.registry_last_updated);
    save_sync_settings(db, &settings).await;

    let total = (aidog_db::registry::bundled_platform_files().len() + bundled_model_total) as u32;
    tracing::info!(
        added,
        updated,
        unchanged,
        failed = failures.len(),
        pruned,
        total,
        "registry seed from bundled completed"
    );
    if added + updated + pruned > 0 {
        aidog_ctx::emit_unit("registry-updated");
    }
    Ok(PriceSyncResult {
        added,
        updated,
        unchanged,
        failed: failures.len() as u32,
        total,
        failures,
    })
}

fn presets_all_codes() -> HashSet<String> {
    aidog_db::registry::bundled_platform_files()
        .iter()
        .map(|(code, _)| (*code).to_string())
        .collect()
}

/// DB 写入结果 → 失败清单：整个事务打不开记一条（`what` 定位到表），单行失败逐条记。
/// 返回空 = 全部写成功。
fn write_failures(
    what: &str,
    r: Result<(u32, Vec<aidog_db::WriteFailure>), String>,
) -> Vec<SyncFailure> {
    match r {
        Ok((_, rows)) => {
            for (file, error) in &rows {
                tracing::warn!(%file, %error, "registry seed: db row write failed");
            }
            rows.into_iter()
                .map(|(file, error)| SyncFailure { file, error })
                .collect()
        }
        Err(error) => {
            tracing::error!(table = what, %error, "registry seed: db write failed");
            vec![SyncFailure {
                file: what.to_string(),
                error,
            }]
        }
    }
}

/// Read price settings from DB
pub async fn get_sync_settings(db: &Db) -> super::models::PriceSyncSettings {
    aidog_db::get_setting(db, "pricing", "sync")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Save price settings to DB
pub async fn save_sync_settings(db: &Db, settings: &super::models::PriceSyncSettings) {
    let value = match serde_json::to_value(settings) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "save price settings: serialize failed");
            return;
        }
    };
    if let Err(e) = aidog_db::set_setting(
        db,
        super::models::SetSettingInput {
            scope: "pricing".into(),
            key: "sync".into(),
            value,
        },
    )
    .await
    {
        tracing::warn!(error = %e, "save price settings: db write failed");
    }
}

#[cfg(test)]
#[path = "test_price_sync.rs"]
mod test_price_sync;
