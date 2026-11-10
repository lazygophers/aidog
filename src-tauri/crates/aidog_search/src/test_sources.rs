//! 搜索来源清单自测：key 服务 + 免 key 引擎的单一真值源。

use super::*;

#[test]
fn sources_are_8_key_services_then_all_engines() {
    let v = search_sources();
    let ids: Vec<&str> = v.iter().map(|s| s.id.as_str()).collect();
    let keyed = ["tavily", "brave", "exa", "firecrawl", "youcom", "deepseek", "doubao", "tencent_yuanbao"];
    assert_eq!(KEY_SERVICES, keyed);
    let mut want: Vec<&str> = keyed.to_vec();
    want.extend(engines::ENGINES.iter().map(|e| e.id));
    assert_eq!(ids, want);
}

#[test]
fn source_fields_match_kind() {
    for s in search_sources() {
        if s.kind == "keyed" {
            assert!(s.needs_key && !s.default_on && s.billing == "per_request", "{}", s.id);
        } else {
            assert_eq!(s.kind, "engine", "{}", s.id);
            let e = engines::ENGINES.iter().find(|e| e.id == s.id).unwrap();
            assert!(!s.needs_key && s.billing == "free" && s.default_on == e.default_on, "{}", s.id);
        }
    }
}
