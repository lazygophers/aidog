# Support MiniMax + Codex + Claude Code Protocols + Default Base URLs

## 背景

aidog 是 AI 代理网关，当前支持 4 种协议: Anthropic / OpenAI / GLM / Kimi。
新增 3 种"厂商绑定"协议（带默认 base_url），保留 openai/anthropic 为通用协议（不填默认 URL）。

## 协议规划

| 协议 | 适配器 | 默认 base_url | endpoint |
|------|--------|---------------|----------|
| anthropic | 独立 (Anthropic) | — (用户自填) | `/v1/messages` |
| openai | 独立 (OpenAI) | — (用户自填) | `/v1/chat/completions` |
| glm | 复用 OpenAI | `https://open.bigmodel.cn/api/paas/v4` | `/api/paas/v4/chat/completions` |
| kimi | 复用 OpenAI | `https://api.moonshot.cn/v1` | `/v1/chat/completions` |
| **minimax** | 复用 OpenAI | `https://api.minimaxi.com/v1` | `/v1/text/chatcompletion_v2` |
| **codex** | 复用 OpenAI | `https://api.openai.com/v1` | `/v1/chat/completions` |
| **claude_code** | 复用 Anthropic | `https://api.anthropic.com` | `/v1/messages` |

## 需求

### R1: Rust 后端 — 新增 3 个 Protocol 变体 + 适配器

1. `models.rs`: Protocol enum 新增 `MiniMax` / `Codex` / `ClaudeCode`
2. 新建 `adapter/minimax.rs`:
   - `to_minimax()` 复用 `openai::to_openai()` 转换
   - `parse_minimax_sse()` 复用 `openai::parse_openai_sse()`
   - endpoint: `/v1/text/chatcompletion_v2`
3. 新建 `adapter/codex.rs`:
   - `to_codex()` 复用 `openai::to_openai()` 转换
   - `parse_codex_sse()` 复用 `openai::parse_openai_sse()`
   - endpoint: `/v1/chat/completions`
4. 新建 `adapter/claude_code.rs`:
   - `to_claude_code()` 复用 `anthropic::to_anthropic()` 转换
   - `parse_claude_code_sse()` 复用 `anthropic::parse_anthropic_sse()`
   - endpoint: `/v1/messages`
5. `converter.rs`: 新增 3 个 match 分支
6. `mod.rs`: 新增 `pub mod minimax; pub mod codex; pub mod claude_code;`
7. `proxy.rs`:
   - `ClaudeCode` 需要加 `anthropic-version` + `x-api-key` header（同 Anthropic）
   - `MiniMax` / `Codex` 用标准 Bearer auth（同 OpenAI）

### R2: 前端 — 默认 base_url + 新协议

1. `api.ts`: Protocol type 新增 `"minimax" | "codex" | "claude_code"`
2. `Platforms.tsx`:
   - `DEFAULT_BASE_URLS` 常量 map
   - `PROTOCOLS` 数组新增 3 项
   - `onChange` protocol 时自动填充 base_url（仅当 base_url 为空或等于旧默认值时）
3. `locales/*.json` × 7: 新增 `protocol.minimax` / `protocol.codex` / `protocol.claude_code`

## 默认 base_url 映射 (前端常量)

```typescript
const DEFAULT_BASE_URLS: Partial<Record<Protocol, string>> = {
  glm: "https://open.bigmodel.cn/api/paas/v4",
  kimi: "https://api.moonshot.cn/v1",
  minimax: "https://api.minimaxi.com/v1",
  codex: "https://api.openai.com/v1",
  claude_code: "https://api.anthropic.com",
};
```

## 涉及文件

### 新建
- `src-tauri/src/gateway/adapter/minimax.rs`
- `src-tauri/src/gateway/adapter/codex.rs`
- `src-tauri/src/gateway/adapter/claude_code.rs`

### 修改
- `src-tauri/src/gateway/models.rs` — Protocol enum
- `src-tauri/src/gateway/adapter/converter.rs` — 3 个 match 分支
- `src-tauri/src/gateway/adapter/mod.rs` — 3 个 pub mod
- `src-tauri/src/gateway/proxy.rs` — ClaudeCode header 逻辑
- `src/services/api.ts` — Protocol type
- `src/pages/Platforms.tsx` — PROTOCOLS + DEFAULT_BASE_URLS + auto-fill
- `src/locales/*.json` × 7 — 3 个新 key

## 不改
- DB schema (protocol 是 string)
- 主题 / Context / 代理主流程

## 验证

- `cargo build` 通过
- UI: 选 minimax → base_url 自动填 `https://api.minimaxi.com/v1`
- UI: 选 openai → base_url 留空
- 代理请求经 MiniMax/Codex/Claude Code 上游 → SSE 正常
