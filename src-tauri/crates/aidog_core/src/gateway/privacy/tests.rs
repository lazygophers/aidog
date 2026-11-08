//! 票 01 验收测试：命中/不命中、确定性、往返、幂等、开关。

use super::*;

const API_KEY_VAL: &str = "sk-ant-api03-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BANK_CARD_VAL: &str = "4532015112830366";
const BAD_BANK_CARD_VAL: &str = "4532015112830367";
const ID_VAL: &str = "11010119900307803X";
const BAD_ID_VAL: &str = "110101199003078030";
const EMAIL_VAL: &str = "alice@example.com";
const PEM_VAL: &str = "-----BEGIN RSA PRIVATE KEY-----\nMIabc123def456\n-----END RSA PRIVATE KEY-----";
const BEARER_VAL: &str = "Bearer abcdef0123456789abcdef0123456789";

fn scrub_all(text: &str) -> (String, ScrubMap) {
    scrub(text, &ScrubOptions::all())
}

/// 票 03 守卫（对抗性形态）：检测短路开启后，特征形态各异的 6 类 PII 仍必须全部检出。
/// 覆盖大写 Bearer、sk_ 下划线 key、小写 x 结尾身份证、含 % + 多段域名邮箱；
/// 该测试在无短路的旧实现下也必须绿（回归基线），短路实现有漏检时变红。
#[test]
fn all_six_kinds_detected_with_adversarial_forms() {
    let text = format!(
        "auth {0} | key {1} | id {2} | mail {3} | card {4} | {5}",
        "BEARER abcdef0123456789abcdef0123456789",
        "sk_abcdefghijklmnopqrstuvwxyz",
        "11010119900307803x",
        "first.last%tag+pfx@mail.example.co.uk",
        BANK_CARD_VAL,
        PEM_VAL,
    );
    let (s, m) = scrub_all(&text);
    for ph in [
        "{{PII_API_KEY_",
        "{{PII_PRIVATE_KEY_",
        "{{PII_BEARER_TOKEN_",
        "{{PII_ID_NUMBER_",
        "{{PII_BANK_CARD_",
        "{{PII_EMAIL_",
    ] {
        assert!(s.contains(ph), "短路后漏检 {ph}: {s}");
    }
    assert_eq!(m.len(), 6, "map: {m:?}");
}

#[test]
fn api_key_hit_and_miss() {
    let (s, m) = scrub_all(&format!("key={API_KEY_VAL} ok"));
    assert!(s.contains("{{PII_API_KEY_"), "scrubbed: {s}");
    assert!(!s.contains(API_KEY_VAL));
    assert_eq!(m.len(), 1);
    let (s, m) = scrub_all("no keys here, plain sk- short too");
    assert!(m.is_empty());
    assert_eq!(s, "no keys here, plain sk- short too");
}

#[test]
fn pem_hit_and_public_key_miss() {
    let (s, m) = scrub_all(PEM_VAL);
    assert!(s.starts_with("{{PII_PRIVATE_KEY_"), "scrubbed: {s}");
    assert_eq!(m.len(), 1);
    let (s, m) = scrub_all("-----BEGIN PUBLIC KEY-----\nMIabc\n-----END PUBLIC KEY-----");
    assert!(m.is_empty());
    assert_eq!(s, "-----BEGIN PUBLIC KEY-----\nMIabc\n-----END PUBLIC KEY-----");
}

#[test]
fn bearer_hit_and_miss() {
    let (s, m) = scrub_all(&format!("Authorization: {BEARER_VAL}"));
    assert!(s.contains("{{PII_BEARER_TOKEN_"), "scrubbed: {s}");
    assert_eq!(m.len(), 1);
    let (_, m) = scrub_all("bearer short");
    assert!(m.is_empty());
}

#[test]
fn email_hit_and_miss() {
    let (s, m) = scrub_all(&format!("mail {EMAIL_VAL} end"));
    assert!(s.contains("{{PII_EMAIL_"), "scrubbed: {s}");
    assert!(!s.contains(EMAIL_VAL));
    assert_eq!(m.len(), 1);
    let (_, m) = scrub_all("not-an-email no-at-sign");
    assert!(m.is_empty());
}

#[test]
fn bank_card_luhn_hit_bad_luhn_miss() {
    let (s, m) = scrub_all(&format!("card {BANK_CARD_VAL}"));
    assert!(s.contains("{{PII_BANK_CARD_"), "scrubbed: {s}");
    assert_eq!(m.len(), 1);
    let (s, m) = scrub_all(&format!("card {BAD_BANK_CARD_VAL}"));
    assert!(!s.contains("{{PII_BANK_CARD_"), "bad luhn must not match: {s}");
    assert!(m.is_empty());
}

#[test]
fn id_number_hit_and_bad_checksum_miss() {
    let (s, m) = scrub_all(&format!("id {ID_VAL}"));
    assert!(s.contains("{{PII_ID_NUMBER_"), "scrubbed: {s}");
    assert_eq!(m.len(), 1);
    let (s, m) = scrub_all(&format!("id {BAD_ID_VAL}"));
    assert!(m.is_empty(), "bad checksum must not match");
    assert_eq!(s, format!("id {BAD_ID_VAL}"));
}

#[test]
fn deterministic_placeholder_for_same_value() {
    let (s1, _) = scrub_all(&format!("key={API_KEY_VAL}"));
    let (s2, _) = scrub_all(&format!("key={API_KEY_VAL}"));
    assert_eq!(s1, s2);
}

#[test]
fn restore_roundtrip() {
    let text = format!("mail {EMAIL_VAL} card {BANK_CARD_VAL} id {ID_VAL}");
    let (scrubbed, map) = scrub_all(&text);
    assert_eq!(restore(&scrubbed, &map), text);
}

#[test]
fn scrub_is_idempotent_on_placeholders() {
    let text = format!("mail {EMAIL_VAL} card {BANK_CARD_VAL}");
    let (once, _) = scrub_all(&text);
    let (twice, m2) = scrub_all(&once);
    assert_eq!(once, twice);
    assert!(m2.is_empty());
}

#[test]
fn secrets_tier_off_skips_api_key() {
    let opts = ScrubOptions {
        secrets: false,
        personal: true,
        per_kind: HashMap::new(),
    };
    let (s, m) = scrub(&format!("key={API_KEY_VAL}"), &opts);
    assert_eq!(s, format!("key={API_KEY_VAL}"));
    assert!(m.is_empty());
}

#[test]
fn personal_tier_off_skips_email() {
    let opts = ScrubOptions {
        secrets: true,
        personal: false,
        per_kind: HashMap::new(),
    };
    let (_, m) = scrub(&format!("mail {EMAIL_VAL}"), &opts);
    assert!(m.is_empty());
}

#[test]
fn per_kind_override_wins_over_tier() {
    let opts = ScrubOptions {
        secrets: false,
        personal: true,
        per_kind: [(PiiKind::ApiKey, true)].into_iter().collect(),
    };
    let (s, m) = scrub(&format!("key={API_KEY_VAL}"), &opts);
    assert!(s.contains("{{PII_API_KEY_"));
    assert_eq!(m.len(), 1);
    let opts_off = ScrubOptions {
        secrets: true,
        personal: true,
        per_kind: [(PiiKind::Email, false)].into_iter().collect(),
    };
    let (_, m) = scrub(&format!("mail {EMAIL_VAL}"), &opts_off);
    assert!(m.is_empty());
}

#[test]
fn scrub_map_debug_hides_original() {
    let text = format!("mail {EMAIL_VAL}");
    let (_, map) = scrub_all(&text);
    let dbg = format!("{map:?}");
    assert!(!dbg.contains(EMAIL_VAL), "Debug leaked original: {dbg}");
    assert!(dbg.contains("[REDACTED"));
}

#[test]
fn placeholder_survives_second_scrub_of_restored_text() {
    let text = format!("mail {EMAIL_VAL}");
    let (scrubbed, map) = scrub_all(&text);
    let restored = restore(&scrubbed, &map);
    assert_eq!(restored, text);
    let (again, map2) = scrub_all(&restored);
    assert_eq!(map2.len(), 1);
    assert_eq!(again, scrubbed);
}
