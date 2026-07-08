# 补全 openai model_list 全部官方模型

## Goal

现 preset `protocols.openai.model_list.default` 仅 4 GPT-5.4/5.5（`['gpt-5.5','gpt-5.4','gpt-5.4-mini','gpt-5.4-nano']`），research 查官方文档当前 Chat Completions 可用模型共 19 个，遗漏 15 个。用户要「最大化与 anthropic 对齐」—— 全当前可用 alias 无遗漏。

## Decision (ADR-lite)

**Context**: research（research/openai-models.md）查 developers.openai.com 全模型表 + deprecations，当前可用 Chat Completions 文本/代码模型 19 个。排除 DALL-E/realtime/audio/embedding/moderation/ChatGPT-only/deprecated/gpt-oss（hosted 不明）。
**Decision**: alias 优先（alias 本身指向 snapshot），不重复列 dated snapshot；排除已 deprecated（gpt-4o/o1 系列/o3-mini/o4-mini 等）；gpt-oss 保守不收（OpenAI hosted 推理未明）。
**Consequences**: 覆盖全部当前可用 GPT-5.x 系列 + o3 推理 + 旧代仍可用；gpt-4o 不对称（deprecated 但 mini 仍可用）→ 仅收 mini。

## 最终清单（19 id）

```json
[
  "gpt-5.5",
  "gpt-5.5-pro",
  "gpt-5.4",
  "gpt-5.4-pro",
  "gpt-5.4-mini",
  "gpt-5.4-nano",
  "gpt-5.3-codex",
  "gpt-5.2",
  "gpt-5.2-pro",
  "gpt-5.1",
  "gpt-5",
  "gpt-5-pro",
  "gpt-5-mini",
  "gpt-5-nano",
  "o3",
  "o3-pro",
  "gpt-4.1",
  "gpt-4.1-mini",
  "gpt-4o-mini"
]
```

排序：GPT-5.x 当前代（5.5/5.5-pro/5.4/5.4-pro/5.4-mini/5.4-nano）→ Codex（5.3-codex）→ GPT-5 前代（5.2/5.2-pro/5.1/5/5-pro/5-mini/5-nano）→ o 系列（o3/o3-pro）→ 旧代仍可用（4.1/4.1-mini/4o-mini）。

## Requirements

1. `src-tauri/defaults/platform-presets.json` `protocols.openai.model_list.default` 改为 19 id
2. `models.default` 保持 `{gpt: gpt-5.5}`（default 端点不变）
3. `endpoints.default` 不动
4. `src-tauri/src/gateway/proxy/passthrough.rs:238-241` STATIC_MODEL_IDS openai 段同步 19 id
5. `last_updated` 更新

## Acceptance Criteria

- [ ] preset model_list.default = 19 id（JSON 合法）
- [ ] STATIC_MODEL_IDS 含全部 19 id
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] `yarn build` clean
- [ ] 主仓零改动（worktree 内）

## Out of Scope

- 不含 DALL-E / image / realtime / audio / embedding / moderation（非 chat/completions）
- 不含 ChatGPT-only（chatgpt-latest，官方标 not for API）
- 不含 deprecated（gpt-4o / o1 系列 / o3-mini / o4-mini / gpt-4-turbo / gpt-4 / gpt-3.5-turbo 等）
- 不含 gpt-oss-120b/20b（OpenAI API hosted 推理未明，保守）
- 不重复列 dated snapshot

## Technical Notes

- 真值源 = research/openai-models.md（每 id 有 developers.openai.com 详情页 URL）
- 不对称：gpt-4o deprecated 但 gpt-4o-mini 仍可用
- gpt-5.3-codex 无 dated snapshot（仅 alias 自身）
