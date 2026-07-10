//! 模型测试模型：请求/结果 + 随机可校验题库与响应校验逻辑。

use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "test_model_test.rs"]
mod test_model_test;

#[derive(Debug, Deserialize)]
pub struct ModelTestRequest {
    pub platform_id: u64,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTestResult {
    pub success: bool,
    pub model: String,
    pub prompt_preview: String,
    pub response_preview: String,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub error: String,
}

/// 内置常识问答题库（问题, 期望关键词）。
/// 关键词须极短且为模型自然回答的开头，以便在 max_tokens=16 截断下仍可校验。
#[allow(dead_code)]
pub const TEST_TRIVIA: &[(&str, &str)] = &[
    ("中国的首都是哪个城市？", "北京"),
    ("一年有几个月？", "12"),
    ("水的化学式是什么？", "H2O"),
    ("地球有几个卫星（月亮）？", "1"),
    ("一周有几天？", "7"),
    ("彩虹有几种颜色？", "7"),
    ("太阳从哪个方向升起？", "东"),
    ("一个三角形有几条边？", "3"),
    ("人类有几只手？", "2"),
    ("英文字母表有几个字母？", "26"),
];

/// 生成一道随机可校验的测试题，返回 `(prompt, expected)`。
/// 两类轮换：算术（随机两位数 +/-/×）与常识问答。
/// prompt 每次随机 → 防指纹；expected 为归一化后用于子串校验的极短答案。
#[allow(dead_code)]
pub fn random_test_challenge() -> (String, String) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    if rng.gen_bool(0.5) {
        // 算术：两位数 10..=99
        let a: i64 = rng.gen_range(10..=99);
        let b: i64 = rng.gen_range(10..=99);
        match rng.gen_range(0..3) {
            0 => (format!("{} 加 {} 等于多少？", a, b), (a + b).to_string()),
            1 => {
                // 保证非负，便于关键词在开头
                let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
                (format!("{} 减 {} 等于多少？", hi, lo), (hi - lo).to_string())
            }
            _ => (format!("{} 乘以 {} 等于多少？", a, b), (a * b).to_string()),
        }
    } else {
        let (q, ans) = TEST_TRIVIA[rng.gen_range(0..TEST_TRIVIA.len())];
        (q.to_string(), ans.to_string())
    }
}

/// 归一化文本用于子串校验：转小写 + 去除空白/标点（仅保留字母数字与 CJK）。
/// 同一规则同时作用于响应文本与 expected，避免 "H2O"/"h2o"、"北京。"/"北京" 等差异。
#[allow(dead_code)]
pub fn normalize_for_match(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// 测试响应内容校验（产线 model_test 复用，保证单测覆盖真实逻辑）。返回 true=通过。
///
/// - `expected = Some(exp)`（随机可校验题）：归一化后响应须含 expected 子串。
/// - `expected = None`（自定义 prompt）：跳过内容校验，仅要求响应非空。
#[allow(dead_code)]
pub fn verify_test_response(response_text: &str, expected: Option<&str>) -> bool {
    match expected {
        Some(exp) => {
            let norm_exp = normalize_for_match(exp);
            !norm_exp.is_empty() && normalize_for_match(response_text).contains(&norm_exp)
        }
        None => !response_text.trim().is_empty(),
    }
}
