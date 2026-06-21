use super::*;
use std::collections::BTreeMap;

#[test]
fn mask_env_sensitive_keys() {
    let mut m = BTreeMap::new();
    m.insert("API_KEY".into(), "sk-secret".into());
    m.insert("AUTH_TOKEN".into(), "tok".into());
    m.insert("PASSWORD".into(), "p".into());
    m.insert("DEBUG".into(), "1".into());
    m.insert("CREDS".into(), "c".into()); // 含 credential? no → 'creds' 不含
    let masked = mask_env(m);
    assert_eq!(masked.get("API_KEY").unwrap(), "***");
    assert_eq!(masked.get("AUTH_TOKEN").unwrap(), "***");
    assert_eq!(masked.get("PASSWORD").unwrap(), "***");
    assert_eq!(masked.get("DEBUG").unwrap(), "1"); // 非敏感保留
    // 'creds' 不含敏感词根 → 保留（credential 子串匹配，creds 无 'credential'）
    assert_eq!(masked.get("CREDS").unwrap(), "c");
}

#[test]
fn merge_masked_keeps_old_secret_for_placeholder() {
    let mut old = BTreeMap::new();
    old.insert("API_KEY".into(), "sk-real".into());
    old.insert("DEBUG".into(), "0".into());
    // 前端未改 API_KEY（*** 占位），改 DEBUG，加 NEW_VAR，删（不传）无
    let mut incoming = BTreeMap::new();
    incoming.insert("API_KEY".into(), "***".into());
    incoming.insert("DEBUG".into(), "1".into());
    incoming.insert("NEW_VAR".into(), "x".into());
    let merged = merge_masked(incoming, &old);
    assert_eq!(merged.get("API_KEY").unwrap(), "sk-real"); // *** → 旧明文
    assert_eq!(merged.get("DEBUG").unwrap(), "1"); // 新值
    assert_eq!(merged.get("NEW_VAR").unwrap(), "x"); // 新 key
}

#[test]
fn merge_masked_placeholder_without_old_falls_back() {
    // *** 但旧 DB 无该 key → 保留 ***（不应发生，但兜底不 panic）
    let old = BTreeMap::new();
    let mut incoming = BTreeMap::new();
    incoming.insert("X".into(), "***".into());
    let merged = merge_masked(incoming, &old);
    assert_eq!(merged.get("X").unwrap(), "***");
}
