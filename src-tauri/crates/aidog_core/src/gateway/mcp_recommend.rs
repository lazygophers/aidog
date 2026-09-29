//! MCP 推荐清单远程拉取与缓存（mcp-recommend 票 08）。
//!
//! 数据源 = jsDelivr master 主 + raw.githubusercontent 兜底（同 price_sync 双源 idiom），
//! 单文件无并发池。缓存 = DB settings scope `mcp` key `recommended`（值 = 清单 JSON +
//! `fetchedAt`），TTL 24h 内跳过拉取；拉到的 `updated_at` ≤ 缓存不覆盖；
//! `schema_version` 不认识整单拒收。失败一律静默：保留缓存/内置兜底，无 UI 提示。

use aidog_db::Db;
use aidog_mcp::{RecommendedEntry, RecommendedManifest};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 主源：jsDelivr CDN（master 分支），与 price_sync 的 registry 同一条维护流水线。
const MCP_RECOMMEND_PRIMARY_BASE: &str =
    "https://cdn.jsdelivr.net/gh/lazygophers/aidog@master/src-tauri/defaults/mcp";

/// fallback：GitHub raw（master 分支），jsDelivr 不可达时兜底。
const MCP_RECOMMEND_FALLBACK_BASE: &str =
    "https://raw.githubusercontent.com/lazygophers/aidog/master/src-tauri/defaults/mcp";

/// 本客户端认识的清单 schema 版本；不认识 → 整单拒收、保留旧缓存（前向保护）。
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// 缓存 TTL：24h。清单低频变化且 jsDelivr 有边缘缓存窗口，更激进只会拿同样字节。
const CACHE_TTL_MS: i64 = 24 * 3600 * 1000;

/// settings 定位：scope `mcp`、key `recommended`（值 = [`CacheDoc`] 的 camelCase JSON）。
const SETTING_SCOPE: &str = "mcp";
const SETTING_KEY: &str = "recommended";

// ─── 清单 schema ──────────────────────────────────────────
// 类型在 aidog_mcp::recommended（票 07）：`RecommendedManifest` / `RecommendedEntry`，
// 经 `gateway::mcp::X` shim 同路径可达。installed 态由前端对齐 mcp_list（spec §2.2）。

/// settings 缓存值：清单本体 + 拉取时间戳（Unix 毫秒）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheDoc {
    fetched_at: i64,
    manifest: RecommendedManifest,
}

// ─── 拉取（启动后台预取入口） ──────────────────────────────

/// 拉取推荐清单并按需入库。返回 `true` = 写入了新版本。
/// TTL 内直接跳过；失败返回 Err，调用方记日志即可（票 04：静默，无 UI）。
pub async fn sync_mcp_recommended(db: &Db) -> Result<bool, String> {
    sync_from(db, &[MCP_RECOMMEND_PRIMARY_BASE, MCP_RECOMMEND_FALLBACK_BASE]).await
}

/// [`sync_mcp_recommended`] 的可注入源版本（测试用本地 stub server 当 base）。
async fn sync_from(db: &Db, bases: &[&str]) -> Result<bool, String> {
    let cache = read_cache(db).await;
    // TTL 早退：缓存还很新就不发请求。
    if let Some(c) = &cache
        && c.fetched_at > 0
        && aidog_db::now() - c.fetched_at < CACHE_TTL_MS
    {
        return Ok(false);
    }

    let body = fetch_with_fallback(db, bases, "recommended.json").await?;
    let manifest: RecommendedManifest = match serde_json::from_str(&body) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "mcp recommended sync: bad json, rejected");
            bump_fetched_at(db, cache).await;
            return Ok(false);
        }
    };
    if manifest.schema_version != SUPPORTED_SCHEMA_VERSION {
        tracing::warn!(
            version = manifest.schema_version,
            supported = SUPPORTED_SCHEMA_VERSION,
            "mcp recommended sync: unknown schema_version, rejected"
        );
        bump_fetched_at(db, cache).await;
        return Ok(false);
    }

    // 版本比对：远程不比缓存新就只刷新 fetched_at（刚拉过，TTL 重置防启动空转），
    // 清单本体保留旧的。
    if let Some(c) = &cache
        && manifest.updated_at <= c.manifest.updated_at
    {
        save_cache(
            db,
            &CacheDoc {
                fetched_at: aidog_db::now(),
                manifest: c.manifest.clone(),
            },
        )
        .await;
        return Ok(false);
    }

    save_cache(db, &CacheDoc {
        fetched_at: aidog_db::now(),
        manifest,
    })
    .await;
    Ok(true)
}

// ─── 命令读路径 ────────────────────────────────────────────

/// `mcp_recommended_list` 的实现：远程缓存 → 编译期内置兜底。
/// installed 态不在后端关联（spec §2.2：前端自行对齐 mcp_list 结果）。
pub async fn recommended_list(db: &Db) -> Result<Vec<RecommendedEntry>, String> {
    let manifest = match read_cache(db).await {
        Some(c) => c.manifest,
        None => parse_bundled()?,
    };
    Ok(manifest.entries)
}

/// 解析编译期内置清单（票 07：`defaults/mcp/recommended.json` include_str!）。
fn parse_bundled() -> Result<RecommendedManifest, String> {
    serde_json::from_str(aidog_mcp::bundled_mcp_recommended())
        .map_err(|e| format!("bundled mcp recommended manifest: {e}"))
}

// ─── 缓存读写（settings，同 price_sync 的 PriceSyncSettings idiom） ──

async fn read_cache(db: &Db) -> Option<CacheDoc> {
    let raw = match aidog_db::get_setting(db, SETTING_SCOPE, SETTING_KEY).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "mcp recommended cache read failed, fallback to bundled");
            None
        }
    };
    raw.and_then(|v| serde_json::from_value(v).ok())
}

async fn save_cache(db: &Db, doc: &CacheDoc) {
    let value = match serde_json::to_value(doc) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "save mcp recommended cache: serialize failed");
            return;
        }
    };
    if let Err(e) = aidog_db::set_setting(
        db,
        aidog_db::SetSettingInput {
            scope: SETTING_SCOPE.into(),
            key: SETTING_KEY.into(),
            value,
        },
    )
    .await
    {
        tracing::warn!(error = %e, "save mcp recommended cache: db write failed");
    }
}

/// 「拉取完成但清单不变/拒收」路径：只刷新 fetched_at，清单用原缓存
/// （无缓存且拒收 → 无可保存，下次启动重试，单文件开销可忽略）。
async fn bump_fetched_at(db: &Db, cache: Option<CacheDoc>) {
    if let Some(c) = cache {
        save_cache(
            db,
            &CacheDoc {
                fetched_at: aidog_db::now(),
                manifest: c.manifest,
            },
        )
        .await;
    }
}

// ─── HTTP（fetch_with_fallback idiom，抄 price_sync.rs 单文件版） ──

async fn fetch_with_fallback(db: &Db, bases: &[&str], path: &str) -> Result<String, String> {
    let db_arc = Arc::new(db.clone());
    // 单文件走 logo_sync 的 20s/10s 档（price_sync 的 30s 是给千文件整轮的）。
    let client = super::http_client::build_http_client_system(&db_arc, 20, 10).await;
    let mut last = "no source configured".to_string();
    for base in bases {
        match fetch_one(&client, &format!("{base}/{path}")).await {
            Ok(body) => return Ok(body),
            Err(e) => {
                tracing::debug!(%path, base, error = %e, "mcp recommended fetch failed, trying next source");
                last = e;
            }
        }
    }
    Err(last)
}

async fn fetch_one(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("fetch: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }
    resp.text().await.map_err(|e| format!("read body: {e}"))
}

#[cfg(test)]
#[path = "test_mcp_recommend.rs"]
mod test_mcp_recommend;
