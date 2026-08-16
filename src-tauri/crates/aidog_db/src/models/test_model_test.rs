//! model_test.rs 单测（原 models.rs `model_test_challenge_tests`）。

use super::*;

#[test]
fn random_challenge_prompt_varies_and_expected_nonempty() {
    // 多次生成应出现不止一个 prompt（防指纹）；每条 expected 非空且可在自身校验通过。
    let mut prompts = std::collections::HashSet::new();
    for _ in 0..200 {
        let (p, e) = random_test_challenge();
        assert!(!p.trim().is_empty(), "prompt 不应为空");
        assert!(!e.trim().is_empty(), "expected 不应为空");
        // expected 直接喂回校验必然通过（归一化自反）。
        assert!(verify_test_response(&e, Some(&e)), "expected 自校验应通过: {e}");
        prompts.insert(p);
    }
    assert!(prompts.len() > 1, "200 次生成应产生多种 prompt，实际 {}", prompts.len());
}

#[test]
fn arithmetic_answers_are_correct() {
    // 算术题答案须为真实计算结果：采样直到覆盖一道加法验证语义。
    for _ in 0..500 {
        let (p, e) = random_test_challenge();
        if let Some(idx) = p.find(" 加 ") {
            let a: i64 = p[..idx].trim().parse().unwrap();
            let rest = &p[idx + " 加 ".len()..];
            let b: i64 = rest[..rest.find(' ').unwrap()].trim().parse().unwrap();
            assert_eq!(e, (a + b).to_string());
            return;
        }
    }
    panic!("500 次未抽到加法题");
}

#[test]
fn verify_substring_match_tolerates_natural_answers() {
    // 含子串即通过：模型自然长答 + 标点 + 大小写均应匹配。
    assert!(verify_test_response("答案是 95。", Some("95")));
    assert!(verify_test_response("中国的首都是北京，是一座历史名城。", Some("北京")));
    assert!(verify_test_response("The formula is H2O.", Some("H2O")));
    assert!(verify_test_response("h2o", Some("H2O"))); // 大小写归一
    // 不含 expected → 失败。
    assert!(!verify_test_response("上海", Some("北京")));
    assert!(!verify_test_response("", Some("12")));
}

#[test]
fn verify_custom_mode_skips_content_check() {
    // expected=None（自定义 prompt）：非空即通过，空白即失败，不做关键词比对。
    assert!(verify_test_response("任意非空回答", None));
    assert!(verify_test_response("anything goes here", None));
    assert!(!verify_test_response("   ", None));
    assert!(!verify_test_response("", None));
}
