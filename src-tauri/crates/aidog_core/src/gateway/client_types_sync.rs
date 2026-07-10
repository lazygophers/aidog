//! client-types.json 同步：拉 jsDelivr master `src-tauri/defaults/client-types.json`（主）+ raw fallback。
//!
//! 架构同 `defaults_sync.rs`（platform-presets）：双源 fetch + 远程 `last_updated`（Unix 秒）与本地比对，
//! 远程较新才写。写入 app data (`~/.aidog/client-types.json`)，由 `commands/defaults.rs::get_client_types_json`
//! reader 自动优先读取（缺失/损坏/schema gate 失败回退 bundled）。节流时间戳：
//! `~/.aidog/client-types.json.last_sync`（Unix 秒）。
//!
//! 三路触发：
//! - 启动 hook（maybe_sync_on_startup，24h 节流 + 用户定制保护）
//! - 每日定时器（spawn_daily_sync，复用 spawn 模式）
//! - 设置页手动按钮（sync_client_types_json command，无视节流）
//!
//! **schema gate**：写盘前 `validate_structure` 校验远端 body（client_types 数组 + 每 entry value/group/name 关键字段），
//! 失败拒绝写入保留本地。
//! **用户定制保护**：成功同步后写 `.hash` 快照（sha256 of body）；启动 hook 检测 app data 被手工修改则跳过自动同步；
//! 手动按钮强制覆盖 + 重置快照。

use crate::shared::aidog_data_dir;
use serde::Serialize;
use std::sync::OnceLock;

/// 主源：jsDelivr CDN（master 分支）。
const CLIENT_TYPES_JSON_PRIMARY_URL: &str =
    "https://cdn.jsdelivr.net/gh/lazygophers/aidog@master/src-tauri/defaults/client-types.json";

/// fallback：GitHub raw（master 分支）。
const CLIENT_TYPES_JSON_FALLBACK_URL: &str =
    "https://raw.githubusercontent.com/lazygophers/aidog/master/src-tauri/defaults/client-types.json";

const THROTTLE_SECS: i64 = 24 * 3600;

/// 编译期编入的本地真值（与 `commands/defaults.rs::CLIENT_TYPES_BUNDLED` 同源同文件，各自 include_str!，
/// 编译期同值无重复维护负担）。`validate_structure` 据此判定远端是否同构。
const BUNDLED: &str = include_str!("../../../../defaults/client-types.json");

/// bundled 解析缓存：首次访问解析一次，后续直接索引（参考 defaults_sync.rs `BUNDLED_VALUE` 模式）。
static BUNDLED_VALUE: OnceLock<serde_json::Value> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTypesSyncResult {
    pub updated: bool,
    pub last_updated: i64,
    /// "jsdelivr" | "raw" | "local" — 写盘来源；"local" = 全失败 / 校验失败不写
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 启动 hook 路径：检测到用户手工修改 app data 后跳过同步时为 true。
    /// 手动按钮路径（`sync_client_types_json` command）恒为 false（强制覆盖语义）。
    #[serde(default)]
    pub user_modified: bool,
}

#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn sync_client_types_json() -> ClientTypesSyncResult {
    tracing::info!("client-types.json sync started");
    let fetched = match fetch_client_types_json().await {
        Ok((body, source)) => (body, source),
        Err(e) => {
            tracing::warn!(error = %e, "client-types sync: fetch failed, keep local");
            let local_ts = read_local_last_updated().unwrap_or(0);
            return ClientTypesSyncResult {
                updated: false,
                last_updated: local_ts,
                source: "local".into(),
                error: Some(e),
                user_modified: false,
            };
        }
    };

    let (body, source) = fetched;
    let remote_ts = match parse_last_updated(&body) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "client-types sync: parse last_updated failed");
            let local_ts = read_local_last_updated().unwrap_or(0);
            return ClientTypesSyncResult {
                updated: false,
                last_updated: local_ts,
                source: "local".into(),
                error: Some(format!("parse last_updated: {e}")),
                user_modified: false,
            };
        }
    };

    if let Err(e) = validate_structure(&body) {
        tracing::warn!(error = %e, "client-types sync: structure validation failed, keep local");
        let local_ts = read_local_last_updated().unwrap_or(0);
        return ClientTypesSyncResult {
            updated: false,
            last_updated: local_ts,
            source: "local".into(),
            error: Some(format!("validate_structure: {e}")),
            user_modified: false,
        };
    }

    let local_ts = read_local_last_updated().unwrap_or(0);
    if remote_ts > local_ts {
        match write_app_data(&body) {
            Ok(()) => {
                let _ = write_last_sync_ts(now_secs());
                if let Err(e) = write_hash_snapshot(&body) {
                    tracing::warn!(error = %e, "client-types sync: write hash snapshot failed");
                }
                tracing::info!(remote_ts, local_ts, source = %source, "client-types.json updated from remote");
                ClientTypesSyncResult {
                    updated: true,
                    last_updated: remote_ts,
                    source,
                    error: None,
                    user_modified: false,
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "client-types sync: write app data failed");
                ClientTypesSyncResult {
                    updated: false,
                    last_updated: local_ts,
                    source: "local".into(),
                    error: Some(format!("write app data: {e}")),
                    user_modified: false,
                }
            }
        }
    } else {
        let _ = write_last_sync_ts(now_secs());
        tracing::debug!(remote_ts, local_ts, "client-types.json not newer, skip");
        ClientTypesSyncResult {
            updated: false,
            last_updated: local_ts,
            source,
            error: None,
            user_modified: false,
        }
    }
}

/// 启动 hook：用户定制保护 + 24h 节流。
/// 节流判定 = 读 `~/.aidog/client-types.json.last_sync`。
/// 全失败静默（warn log），绝不阻塞启动或破坏现有功能。
pub async fn maybe_sync_on_startup() {
    if is_user_modified() {
        tracing::info!("client-types sync skipped: user modified client-types.json (manual button re-enables)");
        return;
    }
    if !should_sync_due() {
        tracing::debug!("client-types sync throttled (within 24h), skip");
        return;
    }
    let _ = sync_client_types_json().await;
}

/// 节流判定：返回现在距上次同步 > 24h（或从未同步）。
fn should_sync_due() -> bool {
    let last = match read_last_sync_ts() {
        Ok(t) => t,
        Err(_) => return true,
    };
    if last <= 0 {
        return true;
    }
    now_secs() - last >= THROTTLE_SECS
}

async fn fetch_client_types_json() -> Result<(String, String), String> {
    // 同 defaults_sync：无 DB 依赖，裸 reqwest::Client，无代理需求。timeout 短（30s），
    // 失败回退 bundled（reader 端处理）。
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("build http client: {e}"))?;

    for (source, url) in [
        ("jsdelivr", CLIENT_TYPES_JSON_PRIMARY_URL),
        ("raw", CLIENT_TYPES_JSON_FALLBACK_URL),
    ] {
        match fetch_one(&client, url).await {
            Ok(body) => {
                tracing::info!(source, bytes = body.len(), "client-types.json fetched");
                return Ok((body, source.into()));
            }
            Err(e) => tracing::warn!(source, error = %e, "client-types.json fetch failed, trying next"),
        }
    }
    Err("client-types.json: all sources failed (jsDelivr + raw)".into())
}

async fn fetch_one(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client.get(url).send().await.map_err(|e| format!("fetch: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }
    resp.text().await.map_err(|e| format!("read body: {e}"))
}

/// 解析 client-types.json top-level `last_updated`（Unix 秒）。
fn parse_last_updated(body: &str) -> Result<i64, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| format!("json: {e}"))?;
    v.get("last_updated")
        .and_then(|t| t.as_i64())
        .ok_or_else(|| "missing/invalid last_updated".into())
}

/// bundled JSON 解析缓存（OnceLock，参考 defaults_sync.rs `BUNDLED_VALUE` 模式）。
fn bundled_value() -> &'static serde_json::Value {
    BUNDLED_VALUE.get_or_init(|| {
        serde_json::from_str(BUNDLED).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "client-types.json bundled parse failed (should never happen)");
            serde_json::Value::Object(serde_json::Map::new())
        })
    })
}

/// schema gate：写盘前对远端 body 做校验。
///
/// 校验项（任一失败 → `Err`）：
/// - body 可解析为 object，顶层含 `client_types` 数组
/// - 远端 value 集合 ⊇ 本地 bundled（可增不可减，前向兼容；新 client_type 不丢）
/// - 远端每个 entry 含 `value`(string) / `group`(string) / `name`(object) 关键字段
///
/// 不校验 desc / locale 完整性（值细节），仅存在性 + 粗类型，平衡安全与前向兼容。
pub(crate) fn validate_structure(body: &str) -> Result<(), String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("json parse: {e}"))?;
    let top = v
        .as_object()
        .ok_or_else(|| "top level not object".to_string())?;
    let arr = top
        .get("client_types")
        .and_then(|c| c.as_array())
        .ok_or_else(|| "missing/invalid client_types".to_string())?;

    let bundled = bundled_value();
    let bundled_arr = bundled
        .get("client_types")
        .and_then(|c| c.as_array())
        .ok_or_else(|| "bundled client_types missing (binary corrupt?)".to_string())?;

    // 每个 entry 字段存在性 + 粗类型（先于 value 集合检查：缺 value 字段应报 per-entry 错
    // 而非 `missing client_type`，便于测试与诊断）
    for (i, entry) in arr.iter().enumerate() {
        let obj = entry
            .as_object()
            .ok_or_else(|| format!("client_types[{i}]: entry not object"))?;
        if !obj.get("value").map(|v| v.is_string()).unwrap_or(false) {
            return Err(format!("client_types[{i}]: missing/invalid value"));
        }
        if !obj.get("group").map(|v| v.is_string()).unwrap_or(false) {
            return Err(format!("client_types[{i}]: missing/invalid group"));
        }
        if !obj.get("name").map(|v| v.is_object()).unwrap_or(false) {
            return Err(format!("client_types[{i}]: missing/invalid name"));
        }
    }

    // 收集 bundled value 集合，远端必须 ⊇ 本地（per-entry 字段齐后，再判定 value 集合完整）
    let bundled_values: Vec<&str> = bundled_arr
        .iter()
        .filter_map(|e| e.get("value").and_then(|v| v.as_str()))
        .collect();
    let remote_values: std::collections::HashSet<&str> = arr
        .iter()
        .filter_map(|e| e.get("value").and_then(|v| v.as_str()))
        .collect();
    for v in bundled_values {
        if !remote_values.contains(v) {
            return Err(format!("missing client_type: {v}"));
        }
    }

    Ok(())
}

fn app_data_path() -> Result<std::path::PathBuf, String> {
    Ok(aidog_data_dir()?.join("client-types.json"))
}

fn last_sync_path() -> Result<std::path::PathBuf, String> {
    Ok(aidog_data_dir()?.join("client-types.json.last_sync"))
}

fn hash_path() -> Result<std::path::PathBuf, String> {
    Ok(aidog_data_dir()?.join("client-types.json.hash"))
}

fn read_local_last_updated() -> Result<i64, String> {
    let p = app_data_path()?;
    if !p.exists() {
        return Err("no local client-types.json".into());
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

/// 成功同步后写 sha256 快照（hex of body），作为后续 user_modified 检测基线。
fn write_hash_snapshot(body: &str) -> Result<(), String> {
    let p = hash_path()?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let hash = sha256_hex(body.as_bytes());
    std::fs::write(&p, hash).map_err(|e| format!("write hash: {e}"))
}

/// user_modified 检测：当前 app data sha256 ≠ `.hash` 快照内容 → true。
/// 无 `.hash` 文件（首次 / 旧版升级）→ false 不阻塞（随后正常同步建立基线）。
/// 无 app data / 读失败 → false（交由 fetch 流程处理）。
fn is_user_modified() -> bool {
    let app = match app_data_path() {
        Ok(p) => p,
        Err(_) => return false,
    };
    if !app.exists() {
        return false;
    }
    let hash_path = match hash_path() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let stored = match std::fs::read_to_string(&hash_path) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return false,
    };
    let bytes = match std::fs::read(&app) {
        Ok(b) => b,
        Err(_) => return false,
    };
    sha256_hex(&bytes) != stored
}

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

/// sha256 hex 单点 helper（复用 `import_export::container::sha256_hex`，避免重写）。
fn sha256_hex(bytes: &[u8]) -> String {
    crate::gateway::import_export::container::sha256_hex(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_last_updated_ok() {
        let body = r#"{"last_updated":1783347706,"client_types":[]}"#;
        assert_eq!(parse_last_updated(body).unwrap(), 1783347706);
    }

    #[test]
    fn parse_last_updated_missing_fails() {
        let body = r#"{"client_types":[]}"#;
        assert!(parse_last_updated(body).is_err());
    }

    #[test]
    fn parse_last_updated_bad_json_fails() {
        assert!(parse_last_updated("not json").is_err());
    }

    #[test]
    fn should_sync_due_when_no_file() {
        assert_eq!(should_sync_due_internal(None), true);
        assert_eq!(should_sync_due_internal(Some(0)), true);
        assert_eq!(should_sync_due_internal(Some(now_secs())), false);
        assert_eq!(should_sync_due_internal(Some(now_secs() - THROTTLE_SECS - 1)), true);
    }

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

    // ===== validate_structure 单测 =====

    /// 合法 body（bundled 本身满足）→ Ok。
    #[test]
    fn validate_structure_ok() {
        assert!(validate_structure(BUNDLED).is_ok());
    }

    /// 顶层缺 client_types → Err。
    #[test]
    fn validate_structure_missing_client_types_fails() {
        let body = r#"{"last_updated":1}"#;
        let err = validate_structure(body).unwrap_err();
        assert!(err.contains("client_types"), "unexpected err: {err}");
    }

    /// 远端少一个本地 client_type → Err 含 "missing client_type"。
    #[test]
    fn validate_structure_missing_one_client_type_fails() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        let arr = v.get_mut("client_types").unwrap().as_array_mut().unwrap();
        arr.remove(0);
        let body = serde_json::to_string(&v).unwrap();
        let err = validate_structure(&body).unwrap_err();
        assert!(err.contains("missing client_type"), "unexpected err: {err}");
    }

    /// 远端多一个新 client_type（字段齐）→ Ok（前向兼容）。
    #[test]
    fn validate_structure_extra_remote_client_type_ok() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        let arr = v.get_mut("client_types").unwrap().as_array_mut().unwrap();
        arr.push(serde_json::json!({
            "value": "brand_new_xyz",
            "group": "NewGroup",
            "name": { "en-US": "New Client" }
        }));
        let body = serde_json::to_string(&v).unwrap();
        assert!(validate_structure(&body).is_ok());
    }

    /// 某 entry 缺 value → Err。
    /// 注：移除 entry[0] ("default") 的 value 会先触发「bundled ⊇ remote」检查报
    /// `missing client_type: default`（因 value 缺致 filter_map 跳过该 entry），
    /// 故此处选 entry[1] 测 per-entry 字段检查。
    #[test]
    fn validate_structure_missing_value_fails() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        v["client_types"][1]
            .as_object_mut()
            .unwrap()
            .remove("value");
        let body = serde_json::to_string(&v).unwrap();
        let err = validate_structure(&body).unwrap_err();
        assert!(err.contains("value"), "unexpected err: {err}");
    }

    /// 某 entry name 类型错位（给 array）→ Err。
    /// 注：选 entry[1] 避开 entry[0] 缺 name 时被 `bundled ⊇ remote` 误报（同 _missing_value_fails）。
    #[test]
    fn validate_structure_name_wrong_type_fails() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        v["client_types"][1]["name"] = serde_json::json!([]);
        let body = serde_json::to_string(&v).unwrap();
        let err = validate_structure(&body).unwrap_err();
        assert!(err.contains("name"), "unexpected err: {err}");
    }

    #[test]
    fn validate_structure_bad_json_fails() {
        assert!(validate_structure("not json").is_err());
    }

    #[test]
    fn validate_structure_top_not_object_fails() {
        let body = r#"[1,2,3]"#;
        let err = validate_structure(body).unwrap_err();
        assert!(err.contains("top level"), "unexpected err: {err}");
    }

    /// user_modified 检测逻辑（不依赖真实 fs）。
    #[test]
    fn user_modified_detection_logic() {
        let body = b"hello world";
        let hash = sha256_hex(body);

        assert_eq!(is_user_modified_internal(body, Some(&hash)), false);
        assert_eq!(is_user_modified_internal(body, Some("deadbeef")), true);
        assert_eq!(is_user_modified_internal(body, None), false);
        let body2 = b"goodbye world";
        assert_eq!(is_user_modified_internal(body2, Some(&hash)), true);
    }

    fn is_user_modified_internal(app_bytes: &[u8], stored: Option<&str>) -> bool {
        let stored = match stored {
            Some(s) => s,
            None => return false,
        };
        sha256_hex(app_bytes) != stored
    }

    #[test]
    fn sha256_hex_matches_container() {
        let bytes = b"test payload";
        assert_eq!(sha256_hex(bytes), crate::gateway::import_export::container::sha256_hex(bytes));
    }
}
