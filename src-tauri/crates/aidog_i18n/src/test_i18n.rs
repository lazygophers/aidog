use super::*;

const ALL_KEYS: [ErrorKey; 19] = [
    ErrorKey::ReadBody,
    ErrorKey::NoMatchingGroup,
    ErrorKey::ParseJson,
    ErrorKey::ParseRequest,
    ErrorKey::Route,
    ErrorKey::BudgetExhausted,
    ErrorKey::BudgetResetDaily,
    ErrorKey::BudgetResetRolling,
    ErrorKey::BudgetResetFixed,
    ErrorKey::BudgetResetTotal,
    ErrorKey::Upstream,
    ErrorKey::MiddlewareBlocked,
    ErrorKey::ProxyStartPortInUse,
    ErrorKey::ProxyStartOther,
    ErrorKey::TrayStillRunning,
    ErrorKey::UpdateAvailable,
    ErrorKey::QuotaExhaustedCodingPlan,
    ErrorKey::QuotaExhaustedCodingPlanResume,
    ErrorKey::BalanceInsufficient,
];

const ALL_LANGS: [Lang; 8] = [
    Lang::ZhCn,
    Lang::EnUs,
    Lang::JaJp,
    Lang::FrFr,
    Lang::DeDe,
    Lang::RuRu,
    Lang::ArSa,
    Lang::EsEs,
];

#[test]
fn from_locale_all_variants() {
    assert_eq!(Lang::from_locale("zh-CN"), Lang::ZhCn);
    assert_eq!(Lang::from_locale("zh_cn"), Lang::ZhCn);
    assert_eq!(Lang::from_locale("zh-Hans"), Lang::ZhCn);
    assert_eq!(Lang::from_locale("zh_hans"), Lang::ZhCn);
    assert_eq!(Lang::from_locale(" zh "), Lang::ZhCn);
    assert_eq!(Lang::from_locale("ja"), Lang::JaJp);
    assert_eq!(Lang::from_locale("fr-FR"), Lang::FrFr);
    assert_eq!(Lang::from_locale("de"), Lang::DeDe);
    assert_eq!(Lang::from_locale("ru"), Lang::RuRu);
    assert_eq!(Lang::from_locale("ar"), Lang::ArSa);
    assert_eq!(Lang::from_locale("es-ES"), Lang::EsEs);
    assert_eq!(Lang::from_locale("es_es"), Lang::EsEs);
    assert_eq!(Lang::from_locale("es"), Lang::EsEs);
    assert_eq!(Lang::from_locale("en-US"), Lang::EnUs);
    assert_eq!(Lang::from_locale("xx"), Lang::EnUs); // fallback
    assert_eq!(Lang::default(), Lang::EnUs);
}

#[test]
fn t_covers_all_langs_and_keys() {
    for lang in ALL_LANGS {
        for key in ALL_KEYS {
            let s = t(lang, key);
            assert!(!s.is_empty(), "empty translation for {lang:?}/{key:?}");
        }
    }
}

/// 模板里 `{...}` 占位（含 `{ · |session_name}` 可选段、`{{n}}`）的集合。
fn placeholders(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find('{') {
        let Some(j) = rest[i..].find('}') else { break };
        let mut end = i + j + 1;
        if rest[end..].starts_with('}') {
            end += 1; // `{{n}}`
        }
        out.push(rest[i..end].to_string());
        rest = &rest[end..];
    }
    out.sort();
    out
}

#[test]
fn backend_shared_keys_keep_placeholders_in_all_langs() {
    let zh = locale_table(Lang::ZhCn);
    let keys: Vec<&String> = zh
        .keys()
        .filter(|k| k.starts_with("notif.eventTemplate.") || k.starts_with("tray.render.countdown") || *k == "tray.render.clockDays")
        .collect();
    assert_eq!(keys.len(), 33 + 4, "zh-Hans 后端共用 key 数");
    let mut bad = Vec::new();
    for lang in ALL_LANGS {
        for k in &keys {
            match locale_table(lang).get(k.as_str()) {
                Some(v) if placeholders(v) == placeholders(&zh[k.as_str()]) => {}
                other => bad.push(format!("{lang:?} {k}: {other:?}")),
            }
        }
    }
    assert!(bad.is_empty(), "占位符与 zh-Hans 不一致:\n{}", bad.join("\n"));
}

#[test]
fn lookup_falls_back_to_en_then_none() {
    assert_eq!(lookup(Lang::ZhCn, "tray.render.peakOn"), Some("峰"));
    assert_eq!(lookup(Lang::EnUs, "tray.render.peakOn"), Some("Peak"));
    assert_eq!(lookup(Lang::JaJp, "no.such.key"), None);
    assert_eq!(tr(Lang::JaJp, "no.such.key"), "no.such.key");
}
