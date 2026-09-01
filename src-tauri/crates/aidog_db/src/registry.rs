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

use serde::{Deserialize, Serialize};
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

/// 一份 platform.json 文本顶层的 `last_updated`（Unix 秒）；缺失 / 非数字 → 0。
/// 新旧比较用：registry 约定每个数据文件顶层必带该戳（见 CLAUDE.md「平台默认配置」）。
fn last_updated_of(platform_json: &str) -> i64 {
    serde_json::from_str::<Map<String, Value>>(platform_json)
        .ok()
        .and_then(|m| m.get("last_updated").and_then(Value::as_i64))
        .unwrap_or(0)
}

/// bundled + DB 行合并：同 `code` **取两者中较新的那份**（比 `last_updated` 戳），
/// bundled 里 DB 缺的补齐（票 13-C）。
/// DB 一份都没有时结果与 [`presets`] 逐字节相同（首次同步失败不该让协议下拉少几项）。
///
/// 为什么不是「DB 行无条件覆盖」：同步源是上游仓库，二进制里的 bundled 可能比上游更新
/// （本地新增字段尚未发布、或用户升级了版本而上游那份还没动）。无条件覆盖会让新字段
/// 被旧行整篇盖掉，症状是新加的能力在界面上凭空消失（如 quota_scripts 变体下拉不出现）。
///
/// `db_last_updated_secs` = DB 行 `updated_at` 最大值（秒）；文档级 `last_updated` 取它与
/// bundled `index.json` 的较大值（`None` = 没有 DB 行 → 用 bundled 那个）。
pub fn merge_presets_doc<'a>(
    db_rows: impl IntoIterator<Item = (&'a str, &'a str)>,
    db_last_updated_secs: Option<i64>,
) -> Value {
    let mut entries: BTreeMap<&str, &str> = PLATFORM_FILES.iter().map(|(c, j)| (*c, *j)).collect();
    for (code, json) in db_rows {
        match entries.get(code) {
            // bundled 更新 → 保留 bundled（同戳按 DB 走，维持旧行为）
            Some(bundled) if last_updated_of(bundled) > last_updated_of(json) => {
                tracing::debug!(code, "bundled preset 比 DB 行新，保留 bundled");
            }
            _ => {
                entries.insert(code, json);
            }
        }
    }
    let index = parse("index.json", INDEX_JSON);
    let bundled_last_updated = index["last_updated"].as_i64().unwrap_or(0);
    let last_updated = match db_last_updated_secs {
        Some(secs) => Value::from(secs.max(bundled_last_updated)),
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

// ── 配额查询脚本（platform.json 顶层 `quota_scripts`）──────────────

/// quota 脚本的用户参数声明（`quota_scripts[].requires` 一项）。
/// 值由用户在前端按选中变体填写，存 `platform.extra.<key>`，脚本经 `ctx.extra` 读取。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaScriptParam {
    pub key: String,
    /// 8 locale 显示标签（同品牌 `name` 的惯例形状）。
    pub label: BTreeMap<String, String>,
}

/// quota 脚本能力声明（`quota_scripts[].returns`）。前端查询入口是否渲染
/// 由该声明驱动（索引只存 `capable` 汇总布尔）。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct QuotaScriptReturns {
    #[serde(default)]
    pub balance: bool,
    #[serde(default)]
    pub coding_plan: bool,
    #[serde(default)]
    pub mcp: bool,
    /// 配额层级名（`QuotaTier.name` 词表：five_hour / weekly_limit / monthly / mcp_monthly）。
    #[serde(default)]
    pub tiers: Vec<String>,
}

/// platform.json 顶层 `quota_scripts` 一条：一个部署变体一份自包含 JS 脚本
/// （内部可多次调上游接口再汇总；同族协议各自文件复制正文，无跨文件引用）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaScriptVariant {
    pub id: String,
    /// 8 locale 变体显示名。
    pub name: BTreeMap<String, String>,
    #[serde(default)]
    pub requires: Vec<QuotaScriptParam>,
    #[serde(default)]
    pub returns: QuotaScriptReturns,
    pub script: String,
}

/// 解析一份 platform.json 文本顶层的 `quota_scripts`。
/// 无该字段 / 单份解析失败 → warn 后返回空 Vec（与 [`endpoints_in`] 同 idiom：
/// DB 里远程同步来的脏行不该炸掉调用方）。运行时读取应取自 [`effective_presets`]
/// 生效文档（DB 同步值优先），禁直读 bundled。
pub fn parse_quota_scripts(platform_json: &str) -> Vec<QuotaScriptVariant> {
    let Ok(map) = serde_json::from_str::<Map<String, Value>>(platform_json) else {
        return Vec::new();
    };
    let Some(arr) = map.get("quota_scripts") else {
        return Vec::new();
    };
    serde_json::from_value(arr.clone()).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "registry quota_scripts parse failed; empty");
        Vec::new()
    })
}

/// 从任意一篇 presets 文档取某协议的 quota 脚本变体列表（[`endpoints_in`] 同 idiom：
/// 无该字段 / 解析失败 → 空 Vec）。运行时读取应取自 [`effective_presets`]（DB 同步值优先）。
pub fn quota_scripts_in(doc: &Value, protocol: &str) -> Vec<QuotaScriptVariant> {
    let Some(arr) = doc
        .get("protocols")
        .and_then(|p| p.get(protocol))
        .and_then(|e| e.get("quota_scripts"))
    else {
        return Vec::new();
    };
    serde_json::from_value(arr.clone()).unwrap_or_else(|e| {
        tracing::warn!(error = %e, protocol, "registry quota_scripts parse failed; empty");
        Vec::new()
    })
}

/// 变体选中语义（spec「变体选择」）：`quota_script_id` 命中取该条；缺省 / id 失效
/// （远程改名删条）回落数组首条。空数组 → None。
pub fn select_quota_variant<'a>(
    variants: &'a [QuotaScriptVariant],
    id: Option<&str>,
) -> Option<&'a QuotaScriptVariant> {
    variants
        .iter()
        .find(|v| Some(v.id.as_str()) == id)
        .or_else(|| variants.first())
}

/// base_url 启发式分派（数据驱动）：读生效 preset 文档各协议顶层 `quota_url_match`
/// 关键词数组，小写子串匹配（`base_url_lower` 调用方先转小写）命中的首个协议 code。
/// 同族多协议共享关键词时取文档序首个（serde_json Map 排序 = 协议名序，base 变体
/// 排在 coding/_en 变体前，如 bigmodel.cn → glm / api.kimi.com/coding → kimi）。
/// 平台匹配词一律 registry 数据驱动，禁在代码硬编码。无命中 → None。
pub fn quota_code_for_base_url(base_url_lower: &str) -> Option<String> {
    let doc = effective_presets();
    let protocols = doc.get("protocols")?.as_object()?;
    for (code, entry) in protocols {
        let hit = entry
            .get("quota_url_match")
            .and_then(Value::as_array)
            .is_some_and(|kws| {
                kws.iter()
                    .filter_map(Value::as_str)
                    .any(|k| base_url_lower.contains(k))
            });
        if hit {
            return Some(code.clone());
        }
    }
    None
}

/// 读 `platform.extra` 顶层小字符串键（`quota_script_id` / `quota_custom_script` 等未建模键，
/// 天然落在 `PlatformExtra::rest`）。extra 非 JSON / 键非字符串 → None。
fn extra_str_key(extra_json: &str, key: &str) -> Option<String> {
    serde_json::from_str::<Map<String, Value>>(extra_json)
        .ok()?
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// 解析某平台执行用的 quota 脚本正文（回落链，spec「存储」决策）：
/// ① 物化列非空 → 用之（用户保存时固化，远程同步不换）；
/// ② `extra.quota_custom_script` 非空 → 用户手写脚本；
/// ③ [`select_quota_variant`] 选中变体（`extra.quota_script_id` → 首条，零配置开箱即用）。
/// 返回 None = 该协议无任何脚本（调用方维持原 err 行为）。
pub fn resolve_quota_script(protocol: &str, extra_json: &str, materialized: &str) -> Option<String> {
    if !materialized.trim().is_empty() {
        return Some(materialized.to_string());
    }
    if let Some(custom) = extra_str_key(extra_json, "quota_custom_script")
        .filter(|s| !s.trim().is_empty())
    {
        return Some(custom);
    }
    let variants = quota_scripts_in(&effective_presets(), protocol);
    let id = extra_str_key(extra_json, "quota_script_id");
    select_quota_variant(&variants, id.as_deref()).map(|v| v.script.clone())
}

/// 用户保存平台（create/update）时物化 `platform.quota_script` 列的取值：
/// - `extra.quota_custom_script` 非空 → 物化用户手写脚本（优先于变体）；
/// - 否则 `extra.quota_script_id` 有值（用户显式选过变体，远程更新待拉入）或列空
///   （从未物化）/ 协议变更（旧列是别的协议的脚本）→ 写入选中（或首条）变体正文；
/// - 否则（无 id 且列已有值）保留现值——变体正文已在保存时固化，远程同步不自动换脚本。
///
/// 无脚本协议 → 空串（清列）。
pub fn materialize_quota_script(
    protocol: &str,
    extra_json: &str,
    current: &str,
    type_changed: bool,
) -> String {
    if let Some(custom) = extra_str_key(extra_json, "quota_custom_script")
        .filter(|s| !s.trim().is_empty())
    {
        return custom;
    }
    let id = extra_str_key(extra_json, "quota_script_id");
    if id.is_none() && !current.is_empty() && !type_changed {
        return current.to_string();
    }
    let variants = quota_scripts_in(&effective_presets(), protocol);
    select_quota_variant(&variants, id.as_deref())
        .map(|v| v.script.clone())
        .unwrap_or_default()
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
