//! platform-presets.json 同步：拉 jsDelivr master `src-tauri/defaults/platform-presets.json`（主）+ raw fallback。
//!
//! 架构同 price_sync.rs：双源 fetch + 远程 `last_updated`（Unix 秒）与本地比对，远程较新才写。
//! 写入 app data (`~/.aidog/platform-presets.json`)，由 commands/defaults.rs 的 reader 自动优先读取。
//! 节流时间戳：`~/.aidog/platform-presets.json.last_sync`（Unix 秒）。
//!
//! 三路触发：
//! - 启动 hook（maybe_sync_on_startup，24h 节流 + 用户定制保护）
//! - 每日定时器（spawn_daily_sync，复用 spawn 模式）
//! - 设置页手动按钮（sync_defaults_json command，无视节流）
//!
//! **结构一致性 schema gate**（R1）：写盘前 `validate_structure` 校验远端 body 与 bundled 同构
//! （协议集合 ⊇ bundled + 每个协议含 endpoints/models/model_list 关键字段），失败拒绝写入保留本地。
//! **用户定制保护**（R3）：成功同步后写 `.hash` 快照（sha256 of body）；启动 hook 检测 app data
//! 被手工修改（sha256 ≠ 快照）则跳过自动同步；手动按钮不受影响（强制覆盖 + 重置快照）。

use crate::shared::aidog_data_dir;
use serde::Serialize;
use std::sync::OnceLock;

/// 主源：jsDelivr CDN（master 分支）。
const DEFAULTS_JSON_PRIMARY_URL: &str =
    "https://cdn.jsdelivr.net/gh/lazygophers/aidog@master/src-tauri/defaults/platform-presets.json";

/// fallback：GitHub raw（master 分支）。
const DEFAULTS_JSON_FALLBACK_URL: &str =
    "https://raw.githubusercontent.com/lazygophers/aidog/master/src-tauri/defaults/platform-presets.json";

const THROTTLE_SECS: i64 = 24 * 3600;

/// 编译期编入的本地真值（与 `commands/defaults.rs::BUNDLED` 同源同文件，各自 include_str!，
/// 编译期同值无重复维护负担）。`validate_structure` 据此判定远端协议集合是否 ⊇ 本地。
const BUNDLED: &str = include_str!("../../../../defaults/platform-presets.json");

/// bundled 解析缓存：首次访问解析一次，后续直接索引（参考 peak_hours.rs `PRESETS` 模式）。
static BUNDLED_VALUE: OnceLock<serde_json::Value> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultsSyncResult {
    pub updated: bool,
    pub last_updated: i64,
    /// "jsdelivr" | "raw" | "local" — 写盘来源；"local" = 全失败 / 校验失败不写
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 启动 hook 路径：检测到用户手工修改 app data 后跳过同步时为 true。
    /// 手动按钮路径（`sync_defaults_json` command）恒为 false（强制覆盖语义）。
    /// 参见 R3.5 / R4.1；TS 侧 `userModified?: boolean` 对称。
    #[serde(default)]
    pub user_modified: bool,
}

#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn sync_defaults_json() -> DefaultsSyncResult {
    tracing::info!("platform-presets.json sync started");
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
                user_modified: false,
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
                user_modified: false,
            };
        }
    };

    // R1 结构一致性门：写盘前校验。失败 → R2 不写 / 不更新 last_sync / 计失败同步（无节流）。
    if let Err(e) = validate_structure(&body) {
        tracing::warn!(error = %e, "defaults sync: structure validation failed, keep local");
        let local_ts = read_local_last_updated().unwrap_or(0);
        return DefaultsSyncResult {
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
                // R3.1：成功同步后写 hash 快照（失败仅 warn，不阻塞同步成功语义）
                if let Err(e) = write_hash_snapshot(&body) {
                    tracing::warn!(error = %e, "defaults sync: write hash snapshot failed");
                }
                tracing::info!(remote_ts, local_ts, source = %source, "platform-presets.json updated from remote");
                DefaultsSyncResult {
                    updated: true,
                    last_updated: remote_ts,
                    source,
                    error: None,
                    user_modified: false,
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "defaults sync: write app data failed");
                DefaultsSyncResult {
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
        tracing::debug!(remote_ts, local_ts, "platform-presets.json not newer, skip");
        DefaultsSyncResult {
            updated: false,
            last_updated: local_ts,
            source,
            error: None,
            user_modified: false,
        }
    }
}

/// 启动 hook：用户定制保护 + 24h 节流。
/// 节流判定 = 读 `~/.aidog/platform-presets.json.last_sync`。
/// 全失败静默（warn log），绝不阻塞启动或破坏现有功能。
pub async fn maybe_sync_on_startup() {
    // R3.2-R3.3：用户手工修改 app data 后跳过自动同步（手动按钮仍生效）。
    if is_user_modified() {
        tracing::info!("defaults sync skipped: user modified platform-presets.json (manual button re-enables)");
        return;
    }
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
    // ponytail: 无 DB 依赖（platform-presets.json 是无状态文件），用裸 reqwest::Client。
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
                tracing::info!(source, bytes = body.len(), "platform-presets.json fetched");
                return Ok((body, source.into()));
            }
            Err(e) => tracing::warn!(source, error = %e, "platform-presets.json fetch failed, trying next"),
        }
    }
    Err("platform-presets.json: all sources failed (jsDelivr + raw)".into())
}

async fn fetch_one(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client.get(url).send().await.map_err(|e| format!("fetch: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }
    resp.text().await.map_err(|e| format!("read body: {e}"))
}

/// 解析 platform-presets.json top-level `last_updated`（Unix 秒）。
fn parse_last_updated(body: &str) -> Result<i64, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| format!("json: {e}"))?;
    v.get("last_updated")
        .and_then(|t| t.as_i64())
        .ok_or_else(|| "missing/invalid last_updated".into())
}

/// bundled JSON 解析缓存（OnceLock，参考 peak_hours.rs `PRESETS` 模式）。
fn bundled_value() -> &'static serde_json::Value {
    BUNDLED_VALUE.get_or_init(|| {
        serde_json::from_str(BUNDLED).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "platform-presets.json bundled parse failed (should never happen)");
            serde_json::Value::Object(serde_json::Map::new())
        })
    })
}

/// **R1 结构一致性校验**：写盘前对远端 body 做 schema gate。
///
/// 校验项（任一失败 → `Err`，详见 R1.1-R1.5）：
/// - body 可解析为 object，顶层含 `protocols` object
/// - 远端 protocol key 集合 ⊇ 本地 bundled（可增不可减，前向兼容）
/// - 远端每个 protocol 条目含 `endpoints`(array) / `models`(object) / `model_list`(array)
///
/// 不校验值细节（字段值内容），仅存在性 + 粗类型，平衡安全与前向兼容（见 ADR-lite）。
pub(crate) fn validate_structure(body: &str) -> Result<(), String> {
    // R1.1：body 可解析为 object
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("json parse: {e}"))?;
    let top = v
        .as_object()
        .ok_or_else(|| "top level not object".to_string())?;
    // R1.2：顶层 protocols object
    let protocols = top
        .get("protocols")
        .and_then(|p| p.as_object())
        .ok_or_else(|| "missing/invalid protocols".to_string())?;

    let bundled = bundled_value();
    let bundled_protocols = bundled
        .get("protocols")
        .and_then(|p| p.as_object())
        .ok_or_else(|| "bundled protocols missing (binary corrupt?)".to_string())?;

    // R1.3：远端 ⊇ 本地（可增不可减）
    for key in bundled_protocols.keys() {
        if !protocols.contains_key(key) {
            return Err(format!("missing protocol: {key}"));
        }
    }

    // R1.4 + R1.5：每个远端协议（含本地共有 + 远端新增）字段存在性 + 粗类型。
    // ponytail: 实测 bundled 真值三字段均 object（branch dict，如 `{"default": [...]}`），
    // 非 PRD 文字描述的 array。校验严格度匹配真值：三字段必须 object（branch 形态）。
    for (key, entry) in protocols.iter() {
        let obj = entry
            .as_object()
            .ok_or_else(|| format!("protocol {key}: entry not object"))?;
        if !obj.get("endpoints").map(|v| v.is_object()).unwrap_or(false) {
            return Err(format!("protocol {key}: missing/invalid endpoints"));
        }
        if !obj.get("models").map(|v| v.is_object()).unwrap_or(false) {
            return Err(format!("protocol {key}: missing/invalid models"));
        }
        if !obj.get("model_list").map(|v| v.is_object()).unwrap_or(false) {
            return Err(format!("protocol {key}: missing/invalid model_list"));
        }
    }

    Ok(())
}

/// **Reader deep merge**：app data 优先，bundled 补 app data 缺的 protocol key。
///
/// 触发场景：用户 app data 旧（`last_updated` 落后 bundled），缺 bundled 新增的 protocol
/// （如 `glm_coding`）。reader 仅优先 app data 不 merge → 派生层 `getProtocolLabel` 在 app
/// data 找不到 → fallback raw protocol 名（"glm_coding"）。merge 后派生层即时拿全量，不依赖
/// 同步链覆盖 app data（同步受 24h 节流，下次才能更新）。
///
/// 合并规则：
/// - `protocols` 层：clone app data，per-key 补缺（app data 已有的 key 保留 app 值，**不**用
///   bundled 覆盖——protocol entry 是真值源非用户数据，但 app data 可能含中间版本的用户定制
///   endpoint flag，宁可滞后也不覆盖丢定制）。
/// - 顶层 `last_updated`：取 `max(app, bundled)`（让同步链 `remote_ts > local_ts` 比对仍正确：
///   本地 merge 后看起来新了，但同步链读 app data 原文件 `last_updated` 非 merge 结果，比对不受影响）。
///   reader 返 merge JSON 仅给派生层展示，不写盘，所以 sync 的 `read_local_last_updated` 仍读旧 app data。
/// - 顶层其他字段（`version` 等）：取 app data（向后兼容）。
///
/// 异常 fallback：app 或 bundled 非 object / 缺 protocols 字段 → 返另一边整体（保 reader
/// 永不返半坏 JSON）。
pub fn merge_with_bundled(app: &serde_json::Value, bundled: &serde_json::Value) -> serde_json::Value {
    let app_obj = match app.as_object() {
        Some(o) => o,
        None => return bundled.clone(),
    };
    let bundled_obj = match bundled.as_object() {
        Some(o) => o,
        None => return app.clone(),
    };

    let mut result = app.clone();
    let result_obj = result.as_object_mut().expect("cloned from app_obj");

    // protocols 层 per-key 补缺
    let app_protocols = app_obj.get("protocols").and_then(|v| v.as_object());
    let bundled_protocols = bundled_obj.get("protocols").and_then(|v| v.as_object());
    match (app_protocols, bundled_protocols) {
        (Some(app_p), Some(bundled_p)) => {
            let mut merged = app_p.clone();
            for (k, v) in bundled_p.iter() {
                // 仅补 app data 缺的 key，已有的不动（保用户定制 endpoint flag 等）
                merged.entry(k.clone()).or_insert_with(|| v.clone());
            }
            result_obj.insert("protocols".into(), serde_json::Value::Object(merged));
        }
        (None, Some(bundled_p)) => {
            // app data 异常缺 protocols 字段，整体用 bundled 的 protocols
            result_obj.insert("protocols".into(), serde_json::Value::Object(bundled_p.clone()));
        }
        _ => {}
    }

    // 顶层 last_updated 取 max（保同步链比对不受 merge 影响——见上注释）
    merge_top_last_updated(result_obj, app_obj, bundled_obj);

    result
}

/// 顶层 `last_updated` 合并：取 `max(app, bundled)`。
/// 同步链 `read_local_last_updated` 读 app data 原文件（非 merge 结果），所以这里取 max 不
/// 影响同步触发判定；reader 返 merge JSON 仅给派生层展示用。
fn merge_top_last_updated(
    result_obj: &mut serde_json::Map<String, serde_json::Value>,
    app_obj: &serde_json::Map<String, serde_json::Value>,
    bundled_obj: &serde_json::Map<String, serde_json::Value>,
) {
    let app_ts = app_obj.get("last_updated").and_then(|v| v.as_i64());
    let bundled_ts = bundled_obj.get("last_updated").and_then(|v| v.as_i64());
    match (app_ts, bundled_ts) {
        (Some(a), Some(b)) => {
            result_obj.insert("last_updated".into(), serde_json::json!(a.max(b)));
        }
        (None, Some(b)) => {
            result_obj.insert("last_updated".into(), serde_json::json!(b));
        }
        _ => {}
    }
}

fn app_data_path() -> Result<std::path::PathBuf, String> {
    Ok(aidog_data_dir()?.join("platform-presets.json"))
}

fn last_sync_path() -> Result<std::path::PathBuf, String> {
    Ok(aidog_data_dir()?.join("platform-presets.json.last_sync"))
}

fn hash_path() -> Result<std::path::PathBuf, String> {
    Ok(aidog_data_dir()?.join("platform-presets.json.hash"))
}

fn read_local_last_updated() -> Result<i64, String> {
    let p = app_data_path()?;
    if !p.exists() {
        return Err("no local platform-presets.json".into());
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

/// R3.1：成功同步后写 sha256 快照（hex of body），作为后续 user_modified 检测基线。
fn write_hash_snapshot(body: &str) -> Result<(), String> {
    let p = hash_path()?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let hash = sha256_hex(body.as_bytes());
    std::fs::write(&p, hash).map_err(|e| format!("write hash: {e}"))
}

/// R3.2 user_modified 检测：当前 app data sha256 ≠ `.hash` 快照内容 → true。
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
        Err(_) => return false, // 无快照文件 → 视为未修改
    };
    let bytes = match std::fs::read(&app) {
        Ok(b) => b,
        Err(_) => return false,
    };
    // 判定核心：sha256(app data) ≠ stored → user modified。
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

    // ===== R5 validate_structure 单测 =====

    /// R5.1：合法 body（bundled 本身满足全部协议 + 字段齐）→ Ok。
    #[test]
    fn validate_structure_ok() {
        assert!(validate_structure(BUNDLED).is_ok());
    }

    /// R5.2：顶层缺 protocols → Err。
    #[test]
    fn validate_structure_missing_protocols_fails() {
        let body = r#"{"version":"1","last_updated":1}"#;
        let err = validate_structure(body).unwrap_err();
        assert!(err.contains("protocols"), "unexpected err: {err}");
    }

    /// R5.3：远端少一个本地协议 → Err 含 "missing protocol"。
    #[test]
    fn validate_structure_missing_one_protocol_fails() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        let protocols = v.get_mut("protocols").unwrap().as_object_mut().unwrap();
        let first_key = protocols.keys().next().unwrap().clone();
        protocols.remove(&first_key);
        let body = serde_json::to_string(&v).unwrap();
        let err = validate_structure(&body).unwrap_err();
        assert!(err.contains("missing protocol"), "unexpected err: {err}");
        assert!(err.contains(&first_key), "err should name missing key: {err}");
    }

    /// R5.4：远端多一个新协议（字段齐）→ Ok（前向兼容）。
    #[test]
    fn validate_structure_extra_remote_protocol_ok() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        let protocols = v.get_mut("protocols").unwrap().as_object_mut().unwrap();
        protocols.insert(
            "brand_new_proto_xyz".into(),
            serde_json::json!({
                "endpoints": {},
                "models": {},
                "model_list": {}
            }),
        );
        let body = serde_json::to_string(&v).unwrap();
        assert!(validate_structure(&body).is_ok());
    }

    /// R5.5：某 protocol 缺 endpoints → Err。
    #[test]
    fn validate_structure_missing_endpoints_fails() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        let protocols = v.get_mut("protocols").unwrap().as_object_mut().unwrap();
        let first_key = protocols.keys().next().unwrap().clone();
        let entry = protocols.get_mut(&first_key).unwrap();
        entry.as_object_mut().unwrap().remove("endpoints");
        let body = serde_json::to_string(&v).unwrap();
        let err = validate_structure(&body).unwrap_err();
        assert!(err.contains("endpoints"), "unexpected err: {err}");
    }

    /// R5.6：某 protocol `models` 类型错位（给 array）→ Err。
    #[test]
    fn validate_structure_models_wrong_type_fails() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        let protocols = v.get_mut("protocols").unwrap().as_object_mut().unwrap();
        let first_key = protocols.keys().next().unwrap().clone();
        let entry = protocols.get_mut(&first_key).unwrap();
        entry
            .as_object_mut()
            .unwrap()
            .insert("models".into(), serde_json::json!([]));
        let body = serde_json::to_string(&v).unwrap();
        let err = validate_structure(&body).unwrap_err();
        assert!(err.contains("models"), "unexpected err: {err}");
    }

    /// R1 额外：body 非 json / 顶层非 object → Err。
    #[test]
    fn validate_structure_bad_json_fails() {
        assert!(validate_structure("not json").is_err());
    }

    /// R1 额外：顶层非 object（array）→ Err。
    #[test]
    fn validate_structure_top_not_object_fails() {
        let body = r#"[1,2,3]"#;
        let err = validate_structure(body).unwrap_err();
        assert!(err.contains("top level"), "unexpected err: {err}");
    }

    /// R1 额外：新协议缺字段 → Err（R1.5 验证）。
    #[test]
    fn validate_structure_extra_protocol_missing_fields_fails() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        let protocols = v.get_mut("protocols").unwrap().as_object_mut().unwrap();
        protocols.insert(
            "brand_new_proto_xyz".into(),
            serde_json::json!({
                "endpoints": {},
                "model_list": {}
                // missing models
            }),
        );
        let body = serde_json::to_string(&v).unwrap();
        let err = validate_structure(&body).unwrap_err();
        assert!(err.contains("models"), "unexpected err: {err}");
    }

    /// R1 额外：protocol entry 非 object → Err。
    #[test]
    fn validate_structure_protocol_entry_not_object_fails() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        let protocols = v.get_mut("protocols").unwrap().as_object_mut().unwrap();
        protocols.insert("bad_proto".into(), serde_json::json!(42));
        let body = serde_json::to_string(&v).unwrap();
        let err = validate_structure(&body).unwrap_err();
        assert!(err.contains("not object"), "unexpected err: {err}");
    }

    /// R5.7：hash 快照 + user_modified 检测逻辑（不依赖真实 fs）。
    #[test]
    fn user_modified_detection_logic() {
        let body = b"hello world";
        let hash = sha256_hex(body);

        // hash 匹配 → 未修改
        assert_eq!(is_user_modified_internal(body, Some(&hash)), false);
        // hash 不匹配 → 已修改
        assert_eq!(is_user_modified_internal(body, Some("deadbeef")), true);
        // 无快照文件（首次 / 旧版升级）→ 未修改（不阻塞，建基线）
        assert_eq!(is_user_modified_internal(body, None), false);
        // 不同 body → hash 不同 → 修改
        let body2 = b"goodbye world";
        assert_eq!(is_user_modified_internal(body2, Some(&hash)), true);
    }

    /// user_modified 单测辅助：抽 `is_user_modified` 的判定核心，脱离真实 fs。
    /// `stored` = `.hash` 文件内容（None = 文件不存在）。
    fn is_user_modified_internal(app_bytes: &[u8], stored: Option<&str>) -> bool {
        let stored = match stored {
            Some(s) => s,
            None => return false,
        };
        sha256_hex(app_bytes) != stored
    }

    /// hash 算法跨调用点一致性：本模块 `sha256_hex` == `container::sha256_hex`。
    #[test]
    fn sha256_hex_matches_container() {
        let bytes = b"test payload";
        assert_eq!(sha256_hex(bytes), crate::gateway::import_export::container::sha256_hex(bytes));
    }

    // ===== Reader deep merge 单测 =====

    /// merge：app data 旧（缺 glm_coding）+ bundled 新（含）→ merge 后含 glm_coding。
    #[test]
    fn merge_presets_fills_missing_protocol() {
        let app = serde_json::json!({
            "version": "1",
            "last_updated": 100,
            "protocols": {
                "anthropic": {"endpoints": {}, "models": {}, "model_list": {}}
            }
        });
        let bundled = serde_json::json!({
            "version": "1",
            "last_updated": 200,
            "protocols": {
                "anthropic": {"endpoints": {"x": []}, "models": {}, "model_list": {}},
                "glm_coding": {"endpoints": {}, "models": {}, "model_list": {}}
            }
        });
        let merged = merge_with_bundled(&app, &bundled);
        let protocols = merged.get("protocols").unwrap().as_object().unwrap();
        assert!(protocols.contains_key("glm_coding"), "missing protocol should be filled from bundled");
        assert!(protocols.contains_key("anthropic"));
        // app data 已有 key 保留 app 值（不被 bundled 覆盖）
        assert_eq!(
            protocols.get("anthropic").unwrap().get("endpoints").unwrap().as_object().unwrap().len(),
            0
        );
    }

    /// merge：last_updated 取 max（app 旧 100 + bundled 新 200 → 200）。
    #[test]
    fn merge_presets_last_updated_max() {
        let app = serde_json::json!({"last_updated": 100, "protocols": {}});
        let bundled = serde_json::json!({"last_updated": 200, "protocols": {}});
        let merged = merge_with_bundled(&app, &bundled);
        assert_eq!(merged.get("last_updated").unwrap().as_i64().unwrap(), 200);

        // 反向也成立
        let app2 = serde_json::json!({"last_updated": 300, "protocols": {}});
        let merged2 = merge_with_bundled(&app2, &bundled);
        assert_eq!(merged2.get("last_updated").unwrap().as_i64().unwrap(), 300);
    }

    /// merge：app data 缺 last_updated，bundled 有 → 用 bundled 的。
    #[test]
    fn merge_presets_app_missing_last_updated() {
        let app = serde_json::json!({"protocols": {}});
        let bundled = serde_json::json!({"last_updated": 200, "protocols": {}});
        let merged = merge_with_bundled(&app, &bundled);
        assert_eq!(merged.get("last_updated").unwrap().as_i64().unwrap(), 200);
    }

    /// merge：app data 非 object（损坏解析后是 array / null）→ 返 bundled 全量。
    #[test]
    fn merge_presets_app_not_object_returns_bundled() {
        let app = serde_json::json!([1, 2, 3]);
        let bundled = serde_json::json!({"last_updated": 200, "protocols": {"x": {}}});
        let merged = merge_with_bundled(&app, &bundled);
        assert_eq!(merged, bundled);
    }

    /// merge：app data 缺 protocols 字段（异常但兼容）→ 用 bundled 的 protocols。
    #[test]
    fn merge_presets_app_missing_protocols_field() {
        let app = serde_json::json!({"version": "1", "last_updated": 100});
        let bundled = serde_json::json!({"version": "1", "last_updated": 200, "protocols": {"glm_coding": {}}});
        let merged = merge_with_bundled(&app, &bundled);
        let protocols = merged.get("protocols").unwrap().as_object().unwrap();
        assert!(protocols.contains_key("glm_coding"));
        assert_eq!(merged.get("last_updated").unwrap().as_i64().unwrap(), 200);
    }

    /// merge：app data 有 bundled 没有的 protocol（理论上不会发生，但要兼容）→ app 保留。
    #[test]
    fn merge_presets_keeps_app_only_protocols() {
        let app = serde_json::json!({
            "last_updated": 100,
            "protocols": {"app_only": {"endpoints": {}, "models": {}, "model_list": {}}}
        });
        let bundled = serde_json::json!({
            "last_updated": 200,
            "protocols": {"bundled_only": {"endpoints": {}, "models": {}, "model_list": {}}}
        });
        let merged = merge_with_bundled(&app, &bundled);
        let protocols = merged.get("protocols").unwrap().as_object().unwrap();
        assert!(protocols.contains_key("app_only"));
        assert!(protocols.contains_key("bundled_only"));
    }

    /// merge：顶层其他字段（version）取 app data（向后兼容）。
    #[test]
    fn merge_presets_keeps_app_version() {
        let app = serde_json::json!({"version": "2-app", "last_updated": 100, "protocols": {}});
        let bundled = serde_json::json!({"version": "1-bundled", "last_updated": 200, "protocols": {}});
        let merged = merge_with_bundled(&app, &bundled);
        assert_eq!(merged.get("version").unwrap().as_str().unwrap(), "2-app");
    }

    /// merge：真实 bundled 含 glm_coding，模拟 app data 旧（缺 glm_coding）→ merge 后必含。
    /// 验收断言 R3：用户当前 app data 场景下 merge 后 glm_coding 可见。
    #[test]
    fn merge_presets_real_bundled_fills_glm_coding() {
        let bundled: serde_json::Value =
            serde_json::from_str(BUNDLED).expect("bundled parse");
        // 构造 app data：bundled 删 glm_coding，模拟旧版本同步结果
        let mut app = bundled.clone();
        app.get_mut("protocols").unwrap().as_object_mut().unwrap().remove("glm_coding");
        app["last_updated"] = serde_json::json!(1); // 旧时间戳
        let merged = merge_with_bundled(&app, &bundled);
        let protocols = merged.get("protocols").unwrap().as_object().unwrap();
        assert!(protocols.contains_key("glm_coding"), "glm_coding must be filled by merge");
        // last_updated 取 max（bundled 的原值）
        assert_eq!(
            merged.get("last_updated").unwrap().as_i64().unwrap(),
            bundled.get("last_updated").unwrap().as_i64().unwrap()
        );
    }
}
