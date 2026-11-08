//! 敏感数据规则表：每条规则 = 类型 + 正则 + 额外校验函数。
//! 规则表数据驱动（const 数组）；先命中的规则占用区间，后规则不触碰重叠区。
//! IdNumber 先于 BankCard：18 位且校验位通过时按身份证处理。

use std::sync::LazyLock;

use super::PiiKind;
use regex::Regex;

pub(crate) struct Rule {
    pub kind: PiiKind,
    pub re: &'static LazyLock<Regex>,
    pub validate: fn(&str) -> bool,
    /// 廉价特征预判：返回 false ⇒ 正则必不命中（必含字面特征缺失，严格推理），
    /// 跳过该正则。宁可误命中（true 时照常跑正则）不漏检。
    pub precheck: fn(&str) -> bool,
}

static API_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(sk-ant-|sk-|sk_|tp-|ark-)[A-Za-z0-9_-]{16,}").unwrap()
});

static PEM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----")
        .unwrap()
});

static BEARER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{16,}").unwrap()
});

static ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d{17}[\dXx]\b").unwrap());

static BANK_CARD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d{13,19}\b").unwrap());

static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap()
});

/// API key 必含字面前缀（正则交替分支 sk-ant- / sk- / sk_ / tp- / ark- 的字面部分）。
/// sk-ant- 被 sk- 覆盖（contains("sk-") 即涵盖）。
const API_KEY_PREFIXES: [&str; 4] = ["sk-", "sk_", "tp-", "ark-"];

fn possible_api_key(s: &str) -> bool {
    API_KEY_PREFIXES.iter().any(|p| s.contains(p))
}

fn possible_private_key(s: &str) -> bool {
    // 匹配必含字面 "PRIVATE KEY"（BEGIN/END 头含它）。
    s.contains("PRIVATE KEY")
}

fn possible_bearer(s: &str) -> bool {
    // (?i) 走 Unicode 折叠，非 ASCII 字节在场时保守不预判（宁误命中不漏）。
    if !s.is_ascii() {
        return true;
    }
    s.as_bytes()
        .windows(6)
        .any(|w| w.eq_ignore_ascii_case(b"bearer"))
}

/// 是否可能存在 ≥ n 个连续「\d 兼容」字符。非 ASCII 字节按潜在 Unicode 数字计
/// （\d 在 regex crate 是 Unicode Nd，非 ASCII 字节按 1 计只会多不会少 ⇒ 不漏）。
/// Xx 计入是为 ID 的末位校验字符；对 BANK 只会多计（误命中方向，安全）。
fn digitish_run_possible(s: &str, n: usize) -> bool {
    let mut run = 0usize;
    for &b in s.as_bytes() {
        if b.is_ascii_digit() || b == b'X' || b == b'x' || b >= 0x80 {
            run += 1;
            if run >= n {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

fn possible_id_number(s: &str) -> bool {
    digitish_run_possible(s, 18)
}

fn possible_bank_card(s: &str) -> bool {
    digitish_run_possible(s, 13)
}

fn possible_email(s: &str) -> bool {
    // 匹配必含 '@'。
    s.contains('@')
}

pub(crate) static RULES: [Rule; 6] = [
    Rule {
        kind: PiiKind::ApiKey,
        re: &API_KEY_RE,
        validate: always_valid,
        precheck: possible_api_key,
    },
    Rule {
        kind: PiiKind::PrivateKey,
        re: &PEM_RE,
        validate: always_valid,
        precheck: possible_private_key,
    },
    Rule {
        kind: PiiKind::BearerToken,
        re: &BEARER_RE,
        validate: always_valid,
        precheck: possible_bearer,
    },
    Rule {
        kind: PiiKind::IdNumber,
        re: &ID_RE,
        validate: is_valid_id_number,
        precheck: possible_id_number,
    },
    Rule {
        kind: PiiKind::BankCard,
        re: &BANK_CARD_RE,
        validate: luhn_valid,
        precheck: possible_bank_card,
    },
    Rule {
        kind: PiiKind::Email,
        re: &EMAIL_RE,
        validate: always_valid,
        precheck: possible_email,
    },
];

fn always_valid(_s: &str) -> bool {
    true
}

/// Luhn 校验（银行卡号）。
pub(crate) fn luhn_valid(s: &str) -> bool {
    let digits: Vec<u32> = s.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() != s.chars().count() || digits.is_empty() {
        return false;
    }
    let mut sum = 0u32;
    for (i, d) in digits.iter().rev().enumerate() {
        if i % 2 == 1 {
            let dd = d * 2;
            sum += if dd > 9 { dd - 9 } else { dd };
        } else {
            sum += d;
        }
    }
    sum.is_multiple_of(10)
}

/// 身份证 18 位校验位（GB 11643-1999）。
pub(crate) fn is_valid_id_number(s: &str) -> bool {
    if s.len() != 18 {
        return false;
    }
    let chars: Vec<char> = s.chars().collect();
    let weights = [7usize, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
    let map = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];
    let mut sum = 0usize;
    for (i, c) in chars.iter().take(17).enumerate() {
        let Some(d) = c.to_digit(10) else {
            return false;
        };
        sum += d as usize * weights[i];
    }
    match map[sum % 11] {
        expect if expect == chars[17] => true,
        'X' if chars[17] == 'x' => true,
        _ => false,
    }
}

/// 检测到的敏感片段（字节区间 + 类型）。
pub(crate) struct SpanKind {
    pub start: usize,
    pub end: usize,
    pub kind: PiiKind,
}

/// 按规则表顺序检测：区间先到先占，先前规则占用的区间不被后续规则触碰；
/// 开关关闭或校验不通过的类型跳过。
pub(crate) fn collect_spans(text: &str, enabled: impl Fn(PiiKind) -> bool) -> Vec<SpanKind> {
    let mut spans: Vec<SpanKind> = Vec::new();
    for rule in RULES.iter() {
        if !enabled(rule.kind) {
            continue;
        }
        if !(rule.precheck)(text) {
            // 必含字面特征缺失 ⇒ 正则必不命中，零开销跳过。
            continue;
        }
        for mat in rule.re.find_iter(text) {
            let (start, end) = (mat.start(), mat.end());
            if spans.iter().any(|s| start < s.end && s.start < end) {
                continue;
            }
            if !(rule.validate)(mat.as_str()) {
                continue;
            }
            spans.push(SpanKind { start, end, kind: rule.kind });
        }
    }
    spans.sort_by_key(|s| s.start);
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 短路生效证明：无特征文本必须被预判跳过（若预判恒 true 则此测试红）。
    #[test]
    fn prechecks_skip_featureless_text() {
        assert!(!possible_api_key("just a plain sentence here"));
        assert!(!possible_private_key("hello world"));
        assert!(!possible_bearer("plain ascii text"));
        assert!(!possible_id_number("no digits run, ok"));
        assert!(!possible_bank_card("1234"));
        assert!(!possible_email("no at sign here"));
    }

    /// 预判「宁可误命中不漏」：对每类 PII 夹具（含对抗形态）必须返回 true（不跳过）。
    #[test]
    fn prechecks_keep_every_class_fixture() {
        assert!(possible_api_key("key sk_abcdefghijklmnopqrstuvwxyz ok"));
        assert!(possible_api_key("key sk-ant-api03-abcdefghijklmnop ok"));
        assert!(possible_api_key("key tp-abcdefghijklmnop ok"));
        assert!(possible_api_key("key ark-abcdefghijklmnop ok"));
        assert!(possible_private_key("-----BEGIN RSA PRIVATE KEY----- x"));
        assert!(possible_bearer("BEARER abcdef0123456789abcdef0123456789"));
        assert!(possible_bearer("Bearer abcdef0123456789abcdef0123456789"));
        // 非 ASCII 字节在场时 bearer 保守不预判（Unicode 折叠可能性）。
        assert!(possible_bearer("bearer\u{4e2d}token"));
        assert!(possible_id_number("11010119900307803X"));
        assert!(possible_id_number("11010119900307803x"));
        assert!(possible_bank_card("4532015112830366"));
        assert!(possible_email("alice@example.com"));
        assert!(possible_email("first.last%tag+pfx@mail.example.co.uk"));
    }

    /// 短路不改变检出结果：整条检测链对 6 类夹具仍产出 6 个 span。
    #[test]
    fn collect_spans_detects_all_six_kinds() {
        let text = "auth BEARER abcdef0123456789abcdef0123456789 | key sk_abcdefghijklmnopqrstuvwxyz | \
                    id 11010119900307803x | mail a@b.co | card 4532015112830366 | \
                    -----BEGIN RSA PRIVATE KEY-----\nabc\n-----END RSA PRIVATE KEY-----";
        let spans = collect_spans(text, |_| true);
        let kinds: Vec<_> = spans.iter().map(|s| s.kind).collect();
        for k in [
            PiiKind::ApiKey,
            PiiKind::PrivateKey,
            PiiKind::BearerToken,
            PiiKind::IdNumber,
            PiiKind::BankCard,
            PiiKind::Email,
        ] {
            assert!(kinds.contains(&k), "漏检 {k:?}");
        }
        assert_eq!(spans.len(), 6);
    }
}
