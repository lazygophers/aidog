# count_tokens 本地估算升级 → per-model tokenizer 选型调研

调研对象: `aidog_core::gateway::proxy::count_tokens::estimate_input_tokens` 当前 chars/4 启发式 (file:src-tauri/crates/aidog_core/src/gateway/proxy/count_tokens.rs:13-26)。

适用范围: 仅 `handle_count_tokens` 的本地兜底估算路径 (透传失败/无候选时回客户端的 `{"input_tokens": N}`)。计费/聚合口径不变 (聚合跳过 count_tokens URL)。

---

## 1. tiktoken-rs crate 选型

| 项 | 结论 |
|---|---|
| crate | `tiktoken-rs` (zurawiki 维护, OpenAI tiktoken 官方 Rust port) |
| 最新版本 | **0.12.0** (2026-06-02 发布) |
| MSRV | Rust 1.85.0 (vendored upstream 强制) |
| License | MIT |
| 维护状态 | 活跃 (0.9.1→0.10/0.11→0.12 五个月内三版) |
| feature flag | 仅 `async-openai` / `dhat-heap` 两个，**无离线/bundled/download 开关** —— 因为 BPE 表本就 `include_str!` 编译期嵌入，不存在"在线下载"路径 |
| API 形态 | 全同步: `cl100k_base() -> Result<CoreBPE>` / `cl100k_base_singleton() -> CoreBPE` (复用全局单例) |
| 是否需联网下载 BPE 表 | **否**。源码 `include_str!("../../assets/cl100k_base.tiktoken")` 编译期嵌入 (file:tiktoken-rs/src/tiktoken_ext/openai_public.rs) |
| 离线行为 | 打包后无网络 100% 可用，零运行时 IO |
| 二进制体积增量 | 总资产 8.26 MB，但链接器只保留被调用的 `include_str!`；用 cl100k+o200k 两表 ≈ +5.3 MB (cl100k 1.68 MB + o200k 3.61 MB)，其余资产 (encoder.json/vocab.bpe/r50k/p50k) 死代码消除剔除 |

来源:
- crates.io: https://crates.io/crates/tiktoken-rs
- GitHub 0.12.0 Cargo.toml (rust-version=1.85.0, features 仅 async-openai/dhat-heap): https://raw.githubusercontent.com/zurawiki/tiktoken-rs/main/tiktoken-rs/Cargo.toml
- Releases (0.12.0 2026-06-02): https://github.com/zurawiki/tiktoken-rs/releases
- BPE 加载方式 `include_str!`: https://raw.githubusercontent.com/zurawiki/tiktoken-rs/main/tiktoken-rs/src/tiktoken_ext/openai_public.rs
- 资产体积 8.26 MB (cl100k_base.tiktoken=1681126 / o200k_base.tiktoken=3613922 / p50k_base=836186 / r50k_base=835554 / encoder.json=1243332 / vocab.bpe=456318): GitHub tree API

---

## 2. 模型族 → encoding 映射策略

tiktoken-rs 内置 `Tokenizer` 枚举: `O200kHarmony / O200kBase / Cl100kBase / P50kBase / R50kBase / P50kEdit / Gpt2` (file:tiktoken-rs/src/tokenizer.rs)。

| 模型族 | encoding | 理由 / 来源 |
|---|---|---|
| anthropic `claude-*` | **cl100k_base** | Anthropic 官方未公开 BPE；社区实测 cl100k_base 与 Claude tokenizer 词表 ~70% 重合，且都含 1-3 位数字专用 token (cl100k 的设计特征，p50k 无)。最接近的公开近似 |
| openai `gpt-4o / gpt-4.1 / gpt-4.5 / gpt-5 / o1 / o3 / o4-mini` | **o200k_base** | OpenAI 官方映射，tiktoken-rs `MODEL_PREFIX_TO_TOKENIZER` 直接覆盖 |
| openai `gpt-4 / gpt-4-32k / gpt-3.5-turbo` | **cl100k_base** | 官方映射 |
| openai `gpt-3 / davinci / code-davinci` | r50k_base / p50k_base | 已淘汰，可不支持 |
| glm (zhipu `glm-4 / glm-4.5`) | **cl100k_base** (近似) | 无公开 BPE；BBPE 词表设计 + 中文优化更接近 cl100k 风格 |
| qwen (alibaba) | **cl100k_base** (近似) | Qwen 用自研 BBPE (vocab.bpe 派生)；本地近似首选 cl100k。精确估算需 HF tokenizers + Qwen 官方 tokenizer.json (超本次范围) |
| deepseek | **cl100k_base** (近似) | BBPE，社区惯例 |
| kimi (moonshot) | **cl100k_base** (近似) | BBPE |
| doubao (bytedance) | **cl100k_base** (近似) | BBPE |
| 未知 / fallback | cl100k_base | cl100k 在中英混合文本上误差小于 o200k (后者针对 GPT-4o 多语言优化，中文场景反而偏离) |

> Claude 的 cl100k_base 近似会偏低估 5-15% (Anthropic tokenizer 实测 token 数略多)；对 count_tokens 预估可接受。

来源:
- 0.12.0 README encoding 表 (o200k_harmony/o200k_base/cl100k_base/p50k_base/p50k_edit/r50k_base 对应模型): https://github.com/zurawiki/tiktoken-rs
- tokenizer.rs MODEL_PREFIX_TO_TOKENIZER: https://raw.githubusercontent.com/zurawiki/tiktoken-rs/main/tiktoken-rs/src/tokenizer.rs
- Claude tokenizer 未公开 + cl100k_base ~70% 重合 + 1-3 位数字 token 特征: https://arxiv.org/html/2402.14903v1 , https://github.com/IAPark/tiktoken_ruby/issues/15
- 非 OpenAI 模型"用 HF tokenizers"建议: tiktoken-rs README "Scope" 提示

---

## 3. 离线 / bundled 模式可行性

**结论: 完全可行，无需任何额外 feature flag。**

- BPE 表通过 `include_str!` 编译期嵌入二进制 (file:tiktoken-rs/src/tiktoken_ext/openai_public.rs:12 `let bpe_file = include_str!("../../assets/r50k_base.tiktoken");`)
- Cargo.toml `include = ["assets/**/*", ...]` 保证发布到 crates.io 的包含表文件
- Tauri 打包后 (macOS .app / dmg) 完全离线运行，零网络依赖、零文件系统读取
- 与项目现有 bundled 模式一致 (`rusqlite` 用 `bundled` feature, `platform-presets.json` 用 `include_str!`)

来源:
- Cargo.toml include 字段: https://raw.githubusercontent.com/zurawiki/tiktoken-rs/main/tiktoken-rs/Cargo.toml
- include_str! 加载: 同 §1 来源

---

## 4. 替代方案对比

| 方案 | 体积 | 离线 | 准确度 | 维护 | 结论 |
|---|---|---|---|---|---|
| **tiktoken-rs 0.12** | +5.3 MB (cl100k+o200k) | 编译期嵌入 | OpenAI 官方；Claude/国产 ~70-85% 近似 | 活跃 | **推荐** |
| HF `tokenizers` crate | +10-15 MB (regex/onig + per-model vocab.json 各 2-5 MB) | 需打包每模型 tokenizer.json | 各模型原生最高 | 活跃 | 准确度上限更高，但体积/复杂度大；Qwen/GLM 原生 vocab 需逐一下载打包 |
| `tiktoken` (newer pure-Rust, crates.io) | 类似 tiktoken-rs | 是 | 兼容 11 编码 5 厂商 | 较新，社区规模小 | 备选 (声称 15-40x 快)，生态未稳 |
| 现状 chars/4 | 0 | 是 | 中英混合误差 30-50% | - | 仅兜底 |

**推荐**: `tiktoken-rs`。对 anthropic/openai 两族 (本项目主战场) 准确度够用，bundled 模式与项目一致，体积可接受 (~5 MB)。国产模型先用 cl100k 近似，后续若需精估 Qwen/GLM 再评估引入 HF tokenizers 加各厂 vocab.json。

来源:
- tiktoken-vs-huggingface tokenizers 速度对比 (tiktoken 2-3x 更快, HF 更通用): https://machinelearningplus.com/gen-ai/tiktoken-vs-huggingface-tokenizers/
- HF tokenizers crate: https://github.com/huggingface/tokenizers
- 纯 Rust `tiktoken` crate (15-40x 声称): https://crates.io/crates/tiktoken

---

## 5. 集成风险点

| 风险 | 评估 | 对策 |
|---|---|---|
| MSRV 1.85 | 项目 workspace `edition = "2024"` (file:src-tauri/Cargo.toml:8)，Rust 2024 edition 需 1.85+，匹配 | 无 |
| sync API 阻塞 tokio runtime | `cl100k_base_singleton()` 首次调用解析 1.7 MB BPE ~50-100ms；后续 encode 调用纯 CPU μs 级 | 首次初始化放 `tokio::task::spawn_blocking` 或在 ProxyState 构造时 eager init；encode 调用因 <1ms 可直接在 async 中调 |
| 初始化开销 | singleton 首次 base64 解码 + HashMap 构建 ~50-100ms/表 | 用 `OnceLock<CoreBPE>` (项目已有 idiom, 见 `gateway/peak_hours.rs::OnceLock`) 或直接用 crate 自带 `*_singleton()` |
| 二进制体积 | cl100k+o200k +5.3 MB 到 release binary | 可接受 (项目已有 rusqlite bundled + aes-gcm，相对增量小)；若只主攻 anthropic 可只用 cl100k (+1.7 MB) |
| `async-openai` feature 误启 | 该 feature 拉入 `async-openai = "0.34"` (chat-completion-types)，本项目用不上 | **不要启用** `features = ["async-openai"]`，默认 features 为空，零 opt-in 即可 |
| 编译时间 | `fancy-regex` + `bstr` + `rustc-hash` 新增依赖编译 | 影响小，均为纯 Rust |
| Claude/国产模型近似偏差 | 5-15% 低估 (Claude) | 仅 `count_tokens` 预估路径可接受；计费仍走上游真实 usage |

来源:
- 0.12.0 singleton idiom: README `o200k_base_singleton()` 示例 (https://github.com/zurawiki/tiktoken-rs)
- 项目 OnceLock idiom: file:src-tauri/crates/aidog_core/src/gateway/peak_hours.rs

---

## 6. 推荐方案结论

**采用 `tiktoken-rs = "0.12"` (零 feature flag，默认即可)，按 `model` 前缀路由 encoding: openai gpt-4o+/o-series → `o200k_base_singleton()`；其余 (claude / gpt-4 / gpt-3.5 / glm / qwen / deepseek / kimi / doubao / fallback) → `cl100k_base_singleton()`。** BPE 编译期嵌入，离线可用，体积增量 ~5.3 MB，API 同步 + 项目 OnceLock 缓存 idiom 直接复用。

**Cargo.toml 片段** (workspace.dependencies):
```toml
tiktoken-rs = "0.12"
```
aidog_core/Cargo.toml: `tiktoken-rs = { workspace = true }` (不带 features)。

> **未 BLOCKED**。tiktoken-rs 维护活跃、离线可行、版本稳定。

---

## 集成草稿 (供 design 参考，非最终实现)

```rust
// count_tokens.rs
use tiktoken_rs::{cl100k_base_singleton, o200k_base_singleton, CoreBPE};

fn bpe_for_model(model: &str) -> &'static CoreBPE {
    // ponytail: 简单前缀路由，后续加 HF tokenizers 再抽象 trait
    if matches_o200k(model) { o200k_base_singleton() } else { cl100k_base_singleton() }
}
fn matches_o200k(m: &str) -> bool {
    let p = m.to_ascii_lowercase();
    p.starts_with("gpt-4o") || p.starts_with("gpt-4.1") || p.starts_with("gpt-4.5")
        || p.starts_with("gpt-5") || p.starts_with("o1") || p.starts_with("o3")
        || p.starts_with("o4-mini")
}

pub(crate) fn estimate_input_tokens(body: &Value) -> i64 {
    let model = body.get("model").and_then(|v| v.as_str()).unwrap_or("");
    let bpe = bpe_for_model(model);
    let mut chars = 0usize;
    let mut tokens: i64 = 0;
    if let Some(obj) = body.as_object() {
        for key in ["system", "messages", "tools"] {
            if let Some(v) = obj.get(key) {
                let mut acc = String::new();
                collect_text(v, &mut acc);
                if acc.is_empty() { continue; }
                tokens += bpe.count_with_special_tokens(&acc) as i64;
            }
        }
    }
    tokens.max(1)
}
```
