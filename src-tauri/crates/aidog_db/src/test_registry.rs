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
    assert_eq!(protocols().len(), PLATFORM_FILES.len(), "有 platform.json 解析失败被跳过");
}

/// index.json 的 `models` 数组是远程同步的逐文件拉取清单：漏一个文件 = 该模型永远同步不下来，
/// 而 bundled 读取层照样有（build.rs 自动发现），线上才暴露。这里锁死两者零差集。
#[test]
fn index_model_list_matches_model_files() {
    let mut on_disk: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
    for (code, file, _) in MODEL_FILES {
        on_disk.entry(code).or_default().push(file);
    }
    for e in bundled_index() {
        let mut listed = e.models.clone();
        listed.sort();
        let mut found = on_disk.remove(e.code.as_str()).unwrap_or_default();
        found.sort_unstable();
        assert_eq!(listed, found, "{} 的 index.json models 清单与磁盘文件不一致", e.code);
    }
    assert!(on_disk.is_empty(), "这些平台目录未登记进 index.json: {:?}", on_disk.keys());
}

/// `pricing_only`（litellm / meta / mistral）只出模型条目，不进协议选择器。
#[test]
fn pricing_only_entries_carry_models_without_platform_file() {
    let idx = bundled_index();
    for code in ["litellm", "meta", "mistral"] {
        let e = idx.iter().find(|e| e.code == code).expect(code);
        assert!(e.platform_file.is_none(), "{code} 不该有 platform.json");
        assert!(!e.models.is_empty(), "{code} 模型清单不可为空");
    }
    assert!(idx.iter().find(|e| e.code == "anthropic").expect("anthropic").platform_file.is_some());
}

/// DB 同步后的 presets 文档与 bundled 同形状；单份脏 JSON 只丢该协议，不炸整篇。
#[test]
fn presets_doc_skips_unparsable_entry() {
    let doc = presets_doc(
        [("good", r#"{"name":{"en-US":"Good"}}"#), ("bad", "{oops")],
        Value::String("1".into()),
        Value::from(7),
    );
    assert_eq!(doc["version"], "1");
    assert_eq!(doc["last_updated"], 7);
    let p = doc["protocols"].as_object().expect("protocols");
    assert_eq!(p.len(), 1);
    assert_eq!(p["good"]["name"]["en-US"], "Good");
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

/// 每个 `model_id` 至少有一个 `official` 平台条目（模型维度列表默认展示官方条目，
/// 一个都没有 → 该模型在 UI 里挑不出默认平台）。旧断言经 `registry::model_entry`
/// 归并视图间接验证，该视图随票 T6 删除，这里直接数原始文件。
#[test]
fn every_model_has_an_official_platform() {
    let mut seen = std::collections::HashSet::new();
    let mut official: std::collections::BTreeMap<String, Vec<&str>> = Default::default();
    for (code, _file, json) in MODEL_FILES {
        let e = parse(code, json);
        let id = e["model_id"].as_str().expect("model_id");
        assert!(seen.insert(format!("{code}/{id}")), "{code} 内 model_id `{id}` 重复");
        assert!(e["capabilities"].is_array(), "{code}/{id} 缺 capabilities");
        let slot = official.entry(id.to_string()).or_default();
        if e["official"] == Value::Bool(true) {
            slot.push(code);
        }
    }
    for (id, codes) in &official {
        assert!(!codes.is_empty(), "{id} 无 official 条目");
    }
}
