//! 校验位二次校验（票 09）。
//!
//! 内置识别器里「信用卡 / IBAN / 中国身份证」光靠正则误报极高（任意 16 位数字都像卡号）。
//! 这些叶子在 `ConditionLeaf::validator` 里指定校验器名，正则命中的片段必须再过一遍
//! 校验位算法才算命中；校验失败的片段既不触发条件、也不被 mask 替换。
//!
//! 未知校验器名 → `true`（fail-open，退化为纯正则），避免拼错名字让规则静默全失效。

/// 按名字跑校验位校验。`s` 是正则命中的原始片段（可含分隔符）。
pub(crate) fn validate(name: &str, s: &str) -> bool {
    match name {
        "" => true,
        "luhn" => luhn(s),
        "iban" => iban(s),
        "cn_id" => cn_id(s),
        _ => true,
    }
}

/// Luhn 校验（信用卡 / 银行卡）。忽略空格与连字符；长度 12..=19 位数字。
fn luhn(s: &str) -> bool {
    let digits: Vec<u32> = s
        .chars()
        .filter(|c| !matches!(c, ' ' | '-'))
        .map(|c| c.to_digit(10))
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default();
    if !(12..=19).contains(&digits.len()) {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let x = d * 2;
                if x > 9 { x - 9 } else { x }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

/// IBAN 校验（ISO 13616 mod-97-10）。忽略空格；前 4 位移到末尾，字母换成 10+序号，模 97 == 1。
fn iban(s: &str) -> bool {
    let compact: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if !(15..=34).contains(&compact.len()) || !compact.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    let b = compact.as_bytes();
    if !b[0].is_ascii_alphabetic() || !b[1].is_ascii_alphabetic() {
        return false;
    }
    if !b[2].is_ascii_digit() || !b[3].is_ascii_digit() {
        return false;
    }
    // 逐字符取模，避免大整数依赖。
    let mut rem: u32 = 0;
    for c in compact[4..].chars().chain(compact[..4].chars()) {
        let v = if c.is_ascii_digit() {
            c as u32 - '0' as u32
        } else {
            c as u32 - 'A' as u32 + 10
        };
        rem = if v >= 10 {
            (rem * 100 + v) % 97
        } else {
            (rem * 10 + v) % 97
        };
    }
    rem == 1
}

/// 中国大陆二代身份证校验（GB 11643-1999 ISO 7064:1983 MOD 11-2）。18 位，末位可为 X。
fn cn_id(s: &str) -> bool {
    let b: Vec<char> = s.chars().collect();
    if b.len() != 18 {
        return false;
    }
    const W: [u32; 17] = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
    const CHECK: [char; 11] = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];
    let mut sum = 0u32;
    for i in 0..17 {
        match b[i].to_digit(10) {
            Some(d) => sum += d * W[i],
            None => return false,
        }
    }
    let last = b[17].to_ascii_uppercase();
    CHECK[(sum % 11) as usize] == last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luhn_accepts_valid_and_rejects_typo() {
        // 通用测试卡号（Visa / MasterCard 测试段），Luhn 通过。
        assert!(luhn("4111111111111111"));
        assert!(luhn("4111 1111 1111 1111"));
        assert!(luhn("5500-0000-0000-0004"));
        // 改一位 → 正则照样是 16 位数字，Luhn 挂。
        assert!(!luhn("4111111111111112"));
        assert!(!luhn("1234567812345678"));
        // 长度越界。
        assert!(!luhn("41111111111"));
    }

    #[test]
    fn iban_accepts_valid_and_rejects_typo() {
        assert!(iban("GB82WEST12345698765432"));
        assert!(iban("DE89 3704 0044 0532 0130 00"));
        assert!(!iban("GB82WEST12345698765433"));
        assert!(!iban("XX00NOTANIBANATALL12"));
    }

    #[test]
    fn cn_id_accepts_valid_and_rejects_typo() {
        // 构造样本：前 17 位 + 按 MOD 11-2 算出的正确校验位。
        assert!(cn_id("11010519491231002X"));
        assert!(!cn_id("110105194912310021"));
        assert!(!cn_id("11010519491231002"));
    }

    #[test]
    fn unknown_validator_is_fail_open() {
        assert!(validate("", "anything"));
        assert!(validate("no-such-validator", "anything"));
    }
}
