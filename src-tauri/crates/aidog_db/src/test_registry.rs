#![cfg(test)]
use super::*;

const LOCALES: [&str; 8] = ["en-US", "zh-Hans", "ar-SA", "fr-FR", "de-DE", "ru-RU", "ja-JP", "es-ES"];

fn protocols() -> &'static Map<String, Value> {
    presets()["protocols"].as_object().expect("protocols object")
}

#[test]
fn index_platform_list_matches_platform_files() {
    let index = parse("index.json", INDEX_JSON);
    let listed: Vec<&str> = index["platforms"]
        .as_array()
        .expect("platforms array")
        .iter()
        .map(|p| p["code"].as_str().expect("code"))
        .collect();
    let mut found: Vec<&str> = PLATFORM_FILES.iter().map(|(c, _)| *c).collect();
    found.sort_unstable();
    assert_eq!(listed, found, "index.json 平台清单须与 platforms/*/platform.json 一致");
}

#[test]
fn every_protocol_has_full_brand_fields() {
    for (code, entry) in protocols() {
        for field in ["logo_url", "homepage", "color"] {
            assert!(entry[field].is_string(), "{code}.{field} 须为字符串");
        }
        assert!(entry["keywords"].is_array(), "{code}.keywords 须为数组");
        assert!(entry["source_urls"]["docs"].is_string(), "{code}.source_urls.docs 缺失");
        for l in LOCALES {
            let name = entry["name"][l].as_str().unwrap_or("");
            assert!(!name.trim().is_empty(), "{code}.name.{l} 缺失或空");
        }
        assert!(!entry["homepage"].as_str().expect("homepage").is_empty(), "{code}.homepage 空");
        assert!(!entry["color"].as_str().expect("color").is_empty(), "{code}.color 空");
    }
}

/// 三层回落第一层：命中当前 locale。
#[test]
fn display_name_uses_current_locale() {
    assert_eq!(platform_display_name("glm_coding", "ja-JP"), "GLM コーディングプラン（智譜）");
    // locale 变体经 Lang::from_locale 归一后同样命中
    assert_eq!(platform_display_name("glm_coding", "zh-CN"), "GLM 编码套餐（智谱）");
}

/// 第二层：缺当前 locale（或值空白）取 en-US。
#[test]
fn display_name_falls_back_to_en_us() {
    let entry = serde_json::json!({ "name": { "en-US": "Zhipu AI", "ja-JP": "  " } });
    assert_eq!(resolve_name(Some(&entry), "glm", "de-DE"), "Zhipu AI");
    assert_eq!(resolve_name(Some(&entry), "glm", "ja-JP"), "Zhipu AI");
}

/// 第三层：`name` 整体缺失（或协议不存在）取平台 code，UI 不出空白。
#[test]
fn display_name_falls_back_to_code() {
    assert_eq!(resolve_name(Some(&serde_json::json!({})), "mock", "en-US"), "mock");
    assert_eq!(resolve_name(None, "no_such_protocol", "zh-Hans"), "no_such_protocol");
    assert_eq!(platform_display_name("no_such_protocol", "fr-FR"), "no_such_protocol");
}

/// R12：`endpoints_locked()` 协议保存时强制用 preset 端点，读空会清空用户端点。
#[test]
fn default_endpoints_present_for_direct_vendors() {
    for code in ["anthropic", "openai", "gemini", "deepseek", "glm_coding"] {
        assert!(!default_endpoints(code).is_empty(), "{code} 默认端点不可为空");
    }
}

/// glm_coding 是唯一带 peak_hours 与 models.peak 的协议（路由/计费硬依赖）。
#[test]
fn glm_coding_keeps_peak_branches() {
    let glm = &protocols()["glm_coding"];
    assert!(glm["peak_hours"].as_array().is_some_and(|a| !a.is_empty()));
    assert_eq!(glm["models"]["peak"]["default"], "glm-4.7");
    assert_eq!(glm["is_coding_plan"], true);
}

#[test]
fn model_entry_merges_cross_platform_pricing() {
    let e = model_entry("glm-4.6").expect("glm-4.6");
    assert_eq!(e["default_platform"], "glm");
    assert_eq!(e["input_cost_per_token"], 6e-7);
    assert_eq!(e["max_input_tokens"], 200000);
    assert_eq!(e["context_tiers"], serde_json::json!([]));
    let pricing = e["pricing"].as_object().expect("pricing");
    let mut codes: Vec<&String> = pricing.keys().collect();
    codes.sort();
    assert_eq!(codes, ["glm", "litellm", "openrouter"]);
}

/// `time_tiers`（含内嵌 context_tiers）只挂在提供它的平台条目上，不可提升到模型顶层。
#[test]
fn model_entry_keeps_platform_scoped_time_tiers() {
    let e = model_entry("glm-5-turbo").expect("glm-5-turbo");
    assert!(e.get("time_tiers").is_none(), "time_tiers 属平台条目，不入模型顶层");
    let tt = e["pricing"]["glm_coding"]["time_tiers"].as_array().expect("time_tiers");
    assert_eq!(tt[0]["start_at"], 1790784000_i64);
    assert!(tt[0]["context_tiers"].as_array().is_some_and(|a| !a.is_empty()));
    assert_eq!(e["context_tiers"].as_array().expect("context_tiers").len(), 1);
}

/// `official` 条目的 `default_price` 承载旧 models.json 顶层通用价（未匹配平台时的回退价），
/// 与该条目自身平台价不同才写。
#[test]
fn model_entry_top_level_price_differs_from_official_platform_price() {
    let e = model_entry("gemini-2.0-flash").expect("gemini-2.0-flash");
    assert_eq!(e["cache_read_input_token_cost"], 2.5e-8);
    assert!(e["pricing"]["gemini"].get("cache_read_input_token_cost").is_none());
}

#[test]
fn every_model_has_exactly_one_official_platform() {
    let mut ids = std::collections::HashSet::new();
    for (code, json) in MODEL_FILES {
        let e = parse(code, json);
        let id = e["model_id"].as_str().expect("model_id");
        assert!(ids.insert(format!("{code}/{id}")), "{code} 内 model_id `{id}` 重复");
        assert!(e["capabilities"].is_array(), "{code}/{id} 缺 capabilities");
    }
    for id in ids.iter().map(|k| k.split_once('/').expect("key").1) {
        let e = model_entry(id).expect(id);
        assert!(e["default_platform"].is_string(), "{id} 无 official 条目");
    }
}
