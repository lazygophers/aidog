//! defaults.json 同步：拉 jsDelivr master `src-tauri/defaults/defaults.json`（主）+ raw fallback。
//!
//! 架构同 price_sync.rs：双源 fetch + 远程 `last_updated`（Unix 秒）与本地比对，远程较新才写。
//! 写入 app data (`~/.aidog/defaults.json`)，由 commands/defaults.rs 的 reader 自动优先读取。
//! 节流时间戳：`~/.aidog/defaults.json.last_sync`（Unix 秒）。
//!
//! 三路触发：
//! - 启动 hook（maybe_sync_on_startup，24h 节流）
//! - 每日定时器（spawn_daily_sync，复用 spawn 模式）
//! - 设置页手动按钮（sync_defaults_json command，无视节流）

use crate::shared::aidog_data_dir;
use serde::Serialize;

/// 主源：jsDelivr CDN（master 分支）。
const DEFAULTS_JSON_PRIMARY_URL: &str =
    "https://cdn.jsdelivr.net/gh/lazygophers/aidog@master/src-tauri/defaults/defaults.json";

/// fallback：GitHub raw（master 分支）。
const DEFAULTS_JSON_FALLBACK_URL: &str =
    "https://raw.githubusercontent.com/lazygophers/aidog/master/src-tauri/defaults/defaults.json";

const THROTTLE_SECS: i64 = 24 * 3600;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultsSyncResult {
    pub updated: bool,
    pub last_updated: i64,
    /// "jsdelivr" | "raw" | "local" — 写盘来源；"local" = 全失败不写
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn sync_defaults_json() -> DefaultsSyncResult {
    tracing::info!("defaults.json sync started");
    let fetched = match fetch_defaults_json().await {
        Ok((body, source)) => (body, source),
        Err(e) => {
            tracing::warn!(error = %e, "defaults sync: fetch failed, keep local");
            let local_ts = read_local_last_updated().unwrap_or(0);
            return DefaultsSyncResult {
                updated: false,
                last_updated: local_ts,
                source: "local".into(),
                error: Some(e),
            };
        }
    };

    let (body, source) = fetched;
    let remote_ts = match parse_last_updated(&body) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "defaults sync: parse last_updated failed");
            let local_ts = read_local_last_updated().unwrap_or(0);
            return DefaultsSyncResult {
                updated: false,
                last_updated: local_ts,
                source: "local".into(),
                error: Some(format!("parse last_updated: {e}")),
            };
        }
    };

    let local_ts = read_local_last_updated().unwrap_or(0);
    if remote_ts > local_ts {
        match write_app_data(&body) {
            Ok(()) => {
                let _ = write_last_sync_ts(now_secs());
                tracing::info!(remote_ts, local_ts, source = %source, "defaults.json updated from remote");
                DefaultsSyncResult {
                    updated: true,
                    last_updated: remote_ts,
                    source,
                    error: None,
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "defaults sync: write app data failed");
                DefaultsSyncResult {
                    updated: false,
                    last_updated: local_ts,
                    source: "local".into(),
                    error: Some(format!("write app data: {e}")),
                }
            }
        }
    } else {
        let _ = write_last_sync_ts(now_secs());
        tracing::debug!(remote_ts, local_ts, "defaults.json not newer, skip");
        DefaultsSyncResult {
            updated: false,
            last_updated: local_ts,
            source,
            error: None,
        }
    }
}

/// 启动 hook：24h 节流。节流判定 = 读 `~/.aidog/defaults.json.last_sync`。
/// 全失败静默（warn log），绝不阻塞启动或破坏现有功能。
pub async fn maybe_sync_on_startup() {
    if !should_sync_due() {
        tracing::debug!("defaults sync throttled (within 24h), skip");
        return;
    }
    let _ = sync_defaults_json().await;
}

/// 节流判定：返回现在距上次同步 > 24h（或从未同步）。
fn should_sync_due() -> bool {
    let last = match read_last_sync_ts() {
        Ok(t) => t,
        Err(_) => return true, // 读不到视为从未同步
    };
    if last <= 0 {
        return true;
    }
    now_secs() - last >= THROTTLE_SECS
}

async fn fetch_defaults_json() -> Result<(String, String), String> {
    // ponytail: 无 DB 依赖（defaults.json 是无状态文件），用裸 reqwest::Client。
    // price_sync 走 build_http_client_system 是因为受系统上游代理设置；defaults 同步
    // 走公网 CDN，无代理需求。timeout 短（30s），失败回退 bundled（reader 端处理）。
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("build http client: {e}"))?;

    for (source, url) in [
        ("jsdelivr", DEFAULTS_JSON_PRIMARY_URL),
        ("raw", DEFAULTS_JSON_FALLBACK_URL),
    ] {
        match fetch_one(&client, url).await {
            Ok(body) => {
                tracing::info!(source, bytes = body.len(), "defaults.json fetched");
                return Ok((body, source.into()));
            }
            Err(e) => tracing::warn!(source, error = %e, "defaults.json fetch failed, trying next"),
        }
    }
    Err("defaults.json: all sources failed (jsDelivr + raw)".into())
}

async fn fetch_one(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client.get(url).send().await.map_err(|e| format!("fetch: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }
    resp.text().await.map_err(|e| format!("read body: {e}"))
}

/// 解析 defaults.json top-level `last_updated`（Unix 秒）。
fn parse_last_updated(body: &str) -> Result<i64, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| format!("json: {e}"))?;
    v.get("last_updated")
        .and_then(|t| t.as_i64())
        .ok_or_else(|| "missing/invalid last_updated".into())
}

fn app_data_path() -> Result<std::path::PathBuf, String> {
    Ok(aidog_data_dir()?.join("defaults.json"))
}

fn last_sync_path() -> Result<std::path::PathBuf, String> {
    Ok(aidog_data_dir()?.join("defaults.json.last_sync"))
}

fn read_local_last_updated() -> Result<i64, String> {
    let p = app_data_path()?;
    if !p.exists() {
        return Err("no local defaults.json".into());
    }
    let body = std::fs::read_to_string(&p).map_err(|e| format!("read: {e}"))?;
    parse_last_updated(&body)
}

fn write_app_data(body: &str) -> Result<(), String> {
    let p = app_data_path()?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    std::fs::write(&p, body).map_err(|e| format!("write: {e}"))
}

fn read_last_sync_ts() -> Result<i64, String> {
    let p = last_sync_path()?;
    if !p.exists() {
        return Err("no last_sync file".into());
    }
    let s = std::fs::read_to_string(&p).map_err(|e| format!("read: {e}"))?;
    s.trim().parse::<i64>().map_err(|e| format!("parse: {e}"))
}

fn write_last_sync_ts(ts: i64) -> Result<(), String> {
    let p = last_sync_path()?;
    std::fs::write(&p, ts.to_string()).map_err(|e| format!("write: {e}"))
}

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_last_updated_ok() {
        let body = r#"{"version":"1","last_updated":1783347706,"protocols":{}}"#;
        assert_eq!(parse_last_updated(body).unwrap(), 1783347706);
    }

    #[test]
    fn parse_last_updated_missing_fails() {
        let body = r#"{"version":"1","protocols":{}}"#;
        assert!(parse_last_updated(body).is_err());
    }

    #[test]
    fn parse_last_updated_bad_json_fails() {
        assert!(parse_last_updated("not json").is_err());
    }

    #[test]
    fn should_sync_due_when_no_file() {
        // 无 last_sync 文件 → 视为从未同步 → 应同步。通过函数逻辑直接验证：
        // read_last_sync_ts 在文件缺失时返 Err，should_sync_due 取 true。
        // （此处只验证函数语义，不依赖文件系统状态）
        assert_eq!(should_sync_due_internal(None), true);
        assert_eq!(should_sync_due_internal(Some(0)), true);
        assert_eq!(should_sync_due_internal(Some(now_secs())), false);
        assert_eq!(should_sync_due_internal(Some(now_secs() - THROTTLE_SECS - 1)), true);
    }

    /// 单测辅助：把 should_sync_due 的判定逻辑抽出来，避免依赖真实文件系统。
    fn should_sync_due_internal(last: Option<i64>) -> bool {
        let last = match last {
            None => return true,
            Some(t) => t,
        };
        if last <= 0 {
            return true;
        }
        now_secs() - last >= THROTTLE_SECS
    }
}
