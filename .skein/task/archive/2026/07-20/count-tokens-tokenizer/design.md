# design: count_tokens 本地估算升级 per-model tokenizer

## 背景

`gateway/proxy/count_tokens.rs::estimate_input_tokens` 当前通用 `chars/4` 启发式。
透传优先 + 本地兜底链路 (`handle_count_tokens`) 不变; 仅升级本地估算路径到真 BPE 分词。
用户选最大精度方案: tiktoken-rs (openai/anthropic) + HF tokenizers (glm/qwen 原生 vocab)。

## 选型 (research/tiktoken-rs-research.md)

- **tiktoken-rs 0.12** (零 feature flag): cl100k_base + o200k_base BPE 表 `include_str!` 编译期嵌入, 离线 100% 可用, `*_singleton()` 全局单例 (与项目 OnceLock idiom 一致)。
- **tokenizers (HF)** crate: 加载 glm/qwen 等 HF Hub 发布的 `tokenizer.json` (Fast tokenizer 单文件自包含 vocab+merge)。bundled 为 `include_bytes!`, 离线可用。

## 模型族 → encoding 映射

| 模型前缀 | tokenizer | encoding |
|---|---|---|
| `gpt-4o` / `gpt-4.1` / `gpt-4.5` / `gpt-5` / `o1` / `o3` / `o4-mini` | tiktoken | o200k_base |
| `gpt-4` / `gpt-3.5` / `gpt-3.5-turbo` | tiktoken | cl100k_base |
| `claude-*` | tiktoken | cl100k_base (社区近似, 低估 5-15%, 仅预估可接受) |
| `glm-*` / `glm` | HF tokenizers | `glm-4 tokenizer.json` (THUDM/glm-4) |
| `qwen-*` | HF tokenizers | `qwen2 tokenizer.json` (Qwen/Qwen2) |
| `deepseek-*` / `kimi` / `doubao` / 未知 | tiktoken | cl100k_base (近似兜底) |

## 架构

新模块 `gateway/proxy/tokenizer.rs`:
- `TOKENIZER_CACHE: OnceLock<...>`: 缓存 tiktoken singleton + HF Tokenizer 实例 (按 encoding key 复用)。
- `fn pick_encoding(model: &str) -> Enc` 纯函数路由 — 优先精确匹配, 缺省 cl100k。
- `pub(crate) fn count_tokens(text: &str, model: &str) -> usize` 入口。
- HF tokenizer.json 资源: `src-tauri/crates/aidog_core/assets/tokenizers/*.json` (include_bytes! 加载)。

`estimate_input_tokens(body, model)` 重写:
- 收集 system + messages + tools 全部 text (沿用现有 `collect_text` 递归)。
- 拼成单串后调 `count_tokens(&text, model)`。
- 失败 (HF tokenizer 加载异常) → 回退 cl100k → 再失败回退 chars/4 (硬兜底)。
- `handle_count_tokens` 当前调用 `estimate_input_tokens(&raw_body)` 改为传 `requested_model`: `estimate_input_tokens(&raw_body, &requested_model)`。

## 体积增量预算 (s2 实测后修订, 用户已确认 +31MB)

- tiktoken cl100k+o200k BPE: +5.3 MB
- glm-4 tokenizer.json: **19.0 MB** (THUDM/glm-4-9b-chat-hf; 8 候选源仅此可达, GLM 词表 150k+ 字节回退特化, 天然大; 原估 1.8MB 偏低)
- qwen2 tokenizer.json: 6.7 MB (Qwen/Qwen2-7B)
- 总: ~+31 MB (用户 2026-07-20 确认接受)

## 不变量

- `handle_count_tokens` 透传优先链路零改
- 失败硬兜底: 任何 tokenizer 异常都不返错给客户端, 降级到 chars/4
- proxy_log 计费口径不变 (count_tokens 走 first_agg gate 跳过 stats_agg)
- 测试: per-model 分流断言 (gpt-4o ≠ gpt-4 ≠ glm ≠ qwen ≠ unknown)

## 风险

- Qwen2 tokenizer.json 体积大 → 调研期未实测, executor 阶段确认实际 inflate; 若超 10MB 改用更小 vocab 版本或回退 cl100k 近似。
- HF tokenizers crate 默认 feature 含 onig → 显式 `default-features = false` + 仅需 feature (避免拉 onig C 依赖增构建复杂度)。
