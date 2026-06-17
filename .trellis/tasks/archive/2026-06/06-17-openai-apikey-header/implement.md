# Implementation Plan — openai 协议 api-key 头双发

## 单一交付（轻量），改 `src-tauri/src/gateway/proxy.rs`，worktree 隔离

### 改动：openai 协议鉴权处叠加 `api-key:` 头（保留 Bearer）

在以下 openai 分支，`Authorization: Bearer` 之外**追加** `.header("api-key", api_key)`：

1. `apply_default_headers`（:3114）—— `_ =>`（含 OpenAI）分支：Bearer 后加 api-key。
2. `apply_claude_code_family_headers`（:3144）—— `Protocol::OpenAI` 分支 + `_ =>` 分支。
3. `apply_codex_family_headers`（:3170）—— `Protocol::OpenAI` 分支。
4. `apply_models_auth`（:2266）—— `_ =>`（OpenAI 兼容）分支：/models 拉取也叠加 api-key（保 fetchModels 对小米一致）。

注意：仅 openai/兼容协议加；Anthropic（x-api-key）/Gemini（x-goog-api-key）分支**不动**。Bearer 保留（叠加非替换）。

### 验证（exec agent 在 worktree 内）

- `cd <worktree>/src-tauri && cargo build` 绿、`cargo clippy` 无 warning、`cargo test` 绿（proxy 相关 #[test] 不回归）。
- 改动仅 proxy.rs 这 4 处鉴权函数；diff 最小。
- 若有针对 header 构建的单测，确认 openai 路径同时含 Authorization + api-key。

## 增量（exec 后发现的安全缺陷，必修）

新增 `api-key` 头是凭证，但 proxy_log 脱敏链仅匹配 `authorization` → 会明文记录 api-key 值，违反「禁外传凭证」。**必修**：
- 排查 proxy_log 记录 upstream/request header 的所有点（proxy.rs :744 request_headers、:1941 upstream_request_headers、:2179/:2382/:2584 硬编 JSON、:3285 等），凡可能含 `api-key` 值处，把 `api-key`（建议连同 `x-api-key` / `x-goog-api-key` 一并）纳入 [REDACTED] 脱敏判定（不区分大小写）。
- 确认转换路径（apply_*_headers 构建的上游请求）若日志记录真实头，则 api-key 被 redact；若日志走硬编 JSON 字符串则确认不泄露真实值。
- cargo build/clippy/test 仍绿。

## 失败处理

- cargo build/clippy/test 失败 → worktree 内定点修 ≤2 轮，仍败回传错误摘要 + 标 `需要:`，禁宣称成功。
- 若发现某 apply_* 函数签名不带 api_key 或结构不同 → 按实际结构适配，回传说明。
