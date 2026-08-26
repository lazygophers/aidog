//! registry 真值源（`src-tauri/defaults/registry/`）的只读合并视图。
//!
//! 真值源人工维护：`index.json` + `platforms/<code>/platform.json` + `platforms/<code>/models/<model>.json`。
//! 全部文件由 `build.rs` 在编译期枚举并 `include_str!`（新增文件自动纳入，无需改 Rust）。
//! 模型文件名只求稳定唯一，真值是文件内的 `model_id`；同目录大小写撞名的后一个带 `~N` 后缀
//! （macOS 文件系统大小写不敏感，如 `MiniMax-M2` / `minimax-m2`）。
//!
//! 本模块提供两个合并视图，供既有消费方零改动接入：
//! - [`presets`] / [`presets_json`]：等价于旧 `platform-presets.json`（`{version, last_updated, protocols}`）
//! - [`model_entry`]：等价于旧 `models.json` 的单模型节点（跨平台条目按 `model_id` 归并回 `pricing` 映射）

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/registry_includes.rs"));

const PRICE_FIELDS: [&str; 4] = [
    "input_cost_per_token",
    "output_cost_per_token",
    "cache_read_input_token_cost",
    "cache_creation_input_token_cost",
];
const LIMIT_FIELDS: [&str; 3] = ["max_input_tokens", "max_output_tokens", "context_window"];

static PRESETS: OnceLock<Value> = OnceLock::new();
static PRESETS_JSON: OnceLock<String> = OnceLock::new();
static MODELS: OnceLock<HashMap<String, Value>> = OnceLock::new();

fn parse(what: &str, json: &str) -> Map<String, Value> {
    match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => panic!("registry {what}: {e}"),
    }
}

/// 合并 65 份 `platform.json` → 旧 presets 文档。首次访问解析一次，后续共享同一静态实例。
pub fn presets() -> &'static Value {
    PRESETS.get_or_init(|| {
        let index = parse("index.json", INDEX_JSON);
        presets_doc(
            PLATFORM_FILES.iter().map(|(code, json)| (*code, *json)),
            index["version"].clone(),
            index["last_updated"].clone(),
        )
    })
}

/// 用 per-platform JSON 文本组装 presets 文档（`{version, last_updated, protocols}`）。
/// bundled（[`presets`]）与 DB 同步后的 `platform_preset` 行共用这一处形状定义。
/// 单份文本解析失败 → 该协议整体跳过（DB 里的脏行不该炸掉整个文档）。
pub fn presets_doc<'a>(
    entries: impl IntoIterator<Item = (&'a str, &'a str)>,
    version: Value,
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
    serde_json::json!({ "version": version, "last_updated": last_updated, "protocols": protocols })
}

/// [`presets`] 的序列化文本，供命令层直接回传前端。
pub fn presets_json() -> &'static str {
    PRESETS_JSON.get_or_init(|| presets().to_string())
}

/// 按 protocol 名（serde rename 裸名）查 registry 默认端点。
/// 厂商直连平台（`Protocol::endpoints_locked()`）保存时强制用此值，忽略用户传入。
/// protocol 缺失 / 无 endpoints 字段 / 解析失败 → 空 Vec。
pub fn default_endpoints(protocol: &str) -> Vec<crate::models::PlatformEndpoint> {
    let Some(arr) = presets()
        .get("protocols")
        .and_then(|p| p.get(protocol))
        .and_then(|e| e.get("endpoints"))
        .and_then(|e| e.get("default"))
    else {
        return Vec::new();
    };
    serde_json::from_value(arr.clone()).unwrap_or_else(|e| {
        tracing::warn!(error = %e, protocol, "registry endpoints parse failed; empty");
        Vec::new()
    })
}

/// 平台展示名，三层回落收敛在此：`name[locale]` → `name["en-US"]` → 平台 code。
/// 调用方只传当前 locale（`Lang::from_locale` 归一，`zh-CN` / `ja` 等变体同样命中），
/// 不再各写回落分支。协议不存在或 `name` 整体缺失 → 返回 code 本身，UI 不出空白。
pub fn platform_display_name(code: &str, locale: &str) -> String {
    let entry = presets().get("protocols").and_then(|p| p.get(code));
    resolve_name(entry, code, locale)
}

fn resolve_name(entry: Option<&Value>, code: &str, locale: &str) -> String {
    let key = aidog_i18n::Lang::from_locale(locale).locale_key();
    let name = entry.and_then(|e| e.get("name"));
    let pick = |l: &str| {
        name.and_then(|n| n.get(l))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    pick(key).or_else(|| pick("en-US")).unwrap_or(code).to_string()
}

/// `index.json` 的一条同步清单：远程同步照着它逐文件拉取。
///
/// `platform_file` 为 `None` 即 `pricing_only`（litellm / meta / mistral 这类只提供比价条目、
/// 不是可选协议的来源），只拉 models 不拉 platform.json。
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
/// `model_entry` 的 bundled 兜底用（DB 空时直接落成 `platform_preset` 行形状）。
pub fn bundled_platform_files() -> &'static [(&'static str, &'static str)] {
    PLATFORM_FILES
}

/// 编译期内置的全部模型条目文件：`(platform_code, 文件名, JSON 文本)`。
/// 同一 `platform_code` 会出现多次（该平台每个模型一条）。
pub fn bundled_model_files() -> &'static [(&'static str, &'static str, &'static str)] {
    MODEL_FILES
}

/// registry 里该模型的合并 price_data 节点（跨平台条目归并）。DB 未同步时的只读兜底
/// （`resolve_price`：DB 无该模型行才读，DB 恒优先）。
pub fn model_entry(name: &str) -> Option<&'static Value> {
    MODELS.get_or_init(merge_models).get(name)
}

/// 把 per-platform 条目按 `model_id` 归并回旧 `models.json` 单模型形状：
/// `official` 条目提供 `default_platform` / 上下文限制 / `context_tiers` / 顶层通用价
/// （`default_price` 缺省即该条目自身价），全部条目各自落进 `pricing[<code>]`。
fn merge_models() -> HashMap<String, Value> {
    let mut out: HashMap<String, Map<String, Value>> = HashMap::new();
    for (code, _file, json) in MODEL_FILES {
        let e = parse(code, json);
        let id = e["model_id"].as_str().expect("model_id").to_string();
        let prices: Map<String, Value> = PRICE_FIELDS
            .iter()
            .filter_map(|f| e.get(*f).map(|v| ((*f).to_string(), v.clone())))
            .collect();

        let entry = out.entry(id).or_default();
        let mut node = prices.clone();
        if let Some(t) = e.get("time_tiers") {
            node.insert("time_tiers".to_string(), t.clone());
        }
        entry
            .entry("pricing")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("pricing object")
            .insert((*code).to_string(), Value::Object(node));

        if e["official"] == Value::Bool(true) {
            entry.insert("default_platform".to_string(), Value::String((*code).to_string()));
            entry.extend(match e.get("default_price") {
                Some(Value::Object(top)) => top.clone(),
                _ => prices,
            });
            entry.extend(LIMIT_FIELDS.iter().filter_map(|f| e.get(*f).map(|v| ((*f).to_string(), v.clone()))));
            entry.insert(
                "context_tiers".to_string(),
                e.get("context_tiers").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
            );
        }
    }
    out.into_iter().map(|(k, v)| (k, Value::Object(v))).collect()
}

#[cfg(test)]
#[path = "test_registry.rs"]
mod test_registry;
