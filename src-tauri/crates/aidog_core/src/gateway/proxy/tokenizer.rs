//! count_tokens 本地估算的 per-model BPE 分词调度。
//!
//! 按模型族前缀选 encoding：OpenAI o 系列 / 4o / 4.1 / 4.5 / 5 → o200k_base；
//! GPT-4 / 3.5 / claude-* / deepseek / kimi / doubao / 未知 → cl100k_base（兜底）；
//! glm-* / 裸 glm → HF tokenizers 加载 bundled glm-4.json；qwen-* → qwen2.json。
//!
//! 失败链路（不 panic）：HF tokenizer 加载/编码失败 → cl100k。
//! tiktoken 单例内部 `.unwrap()` 仅在 bundled BPE 数据损坏时炸（不会发生），属可接受硬约束。
//!
//! ponytail: 缓存用 std::sync::OnceLock（同 `gateway/peak.rs` idiom），不引入 lazy_static。
//! HF Tokenizer 解析 19MB JSON 耗时百毫秒级，OnceLock 保证进程级一次解析。

use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;
use tokenizers::Tokenizer;

/// bundled HF tokenizer.json 的 zstd 压缩件：`aidog_core/build.rs` 构建期从
/// `assets/tokenizers/` 压缩写入 OUT_DIR，首次使用时解压（解压缓冲在解析后即释放）。
const GLM4_ZST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/glm-4.json.zst"));
const QWEN2_ZST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/qwen2.json.zst"));

fn decompress(zst: &[u8]) -> std::io::Result<Vec<u8>> {
    zstd::decode_all(zst)
}

/// 解压 + 解析 bundled tokenizer；任一步失败返回错误文本（调用方回落 cl100k）。
fn load(zst: &[u8]) -> Result<Tokenizer, String> {
    let json = decompress(zst).map_err(|e| e.to_string())?;
    Tokenizer::from_bytes(json).map_err(|e| e.to_string())
}

/// per-family encoding 选择。未识别模型族 → Cl100k（OpenAI 旧族 / Anthropic / 兜底）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Encoding {
    O200k,
    Cl100k,
    Glm4,
    Qwen2,
}

/// 模型 id → encoding。前缀匹配（大小写不敏感），未命中 → Cl100k 兜底。
fn pick_encoding(model: &str) -> Encoding {
    let m = model.to_ascii_lowercase();
    // OpenAI o200k 家族（顺序敏感：o* / 4o / 4.1 / 4.5 / 5 在 gpt-4 / 3.5 之前判）
    if m.starts_with("gpt-4o")
        || m.starts_with("gpt-4.1")
        || m.starts_with("gpt-4.5")
        || m.starts_with("gpt-5")
        || m.starts_with("o1")
        || m.starts_with("o3")
        || m.starts_with("o4-mini")
        || m.starts_with("chatgpt-4o")
    {
        return Encoding::O200k;
    }
    // GLM（含裸 "glm" / "glm-coding" / "glm-4.x" / "glm-5.x"）
    if m == "glm" || m.starts_with("glm-") || m.starts_with("glm_") {
        return Encoding::Glm4;
    }
    // Qwen
    if m.starts_with("qwen") {
        return Encoding::Qwen2;
    }
    // 其余（gpt-4 / gpt-3.5 / claude-* / deepseek / kimi / doubao / 未知）→ cl100k
    Encoding::Cl100k
}

/// tiktoken cl100k 单例（直接复用 tiktoken-rs 内部 lazy_static，零额外封装）。
fn cl100k() -> &'static CoreBPE {
    tiktoken_rs::cl100k_base_singleton()
}

/// tiktoken o200k 单例。
fn o200k() -> &'static CoreBPE {
    tiktoken_rs::o200k_base_singleton()
}

/// HF glm-4 tokenizer 缓存：None = 加载失败（不应发生，bundled 文件随二进制分发）。
fn glm4() -> Option<&'static Tokenizer> {
    static TOK: OnceLock<Option<Tokenizer>> = OnceLock::new();
    TOK.get_or_init(|| match load(GLM4_ZST) {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::warn!(error = %e, "glm-4 tokenizer load failed; falling back to cl100k");
            None
        }
    })
    .as_ref()
}

/// HF qwen2 tokenizer 缓存。
fn qwen2() -> Option<&'static Tokenizer> {
    static TOK: OnceLock<Option<Tokenizer>> = OnceLock::new();
    TOK.get_or_init(|| match load(QWEN2_ZST) {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::warn!(error = %e, "qwen2 tokenizer load failed; falling back to cl100k");
            None
        }
    })
    .as_ref()
}

/// 估算 `text` 在 `model` tokenizer 下的 token 数（不含特殊 token）。
///
/// 失败链路：HF 加载/编码异常 → cl100k → chars/4。永不 panic。
pub(crate) fn count_tokens(text: &str, model: &str) -> usize {
    match pick_encoding(model) {
        Encoding::O200k => o200k().encode_ordinary(text).len(),
        Encoding::Cl100k => cl100k().encode_ordinary(text).len(),
        Encoding::Glm4 => match glm4() {
            Some(t) => t.encode(text, false).map(|e| e.get_ids().len()).unwrap_or_else(|e| {
                tracing::warn!(error = %e, model = %model, "glm-4 encode failed; fallback cl100k");
                cl100k().encode_ordinary(text).len()
            }),
            None => cl100k().encode_ordinary(text).len(),
        },
        Encoding::Qwen2 => match qwen2() {
            Some(t) => t.encode(text, false).map(|e| e.get_ids().len()).unwrap_or_else(|e| {
                tracing::warn!(error = %e, model = %model, "qwen2 encode failed; fallback cl100k");
                cl100k().encode_ordinary(text).len()
            }),
            None => cl100k().encode_ordinary(text).len(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_encoding_basic() {
        assert_eq!(pick_encoding("gpt-4o"), Encoding::O200k);
        assert_eq!(pick_encoding("gpt-4.1-mini"), Encoding::O200k);
        assert_eq!(pick_encoding("gpt-5"), Encoding::O200k);
        assert_eq!(pick_encoding("o3-mini"), Encoding::O200k);
        assert_eq!(pick_encoding("GPT-4O"), Encoding::O200k); // 大小写不敏感
        assert_eq!(pick_encoding("gpt-4"), Encoding::Cl100k);
        assert_eq!(pick_encoding("gpt-3.5-turbo"), Encoding::Cl100k);
        assert_eq!(pick_encoding("claude-opus-4-8"), Encoding::Cl100k);
        assert_eq!(pick_encoding("deepseek-chat"), Encoding::Cl100k);
        assert_eq!(pick_encoding("unknown-model"), Encoding::Cl100k);
        assert_eq!(pick_encoding("glm"), Encoding::Glm4);
        assert_eq!(pick_encoding("glm-4.6"), Encoding::Glm4);
        assert_eq!(pick_encoding("GLM-5.1"), Encoding::Glm4);
        assert_eq!(pick_encoding("qwen-max"), Encoding::Qwen2);
        assert_eq!(pick_encoding("Qwen2.5-Coder"), Encoding::Qwen2);
    }

    #[test]
    fn count_tokens_ascii_matches_tiktoken() {
        // 英文 ASCII：cl100k 与 chars/4 接近，但必须返回 >0
        let n = count_tokens("hello world", "gpt-4");
        assert!(n > 0);
        // o200k 与 cl100k 对 "hello world" 都给 2，二者一致
        assert_eq!(count_tokens("hello world", "gpt-4o"), 2);
        assert_eq!(count_tokens("hello world", "gpt-4"), 2);
    }

    #[test]
    fn count_tokens_chinese_nonzero() {
        // 中文 BPE 分词：每字 1-2 token，但绝不能按 chars/4 严重低估
        let n = count_tokens("你好世界", "claude-3-opus");
        assert!(n >= 2, "expected >=2 tokens for 4 Chinese chars, got {n}");
    }

    #[test]
    fn count_tokens_glm_fallback_safe() {
        // GLM tokenizer.json bundled 随二进制，加载必成功，但即便失败也走 cl100k 兜底
        let n = count_tokens("hello", "glm-4");
        assert!(n > 0);
        let n = count_tokens("hello", "qwen-max");
        assert!(n > 0);
    }

    #[test]
    fn count_tokens_empty() {
        assert_eq!(count_tokens("", "gpt-4"), 0);
        assert_eq!(count_tokens("", "glm-4"), 0);
    }

    #[test]
    fn bundled_zst_decompresses_to_source_json() {
        for (zst, name) in [(GLM4_ZST, "glm-4.json"), (QWEN2_ZST, "qwen2.json")] {
            let path = format!("{}/assets/tokenizers/{name}", env!("CARGO_MANIFEST_DIR"));
            let src = std::fs::read(&path).expect("read source tokenizer json");
            let out = decompress(zst).expect("decompress");
            assert_eq!(out.len(), src.len(), "{name}: decompressed length differs from source");
            assert!(out == src, "{name}: decompressed bytes differ from source");
        }
    }

    /// 基线取自压缩改造前（include_bytes 原始 JSON）的实测计数。
    #[test]
    fn hf_counts_match_pre_compression_baseline() {
        for m in ["glm-4.6", "qwen-max"] {
            assert_eq!(count_tokens("Hello, world!", m), 4, "{m}");
            assert_eq!(count_tokens("你好，世界！今天天气不错。", m), 8, "{m}");
            assert_eq!(count_tokens("fn main() { println!(\"hi\"); }", m), 9, "{m}");
        }
    }
}
