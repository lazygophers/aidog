use super::*;

const ALL_KEYS: [ErrorKey; 12] = [
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
];

const ALL_LANGS: [Lang; 7] = [
    Lang::ZhCn,
    Lang::EnUs,
    Lang::JaJp,
    Lang::FrFr,
    Lang::DeDe,
    Lang::RuRu,
    Lang::ArSa,
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
