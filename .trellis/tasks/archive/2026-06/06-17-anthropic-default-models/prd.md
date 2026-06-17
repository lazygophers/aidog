# PRD: Anthropic 默认模型改 opus-4-8 / haiku-4-6

## 现状

`getDefaultModels` (Platforms.tsx:376) anthropic preset：
```
anthropic: { opus: "claude-opus-4-8", sonnet: "claude-sonnet-4-6", haiku: "claude-haiku-4-5-20251001" }
```
- 无 `default` slot（ModelSlot = default|sonnet|opus|haiku|gpt）。
- opus 已是 claude-opus-4-8 ✓。
- haiku 是 claude-haiku-4-5-20251001。

## 目标（用户指定值，用户即权威，不 WebSearch）

- **默认（default slot）**：claude-opus-4-8
- **Haiku**：claude-haiku-4-6
- 其余（opus=claude-opus-4-8、sonnet=claude-sonnet-4-6）维持现状。

## 改动

`getDefaultModels` anthropic preset：
```
anthropic: { default: "claude-opus-4-8", opus: "claude-opus-4-8", sonnet: "claude-sonnet-4-6", haiku: "claude-haiku-4-6" }
```
（加 default slot = claude-opus-4-8；haiku 4-5-20251001 → 4-6。）

## 不改

- opus / sonnet 值。
- 其它 protocol preset。
- 后端（默认模型纯前端预设，记忆 `platform-default-model`）。
- fetchModels 兜底机制（月级腐化靠它覆盖）。

## 维护文档

`.claude/skills/aidog-add-platform/references/default-model.md` 若列了 anthropic 默认值，同步更新（保持文档与代码一致）。

## 验收

1. `npx tsc --noEmit` 0 error。
2. dev：添加 Anthropic 平台 → 默认模型槽 = claude-opus-4-8，Haiku 槽 = claude-haiku-4-6，Opus/Sonnet 不变。

## 文件 / 范围

- `src/pages/Platforms.tsx`：getDefaultModels anthropic preset 一行。
- （可选）`.claude/skills/aidog-add-platform/references/default-model.md`。

## subtask

单一交付（一行 preset + 可选文档），不拆 child。
