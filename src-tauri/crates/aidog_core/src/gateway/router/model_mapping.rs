//! 模型映射：根据平台模型配置自动匹配请求模型。

use super::super::models::*;

/// 根据平台模型配置自动匹配请求模型。
/// 匹配规则：请求模型名（小写）包含槽位名（opus/sonnet/haiku/gpt）→ 使用该槽位值；
/// 全部不匹配 → 使用 default；无 default → 透传原始模型（去掉 [... ] 后缀）。
#[cfg(test)]
pub(crate) fn resolve_model(models: &PlatformModels, source_model: &str) -> String {
    resolve_model_chain_with_custom(models, &[], source_model).0.swap_remove(0)
}

/// 自定义槽位定义（全局 settings `model` / `custom_model_slots` 的一项）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomSlotDef {
    pub name: String,
    /// true = 请求模型名（小写后）须等于槽名；false = 包含即命中。
    pub exact: bool,
}

/// 某平台上已填值的自定义槽位（已按匹配顺序排好：精确在前，包含按槽名长度降序，同长按定义序）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomSlotEntry {
    pub name: String,
    pub exact: bool,
    pub chain: Vec<String>,
}

/// 内置槽名（与前端 `BUILTIN_SLOT_NAMES` / `ModelSlot` 一致），自定义槽不得重名。
const BUILTIN_SLOT_NAMES: [&str; 8] =
    ["default", "opus", "sonnet", "haiku", "gpt", "jev", "embedding", "rerank"];

fn valid_slot_name(n: &str) -> bool {
    (1..=24).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// 解析全局槽位定义。非数组 / 缺名字 / 空名字的项跳过；`match` 缺省或未知 = contains。
/// 名字不合 `^[a-z0-9-]{1,24}$`、与内置槽重名、或重复（保留先出现者）的项 warn 后跳过。
pub(crate) fn parse_custom_defs(v: Option<&serde_json::Value>) -> Vec<CustomSlotDef> {
    let Some(arr) = v.and_then(|v| v.as_array()) else { return Vec::new() };
    let mut out: Vec<CustomSlotDef> = Vec::new();
    for item in arr {
        let Some(name) = item.get("name").and_then(|n| n.as_str()).map(str::trim).filter(|n| !n.is_empty()) else {
            continue;
        };
        if !valid_slot_name(name) || BUILTIN_SLOT_NAMES.contains(&name) || out.iter().any(|d| d.name == name) {
            tracing::warn!(slot = %name, "custom_model_slots: invalid, builtin-colliding or duplicate slot name; skipped");
            continue;
        }
        out.push(CustomSlotDef {
            name: name.to_string(),
            exact: item.get("match").and_then(|m| m.as_str()) == Some("exact"),
        });
    }
    out
}

/// 由全局定义 + 平台 `extra.custom_slots` 生成该平台的有序命中表。无值的槽不入表（= 未命中）；
/// extra 里无定义的孤儿键忽略并 warn。
pub(crate) fn build_custom_entries(
    defs: &[CustomSlotDef],
    values: Option<&serde_json::Value>,
) -> Vec<CustomSlotEntry> {
    let Some(map) = values.and_then(|v| v.as_object()) else { return Vec::new() };
    for key in map.keys().filter(|k| !defs.iter().any(|d| &d.name == *k)) {
        tracing::warn!(slot = %key, "platform.extra.custom_slots has key without global definition; ignored");
    }
    let mut out: Vec<CustomSlotEntry> = defs
        .iter()
        .filter_map(|d| {
            let raw: Vec<String> = match map.get(&d.name)? {
                serde_json::Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
                serde_json::Value::String(s) => vec![s.clone()],
                _ => return None,
            };
            let chain = slot_chain(&raw);
            (!chain.is_empty()).then(|| CustomSlotEntry { name: d.name.clone(), exact: d.exact, chain })
        })
        .collect();
    // 稳定排序：精确在前；包含按槽名长度降序；同组同长保持定义序。
    out.sort_by_key(|e| (!e.exact, std::cmp::Reverse(if e.exact { 0 } else { e.name.len() })));
    out
}

/// 在有序命中表里找第一个命中请求名（已小写）的槽位。
pub(crate) fn match_custom<'a>(
    entries: &'a [CustomSlotEntry],
    lower: &str,
) -> Option<&'a CustomSlotEntry> {
    entries.iter().find(|e| if e.exact { lower == e.name } else { lower.contains(e.name.as_str()) })
}

/// 内置档位族（槽位名 = 请求名子串）。
pub(crate) const TIER_FAMILIES: [&str; 4] = ["opus", "sonnet", "haiku", "gpt"];

/// 去掉 Claude Code budget 后缀（如 `[1m]`、`[128k]`）。
fn base_model_of(source_model: &str) -> &str {
    source_model.split('[').next().unwrap_or(source_model)
}

/// 请求名所属档位族（与 `resolve_model_chain_with_custom` 同一子串规则，不看槽位是否已填）。
pub(crate) fn request_tier_families(source_model: &str) -> Vec<&'static str> {
    let lower = base_model_of(source_model).to_lowercase();
    TIER_FAMILIES.into_iter().filter(|f| lower.contains(f)).collect()
}

/// 请求名命中的全局自定义槽名（不看本平台是否填值；规则同 `match_custom`）。
pub(crate) fn custom_defs_hit<'a>(
    defs: &'a [CustomSlotDef],
    source_model: &str,
) -> impl Iterator<Item = &'a str> {
    let lower = base_model_of(source_model).to_lowercase();
    defs.iter()
        .filter(move |d| if d.exact { lower == d.name } else { lower.contains(d.name.as_str()) })
        .map(|d| d.name.as_str())
}

/// 返回命中槽位的完整有序模型链（链首 = 目标模型，恒非空）+ 命中来源。
/// 顺序：自定义槽 → 内置 opus/sonnet/haiku/gpt → default → 透传（无槽位可用 → 透传名单元素链）。
pub(crate) fn resolve_model_chain_with_custom(
    models: &PlatformModels,
    custom: &[CustomSlotEntry],
    source_model: &str,
) -> (Vec<String>, aidog_db::models::ModelVia) {
    use aidog_db::models::ModelVia;
    let base_model = base_model_of(source_model);
    let lower = base_model.to_lowercase();
    if let Some(e) = match_custom(custom, &lower) {
        return (e.chain.clone(), ModelVia::SlotCustom(e.name.clone()));
    }
    let slots: [(&str, &Vec<String>, ModelVia); 4] = [
        (TIER_FAMILIES[0], &models.opus, ModelVia::SlotOpus),
        (TIER_FAMILIES[1], &models.sonnet, ModelVia::SlotSonnet),
        (TIER_FAMILIES[2], &models.haiku, ModelVia::SlotHaiku),
        (TIER_FAMILIES[3], &models.gpt, ModelVia::SlotGpt),
    ];
    for (slot_name, slot_value, via) in &slots {
        if lower.contains(slot_name)
            && slot_filled(slot_value).is_some()
        {
            return (slot_chain(slot_value), via.clone());
        }
    }
    // 回退到 default
    if slot_filled(&models.default).is_some() {
        return (slot_chain(&models.default), ModelVia::SlotDefault);
    }
    // 无匹配无 default — 透传（去掉 budget 后缀）
    (vec![base_model.to_string()], ModelVia::Passthrough)
}

/// 决策请求（/v1/systemone）的目标模型解析（jev-decision-proxy R8）。不走 opus/sonnet 子串匹配：
/// - 平台配了 `jev` 槽位 → **总是**用槽位值（版本固定，不透传请求模型）；
/// - 未配（靠 available_models 推导支持决策）→ 请求模型在 `available_models` 里就用它；
/// - 否则用 `available_models` 中第一个 decision 能力模型；
/// - 都不满足（理论不可达：路由过滤已剔除不支持决策的平台）→ 透传原始模型名（剥 budget 后缀）。
///
/// `decision_models`：registry decision 能力模型（按平台键控），
/// 由 `select_candidates_ctx` 入口一次性查库构建，禁逐平台查库。
pub(crate) fn resolve_decision_model(
    models: &PlatformModels,
    platform: &Platform,
    decision_models: &super::candidates::DecisionModels,
    source_model: &str,
) -> String {
    if let Some(jev) = models.jev_slot() {
        return jev.to_string();
    }
    let base = source_model.split('[').next().unwrap_or(source_model);
    if !base.is_empty() && platform.available_models.iter().any(|m| m == base) {
        return base.to_string();
    }
    if let Some(first) = decision_models.first_in(platform, &platform.available_models) {
        return first.clone();
    }
    base.to_string()
}
