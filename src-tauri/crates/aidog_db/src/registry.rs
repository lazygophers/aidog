//! registry 真值源（`src-tauri/defaults/registry/`）的只读合并视图。
//!
//! 真值源人工维护：`index.json` + `platforms/<code>/platform.json` + `platforms/<code>/models/<model>.json`。
//! 全部文件由 `build.rs` 在编译期枚举并 `include_str!`（新增文件自动纳入，无需改 Rust）。
//! 模型文件名只求稳定唯一，真值是文件内的 `model_id`；同目录大小写撞名的后一个带 `~N` 后缀
//! （macOS 文件系统大小写不敏感，如 `MiniMax-M2` / `minimax-m2`）。
//!
//! 本模块提供一个合并视图 [`presets`] / [`presets_json`]，等价于旧 `platform-presets.json`
//! （`{last_updated, protocols}`），供既有消费方零改动接入。
//!
//! 模型条目**不再有合并视图**：旧 `models.json` 单模型形状（跨平台条目按 `model_id` 归并回
//! `pricing` 映射）随票 T4 把计费切到 `model_entry` 表一并废弃，本模块只出 [`bundled_model_files`]
//! 原始文本，归并逻辑不复存在（同一模型在不同平台是不同的行，不需要合并）。

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, RwLock};

include!(concat!(env!("OUT_DIR"), "/registry_includes.rs"));

static PRESETS: OnceLock<Arc<Value>> = OnceLock::new();
static PRESETS_JSON: OnceLock<String> = OnceLock::new();

fn parse(what: &str, json: &str) -> Map<String, Value> {
    match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => panic!("registry {what}: {e}"),
    }
}

/// 编译期内置那份合并文档的共享句柄（[`effective_presets`] 未命中缓存时返回它）。
pub fn presets_arc() -> &'static Arc<Value> {
    PRESETS.get_or_init(|| {
        let index = parse("index.json", INDEX_JSON);
        Arc::new(presets_doc(
            PLATFORM_FILES.iter().map(|(code, json)| (*code, *json)),
            index["last_updated"].clone(),
        ))
    })
}

/// 合并 65 份 `platform.json` → 旧 presets 文档。首次访问解析一次，后续共享同一静态实例。
/// **这是编译期内置那份**：需要「DB 同步值优先」的读取方一律走 [`effective_presets`]。
pub fn presets() -> &'static Value {
    presets_arc().as_ref()
}

/// DB 同步后的 preset 合并视图缓存：文档 + 其序列化文本。
/// 一处构建、多处共享——前端 `get_defaults_json`、logo 懒加载、路由热路径的
/// `peak` / `models.peak` 都读它，避免各自重建整篇文档。
struct PresetCache {
    doc: Arc<Value>,
    json: Arc<String>,
}

static PRESET_CACHE: RwLock<Option<PresetCache>> = RwLock::new(None);

/// bundled + DB 行合并：同 `code` 以 DB 行为准，bundled 里 DB 缺的补齐（票 13-C）。
/// DB 一份都没有时结果与 [`presets`] 逐字节相同（首次同步失败不该让协议下拉少几项）。
///
/// `db_last_updated_secs` = DB 行 `updated_at` 最大值（秒）；`None` 表示没有 DB 行，
/// 此时 `last_updated` 用 bundled `index.json` 里那个。
pub fn merge_presets_doc<'a>(
    db_rows: impl IntoIterator<Item = (&'a str, &'a str)>,
    db_last_updated_secs: Option<i64>,
) -> Value {
    let mut entries: BTreeMap<&str, &str> = PLATFORM_FILES.iter().map(|(c, j)| (*c, *j)).collect();
    for (code, json) in db_rows {
        entries.insert(code, json);
    }
    let index = parse("index.json", INDEX_JSON);
    let last_updated = match db_last_updated_secs {
        Some(secs) => Value::from(secs),
        None => index["last_updated"].clone(),
    };
    presets_doc(entries, last_updated)
}

/// 缓存命中即返 `(doc, json)`；未填充过返 None。
pub fn cached_presets() -> Option<(Arc<Value>, Arc<String>)> {
    let guard = PRESET_CACHE.read().ok()?;
    guard.as_ref().map(|c| (c.doc.clone(), c.json.clone()))
}

/// 填充缓存并返回共享句柄。
pub fn store_presets_cache(doc: Value) -> (Arc<Value>, Arc<String>) {
    let json = Arc::new(doc.to_string());
    let doc = Arc::new(doc);
    if let Ok(mut guard) = PRESET_CACHE.write() {
        *guard = Some(PresetCache { doc: doc.clone(), json: json.clone() });
    }
    (doc, json)
}

/// 作废缓存（`platform_preset` 写入后调用）。下一次读取重新从 DB 合并。
pub fn invalidate_presets_cache() {
    if let Ok(mut guard) = PRESET_CACHE.write() {
        *guard = None;
    }
}

/// 同步读取当前生效的 preset 文档：缓存（DB 合并视图）优先，未填充过回落编译期内置那份。
/// 代理热路径（`peak` / `models.peak`）用它，不做 DB IO。
pub fn effective_presets() -> Arc<Value> {
    match cached_presets() {
        Some((doc, _)) => doc,
        None => presets_arc().clone(),
    }
}

/// 用 per-platform JSON 文本组装 presets 文档（`{last_updated, protocols}`）。
/// bundled（[`presets`]）与 DB 同步后的 `platform_preset` 行共用这一处形状定义。
/// 单份文本解析失败 → 该协议整体跳过（DB 里的脏行不该炸掉整个文档）。
pub fn presets_doc<'a>(
    entries: impl IntoIterator<Item = (&'a str, &'a str)>,
    last_updated: Value,
) -> Value {
    let protocols: Map<String, Value> = entries
        .into_iter()
        .filter_map(|(code, json)| match serde_json::from_str::<Map<String, Value>>(json) {
            Ok(v) => Some((code.to_string(), Value::Object(v))),
            Err(e) => {
                tracing::warn!(error = %e, code, "platform preset json 解析失败，跳过该协议");
                None
            }
        })
        .collect();
    serde_json::json!({ "last_updated": last_updated, "protocols": protocols })
}

/// [`presets`] 的序列化文本，供命令层直接回传前端。
pub fn presets_json() -> &'static str {
    PRESETS_JSON.get_or_init(|| presets().to_string())
}

/// 按 protocol 名（serde rename 裸名）查 registry 默认端点。
/// 厂商直连平台（`Protocol::endpoints_locked()`）保存时强制用此值，忽略用户传入。
/// protocol 缺失 / 无 endpoints 字段 / 解析失败 → 空 Vec。
///
/// 读 [`effective_presets`]（DB 同步值优先）：否则平台保存会把刚同步下来的新 `base_url`
/// 重置回二进制内置的旧值。
pub fn default_endpoints(protocol: &str) -> Vec<crate::models::PlatformEndpoint> {
    endpoints_in(&effective_presets(), protocol)
}

/// [`default_endpoints`] 的纯函数核心：从任意一篇 presets 文档取某协议的默认端点。
pub fn endpoints_in(doc: &Value, protocol: &str) -> Vec<crate::models::PlatformEndpoint> {
    let Some(arr) = doc
        .get("protocols")
        .and_then(|p| p.get(protocol))
        .and_then(|e| e.get("endpoints"))
        .and_then(|e| e.get("default"))
    else {
        return Vec::new();
    };
    let raw: Vec<Value> = serde_json::from_value(arr.clone()).unwrap_or_else(|e| {
        tracing::warn!(error = %e, protocol, "registry endpoints parse failed; empty");
        Vec::new()
    });
    // registry JSON 已不存冗余 client_type：缺省按 endpoint protocol 派生（仅例外平台
    // 显式标注，如官方 claude_code 直连端点标 default 不模拟）。不补的话 serde default
    // 会落 "default"，厂商直连锁定端点丢失模拟客户端（headers / user_agent）行为。
    let filled: Vec<Value> = raw
        .into_iter()
        .map(|mut e| {
            let missing = e
                .get("client_type")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty);
            if missing && let Some(p) = e.get("protocol").and_then(Value::as_str) {
                e["client_type"] = Value::String(derive_client_type(p));
            }
            e
        })
        .collect();
    serde_json::from_value(Value::Array(filled)).unwrap_or_else(|e| {
        tracing::warn!(error = %e, protocol, "registry endpoints parse failed; empty");
        Vec::new()
    })
}

/// endpoint 协议 → 默认客户端形态（registry 无 client_type 时的缺省派生，与前端
/// `defaults.ts::clientTypeForProtocol` 对称）：anthropic → claude_code、openai 系 →
/// codex_tui、其余（gemini / 未知）→ default。
pub fn derive_client_type(endpoint_protocol: &str) -> String {
    match endpoint_protocol {
        "anthropic" => "claude_code".to_string(),
        "openai" | "openai_responses" | "openai_completions" => "codex_tui".to_string(),
        _ => "default".to_string(),
    }
}

/// `index.json` 的一条同步清单：远程同步照着它逐文件拉取。
///
/// `platform_file` 为 `None` 即 `pricing_only`（纯协议豁免，只拉 models 不拉 platform.json）。
/// 2026-08-31 起该清单为空：非纯协议平台一律登记进 platforms 并带 platform.json。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub code: String,
    pub platform_file: Option<String>,
    pub models_dir: String,
    /// 该平台 models 目录下的文件名清单（相对 `models_dir`）。
    pub models: Vec<String>,
}

/// 解析 `index.json` 的同步清单（`platforms` + `pricing_only` 合并，按 code 升序）。
/// 远程拉到的 index 与编译期内置的 index 共用这一处 schema 定义。
/// index.json 的 `last_updated`（Unix 秒）。同步用它判定远程 registry 是否比 DB 新；
/// 缺省或非法返回 None（调用方按「无法判定」处理，照常全量同步）。
pub fn parse_index_last_updated(json: &str) -> Option<i64> {
    serde_json::from_str::<Map<String, Value>>(json)
        .ok()?
        .get("last_updated")?
        .as_i64()
}

pub fn parse_index(json: &str) -> Result<Vec<IndexEntry>, String> {
    let root: Map<String, Value> = serde_json::from_str(json).map_err(|e| format!("index.json: {e}"))?;
    let mut out = Vec::new();
    for (key, with_platform) in [("platforms", true), ("pricing_only", false)] {
        let Some(arr) = root.get(key).and_then(Value::as_array) else {
            continue;
        };
        for e in arr {
            let Some(code) = e.get("code").and_then(Value::as_str) else {
                return Err(format!("index.json: {key} 条目缺 code"));
            };
            out.push(IndexEntry {
                code: code.to_string(),
                platform_file: with_platform
                    .then(|| e.get("platform_file").and_then(Value::as_str).map(str::to_string))
                    .flatten(),
                models_dir: e.get("models_dir").and_then(Value::as_str).unwrap_or_default().to_string(),
                models: e
                    .get("models")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
            });
        }
    }
    if out.is_empty() {
        return Err("index.json: 平台清单为空".into());
    }
    out.sort_by(|a, b| a.code.cmp(&b.code));
    Ok(out)
}

/// 编译期内置 `index.json` 的同步清单。漂移断言（清单 vs 磁盘实际文件）用。
pub fn bundled_index() -> &'static [IndexEntry] {
    static INDEX: OnceLock<Vec<IndexEntry>> = OnceLock::new();
    INDEX.get_or_init(|| parse_index(INDEX_JSON).expect("bundled index.json"))
}

/// 编译期内置的全部 `platform.json`：`(platform_code, JSON 文本)`，按 code 升序。
/// `platform_preset` 的 bundled 兜底用（DB 空时直接落成 `platform_preset` 行形状）。
pub fn bundled_platform_files() -> &'static [(&'static str, &'static str)] {
    PLATFORM_FILES
}

/// 编译期内置的全部模型条目文件：`(platform_code, 文件名, JSON 文本)`。
/// 同一 `platform_code` 会出现多次（该平台每个模型一条）。
pub fn bundled_model_files() -> &'static [(&'static str, &'static str, &'static str)] {
    MODEL_FILES
}

#[cfg(test)]
#[path = "test_registry.rs"]
mod test_registry;
