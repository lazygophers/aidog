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

/// 所有登记平台必带 platform.json；`pricing_only` 只允许纯协议豁免（现应为空）。
/// 2026-08-31 用户决策：禁止非纯协议豁免——litellm / mistral 已升级正式平台。
#[test]
fn every_platform_entry_carries_platform_file() {
    let idx = bundled_index();
    let po = idx.iter().filter(|e| e.platform_file.is_none()).collect::<Vec<_>>();
    assert!(po.is_empty(), "非纯协议平台不得豁免 platform.json: {:?}", po.iter().map(|e| e.code.clone()).collect::<Vec<_>>());
    for code in ["litellm", "meta", "mistral", "xai"] {
        let e = idx.iter().find(|e| e.code == code).expect(code);
        assert!(e.platform_file.is_some(), "{code} 必须有 platform.json");
        assert!(!e.models.is_empty(), "{code} 模型清单不可为空");
    }
    assert!(idx.iter().find(|e| e.code == "anthropic").expect("anthropic").platform_file.is_some());
}

/// DB 同步后的 presets 文档与 bundled 同形状；单份脏 JSON 只丢该协议，不炸整篇。
#[test]
fn presets_doc_skips_unparsable_entry() {
    let doc = presets_doc(
        [("good", r#"{"name":{"en-US":"Good"}}"#), ("bad", "{oops")],
        Value::from(7),
    );
    assert_eq!(doc["version"], Value::Null);
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
        "meta",
        "minimax",
        "mistralai",
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

/// 票 13-C：更新的 DB 行覆盖 bundled 同 code，bundled 里 DB 缺的补齐。
#[test]
fn merge_presets_doc_unions_db_over_bundled() {
    // DB 一行都没有 → 与 bundled 逐字节相同
    assert_eq!(merge_presets_doc([], None).to_string(), presets_json());

    let newer = r#"{"last_updated":9999999999,"homepage":"https://from-db"}"#;
    let doc = merge_presets_doc([("anthropic", newer)], Some(9_999_999_999));
    let p = doc["protocols"].as_object().expect("protocols");
    // 覆盖：DB 那条整份替换
    assert_eq!(p["anthropic"]["homepage"], "https://from-db");
    // 补齐：DB 没有的协议照样在
    assert_eq!(p.len(), PLATFORM_FILES.len());
    assert!(p["openai"]["homepage"].is_string());
    assert_eq!(doc["last_updated"], 9_999_999_999i64);
}

/// 同步源（上游仓库）比二进制里的 bundled 旧时，DB 行不得覆盖——否则二进制新增的字段
/// （如 quota_scripts / plan_quotas）被旧行整篇盖掉，界面上表现为该能力凭空消失。
#[test]
fn merge_presets_doc_keeps_newer_bundled_over_stale_db_row() {
    let bundled_stamp = presets()["protocols"]["glm_coding"]["last_updated"]
        .as_i64()
        .expect("bundled glm_coding 须带 last_updated");
    let stale = format!(r#"{{"last_updated":{},"homepage":"https://stale"}}"#, bundled_stamp - 1);
    let doc = merge_presets_doc([("glm_coding", stale.as_str())], Some(bundled_stamp - 1));
    // 旧行被忽略：bundled 的 quota_scripts 仍在，homepage 不是旧行那个
    assert_ne!(doc["protocols"]["glm_coding"]["homepage"], "https://stale");
    assert!(doc["protocols"]["glm_coding"]["quota_scripts"].is_array());
    // 文档级 last_updated 取 bundled 与 DB 的较大值
    assert!(doc["last_updated"].as_i64().unwrap() >= bundled_stamp - 1);
    // 无 last_updated 的旧行（戳缺失按 0 处理）同样不覆盖
    let doc2 = merge_presets_doc([("glm_coding", r#"{"homepage":"https://nostamp"}"#)], Some(1));
    assert_ne!(doc2["protocols"]["glm_coding"]["homepage"], "https://nostamp");
}

/// 票 13-D：DB 同步下来的端点覆盖编译期内置那份，`endpoints_locked` 保存不再把
/// 用户端点重置回二进制里的旧 `base_url`。
#[test]
fn endpoints_follow_db_synced_preset() {
    let bundled = endpoints_in(presets(), "anthropic");
    assert!(!bundled.is_empty());
    assert_ne!(bundled[0].base_url, "https://db.example/v1");

    let patched = r#"{"last_updated":9999999999,"endpoints":{"default":[{"protocol":"anthropic","base_url":"https://db.example/v1"}]}}"#;
    let merged = merge_presets_doc([("anthropic", patched)], Some(9_999_999_999));
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
    for code in ["anthropic", "openai", "gemini", "deepseek", "glm_coding", "glm_coding_en"] {
        assert!(!default_endpoints(code).is_empty(), "{code} 默认端点不可为空");
    }
}

/// glm_coding 是唯一带 peak 与 models.peak 的协议（路由/计费硬依赖）。
#[test]
fn glm_coding_keeps_peak_branches() {
    let glm = &protocols()["glm_coding"];
    assert!(glm["peak"].as_array().is_some_and(|a| !a.is_empty()));
    assert_eq!(glm["models"]["peak"]["default"], "glm-5.3-flash");
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

/// T2 加载测试：bundled 全部 platform.json 的 `quota_scripts` 可解析（当前 0 条，
/// 空集须过；T3 起逐族写入后本测试自动兜住可解析性与基本不变量）。
#[test]
fn bundled_quota_scripts_parse() {
    for (code, json) in bundled_platform_files() {
        for v in parse_quota_scripts(json) {
            assert!(!v.id.is_empty(), "{code} quota_scripts 有空 id");
            assert!(!v.script.trim().is_empty(), "{code}/{} script 为空", v.id);
        }
    }
}

/// T2：quota_scripts 解析（字段正确性 + requires/returns 缺省补全 + 脏输入返空）。
#[test]
fn quota_scripts_parse_fields_and_defaults() {
    let locale = serde_json::json!({
        "en-US": "Official", "zh-Hans": "官方", "ar-SA": "رسمي", "fr-FR": "Officiel",
        "de-DE": "Offiziell", "ru-RU": "Официальный", "ja-JP": "公式", "es-ES": "Oficial",
    });
    let doc = serde_json::json!({
        "last_updated": 1,
        "quota_scripts": [
            { "id": "official", "name": locale, "script": "return { success: true }" },
            { "id": "fork", "name": locale,
              "requires": [{ "key": "balance_base_url", "label": locale }],
              "returns": { "balance": true, "mcp": true, "tiers": ["five_hour", "mcp_monthly"] },
              "script": "return 1" },
        ],
    })
    .to_string();
    let vs = parse_quota_scripts(&doc);
    assert_eq!(vs.len(), 2);
    // requires / returns 缺省补全
    assert!(vs[0].requires.is_empty());
    assert_eq!(vs[0].returns, QuotaScriptReturns::default());
    assert_eq!(vs[0].script, "return { success: true }");
    // 声明字段逐项
    assert_eq!(vs[1].name["zh-Hans"], "官方");
    assert_eq!(vs[1].requires[0].key, "balance_base_url");
    assert_eq!(vs[1].requires[0].label["en-US"], "Official");
    assert!(vs[1].returns.balance && vs[1].returns.mcp && !vs[1].returns.coding_plan);
    assert_eq!(vs[1].returns.tiers, ["five_hour", "mcp_monthly"]);
    // 无字段 / 坏 JSON / 错类型 → 空 Vec（脏行不炸调用方）
    assert!(parse_quota_scripts(r#"{"last_updated":1}"#).is_empty());
    assert!(parse_quota_scripts("{oops").is_empty());
    assert!(parse_quota_scripts(r#"{"quota_scripts":"oops"}"#).is_empty());
}

/// T4：`quota_scripts_in`（presets 文档形状）+ 变体选中 / 脚本解析回落链。
#[test]
fn quota_scripts_in_and_selection() {
    // bundled 文档：glm 有脚本、kimi_en 无
    assert!(!quota_scripts_in(presets(), "glm").is_empty());
    assert!(quota_scripts_in(presets(), "kimi_en").is_empty());
    assert!(quota_scripts_in(presets(), "nonexistent").is_empty());

    // 选中：id 命中 / 缺省首条 / id 失效回落首条
    let vs = quota_scripts_in(presets(), "glm");
    assert_eq!(select_quota_variant(&vs, Some("default")).unwrap().id, "default");
    assert_eq!(select_quota_variant(&vs, None).unwrap().id, "default");
    assert_eq!(select_quota_variant(&vs, Some("renamed-by-remote")).unwrap().id, "default");
    assert!(select_quota_variant(&[], None).is_none());
}

/// review-fix：base_url 启发式分派数据驱动（platform.json 顶层 `quota_url_match`）。
/// 锁 bundled 文档的关键词 → 协议映射（同族共享关键词取文档序首个 = base 变体）。
#[test]
fn quota_code_for_base_url_matches_bundled_keywords() {
    assert_eq!(quota_code_for_base_url("https://api.kimi.com/coding/v1").as_deref(), Some("kimi"));
    assert_eq!(quota_code_for_base_url("https://open.bigmodel.cn/api/paas/v4").as_deref(), Some("glm"));
    assert_eq!(quota_code_for_base_url("https://api.z.ai/api/paas/v4").as_deref(), Some("glm"));
    assert_eq!(quota_code_for_base_url("https://api.minimaxi.com/v1").as_deref(), Some("minimax"));
    assert_eq!(quota_code_for_base_url("https://api.minimax.io/v1").as_deref(), Some("minimax_en"));
    assert_eq!(quota_code_for_base_url("https://api.deepseek.com").as_deref(), Some("deepseek"));
    assert_eq!(quota_code_for_base_url("https://api.stepfun.com/v1").as_deref(), Some("stepfun"));
    assert_eq!(quota_code_for_base_url("https://api.stepfun.ai/v1").as_deref(), Some("stepfun_en"));
    assert_eq!(quota_code_for_base_url("https://api.siliconflow.cn/v1").as_deref(), Some("siliconflow"));
    assert_eq!(quota_code_for_base_url("https://api.siliconflow.com/v1").as_deref(), Some("siliconflow_en"));
    assert_eq!(quota_code_for_base_url("https://openrouter.ai/api/v1").as_deref(), Some("openrouter"));
    assert_eq!(quota_code_for_base_url("https://api.novita.ai/v1").as_deref(), Some("novita"));
    // 无命中 / newapi（自部署中转，无专属域名关键词）→ None
    assert_eq!(quota_code_for_base_url("https://unknown.example.com/v1"), None);
    assert_eq!(quota_code_for_base_url("https://my-newapi.example.com/v1"), None);
}

#[test]
fn resolve_quota_script_fallback_chain() {
    let first = quota_scripts_in(presets(), "deepseek")[0].script.clone();
    // ① 物化列非空 → 直接用之（不被 extra 覆盖）
    assert_eq!(resolve_quota_script("deepseek", r#"{"quota_script_id":"x"}"#, "MATERIALIZED"), Some("MATERIALIZED".into()));
    // ② 无物化列：extra.quota_custom_script 优先于变体
    assert_eq!(
        resolve_quota_script("deepseek", r#"{"quota_custom_script":"return 1"}"#, ""),
        Some("return 1".into())
    );
    // ③ id 命中 / 缺省 / 失效 → 首条变体正文
    assert_eq!(resolve_quota_script("deepseek", r#"{"quota_script_id":"default"}"#, ""), Some(first.clone()));
    assert_eq!(resolve_quota_script("deepseek", "{}", ""), Some(first.clone()));
    assert_eq!(resolve_quota_script("deepseek", r#"{"quota_script_id":"gone"}"#, ""), Some(first));
    // 无脚本协议 → None（调用方维持 Unsupported err）
    assert_eq!(resolve_quota_script("kimi_en", "{}", ""), None);
    // extra 非 JSON → 不炸，走首条
    assert!(resolve_quota_script("deepseek", "oops", "").is_some());
}

#[test]
fn materialize_quota_script_rules() {
    let first = quota_scripts_in(presets(), "glm")[0].script.clone();
    // 全新（列空）→ 首条变体；无脚本协议 → 空串
    assert_eq!(materialize_quota_script("glm", "{}", "", true), first);
    assert_eq!(materialize_quota_script("kimi_en", "{}", "", true), "");
    // 自定义脚本优先（即使 id 也在）
    assert_eq!(
        materialize_quota_script("glm", r#"{"quota_script_id":"default","quota_custom_script":"return 9"}"#, "OLD", false),
        "return 9"
    );
    // id 有值 → 重写为选中变体（远程更新待拉入）
    assert_eq!(materialize_quota_script("glm", r#"{"quota_script_id":"default"}"#, "OLD", false), first);
    // id 失效 → 回落首条重写
    assert_eq!(materialize_quota_script("glm", r#"{"quota_script_id":"gone"}"#, "OLD", false), first);
    // 无 id + 列已有值 + 协议未变 → 保留（已物化脚本不随远程同步自动换）
    assert_eq!(materialize_quota_script("glm", "{}", "OLD", false), "OLD");
    // 协议变更（旧列是别的协议的脚本）→ 重物化
    assert_eq!(materialize_quota_script("glm", "{}", "OLD-OTHER-PROTO", true), first);
    // 协议变更为无脚本协议 → 清列
    assert_eq!(materialize_quota_script("openai", "{}", "OLD", true), "");
}
