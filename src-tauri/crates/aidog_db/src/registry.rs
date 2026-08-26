//! registry 真值源（`src-tauri/defaults/registry/`）的只读合并视图。
//!
//! 真值源人工维护：`index.json` + `platforms/<code>/platform.json` + `platforms/<code>/models/<model>.json`。
//! 全部文件由 `build.rs` 在编译期枚举并 `include_str!`（新增文件自动纳入，无需改 Rust）。
//! 模型文件名只求稳定唯一，真值是文件内的 `model_id`；同目录大小写撞名的后一个带 `~N` 后缀
//! （macOS 文件系统大小写不敏感，如 `MiniMax-M2` / `minimax-m2`）。
//!
//! 本模块提供一个合并视图 [`presets`] / [`presets_json`]，等价于旧 `platform-presets.json`
//! （`{version, last_updated, protocols}`），供既有消费方零改动接入。
//!
//! 模型条目**不再有合并视图**：旧 `models.json` 单模型形状（跨平台条目按 `model_id` 归并回
//! `pricing` 映射）随票 T4 把计费切到 `model_entry` 表一并废弃，本模块只出 [`bundled_model_files`]
//! 原始文本，归并逻辑不复存在（同一模型在不同平台是不同的行，不需要合并）。

use serde_json::{Map, Value};
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/registry_includes.rs"));

static PRESETS: OnceLock<Value> = OnceLock::new();
static PRESETS_JSON: OnceLock<String> = OnceLock::new();

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
