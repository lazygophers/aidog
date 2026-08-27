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

#[test]
fn known_simpleicons_slugs_are_valid() {
    // 只校验 registry 中手填过、且 Simple Icons 已收录的 slug；空值仍走 favicon/clearbit/首字母 fallback。
    let allowed = [
        "alibabacloud",
        "anthropic",
        "baidu",
        "bytedance",
        "claude",
        "claudecode",
        "codemirror",
        "deepseek",
        "googlegemini",
        "minimax",
        "modelscope",
        "moonshotai",
        "nebula",
        "nvidia",
        "opencode",
        "openrouter",
        "xiaomi",
        "zai",
    ];
    let allowed: std::collections::BTreeSet<_> = allowed.into_iter().collect();

    for (code, entry) in protocols() {
        let slug = entry["logo_url"].as_str().expect("logo_url").trim();
        if slug.is_empty() {
            continue;
        }
        assert!(allowed.contains(slug), "{code}.logo_url `{slug}` 不在已核验 Simple Icons slug 白名单");
    }
}

/// build.rs 生成的模型文件相对路径必须用 `/`（Windows 上 `strip_prefix` 会留 `\`，
/// 与 index.json 清单零差集断言不符，且远程同步会拼出 404 的 URL）。
#[test]
fn model_file_paths_use_forward_slash() {
    for (code, file, _) in MODEL_FILES {
        assert!(!file.contains('\\'), "{code}/{file} 路径分隔符须归一成 /");
    }
    // vendor 子目录的模型确实存在，否则本断言恒真而无意义
    assert!(MODEL_FILES.iter().any(|(_, f, _)| f.contains('/')), "registry 应有 vendor 子目录模型");
}

/// 票 13-C：DB 行覆盖 bundled 同 code，bundled 里 DB 缺的补齐。
#[test]
fn merge_presets_doc_unions_db_over_bundled() {
    // DB 一行都没有 → 与 bundled 逐字节相同
    assert_eq!(merge_presets_doc([], None).to_string(), presets_json());

    let doc = merge_presets_doc([("anthropic", r#"{"homepage":"https://from-db"}"#)], Some(1234));
    let p = doc["protocols"].as_object().expect("protocols");
    // 覆盖：DB 那条整份替换
    assert_eq!(p["anthropic"]["homepage"], "https://from-db");
    // 补齐：DB 没有的协议照样在
    assert_eq!(p.len(), PLATFORM_FILES.len());
    assert!(p["openai"]["homepage"].is_string());
    assert_eq!(doc["last_updated"], 1234);
}

/// 票 13-D：DB 同步下来的端点覆盖编译期内置那份，`endpoints_locked` 保存不再把
/// 用户端点重置回二进制里的旧 `base_url`。
#[test]
fn endpoints_follow_db_synced_preset() {
    let bundled = endpoints_in(presets(), "anthropic");
    assert!(!bundled.is_empty());
    assert_ne!(bundled[0].base_url, "https://db.example/v1");

    let patched = r#"{"endpoints":{"default":[{"protocol":"anthropic","base_url":"https://db.example/v1"}]}}"#;
    let merged = merge_presets_doc([("anthropic", patched)], Some(1));
    let got = endpoints_in(&merged, "anthropic");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].base_url, "https://db.example/v1");
    // 未被 DB 覆盖的协议照旧走 bundled 端点
    let urls = |d: &Value| endpoints_in(d, "openai").iter().map(|e| e.base_url.clone()).collect::<Vec<_>>();
    assert_eq!(urls(&merged), urls(presets()));
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
