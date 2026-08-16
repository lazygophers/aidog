//! env/header 敏感值脱敏与脱敏 merge 还原。

use std::collections::BTreeMap;

/// 判定 env/header key 是否敏感（含 token/key/secret/auth/password/pass/credential）。
fn is_sensitive_key(k: &str) -> bool {
    let lk = k.to_ascii_lowercase();
    ["token", "key", "secret", "auth", "password", "pass", "credential"]
        .iter()
        .any(|s| lk.contains(s))
}

/// 脱敏 map：敏感 key 的值替换为 "***"。
pub fn mask_env(map: BTreeMap<String, String>) -> BTreeMap<String, String> {
    map.into_iter()
        .map(|(k, v)| {
            if is_sensitive_key(&k) {
                (k, "***".to_string())
            } else {
                (k, v)
            }
        })
        .collect()
}

/// 脱敏 merge：incoming 中值为 "***" 的 key → 取 old 明文；其余用新值。
/// 前端编辑表单初始用脱敏值，用户未改的字段提交 "***"，此处还原。
pub(super) fn merge_masked(
    incoming: BTreeMap<String, String>,
    old: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    incoming
        .into_iter()
        .map(|(k, v)| {
            if v.as_str() == "***" {
                (k.clone(), old.get(&k).cloned().unwrap_or(v))
            } else {
                (k, v)
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "test_mask.rs"]
mod test_mask;
