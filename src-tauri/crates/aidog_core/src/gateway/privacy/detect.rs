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

pub(crate) static RULES: [Rule; 6] = [
    Rule {
        kind: PiiKind::ApiKey,
        re: &API_KEY_RE,
        validate: always_valid,
    },
    Rule {
        kind: PiiKind::PrivateKey,
        re: &PEM_RE,
        validate: always_valid,
    },
    Rule {
        kind: PiiKind::BearerToken,
        re: &BEARER_RE,
        validate: always_valid,
    },
    Rule {
        kind: PiiKind::IdNumber,
        re: &ID_RE,
        validate: is_valid_id_number,
    },
    Rule {
        kind: PiiKind::BankCard,
        re: &BANK_CARD_RE,
        validate: luhn_valid,
    },
    Rule {
        kind: PiiKind::Email,
        re: &EMAIL_RE,
        validate: always_valid,
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
