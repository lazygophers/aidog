# 补全 anthropic model_list 全部官方模型

## Goal

现 preset `protocols.anthropic.model_list.default` 仅 4 旗舰（且 sonnet 用上一代 sonnet-4-6），用户要「最大化含快照」—— 全公开仍可调 API id 无遗漏。按官方文档补全，排除 Mythos/Bedrock 变体/已 Retired。

## Decision (ADR-lite)

**Context**: research（research/anthropic-models.md §6）查官方 overview + deprecations 页，全公开仍可调 API id 共 10 个。用户选「最大化」但去将下线 Opus4.1（2026-08-05 retire，剩 1 月）→ **9 id**。
**Decision**: alias 优先（4.6+ dateless 本身即 pinned snapshot；Haiku4.5 官方背书 alias），pre-4.6 无 alias 的用 dated id。不重复列 alias+dated。
**Consequences**: 覆盖全部 Active anthropic 模型；Opus4.5/Sonnet4.5 必须带日期（官方命名遗留，无法规避）；不含将下线项避免短期维护。

## 最终清单（9 id）

```json
[
  "claude-fable-5",
  "claude-opus-4-8",
  "claude-sonnet-5",
  "claude-haiku-4-5",
  "claude-opus-4-7",
  "claude-opus-4-6",
  "claude-sonnet-4-6",
  "claude-opus-4-5-20251101",
  "claude-sonnet-4-5-20250929"
]
```

排序：旗舰 4（Fable/Opus/Sonnet/Haiku，overview 旗舰表序）→ Opus 前代倒序（4-7→4-6→4-5）→ Sonnet 前代（4-6→4-5）。

## Requirements

1. `src-tauri/defaults/platform-presets.json` `protocols.anthropic.model_list.default` 改为 9 id 清单
2. `models.default` 保持 opus 档 = claude-opus-4-8（default 端点不变）；同步 sonnet 档 claude-sonnet-4-6 → claude-sonnet-5（换代）
3. `src-tauri/src/gateway/proxy/passthrough.rs:234-237` STATIC_MODEL_IDS anthropic 段同步 9 id
4. `last_updated` 更新当前 Unix 秒
5. 不动 endpoints（default 单分支，无 coding_plan）

## Acceptance Criteria

- [ ] preset model_list.default = 9 id（JSON 合法，顺序如上）
- [ ] models.default.sonnet = claude-sonnet-5
- [ ] STATIC_MODEL_IDS 含全部 9 id（GET /proxy/models 返回一致）
- [ ] `cd src-tauri && cargo build` clean
- [ ] `cd src-tauri && cargo clippy` 无新 warning
- [ ] `cd src-tauri && cargo test` 过
- [ ] `yarn build` clean
- [ ] 主仓零改动（worktree 内）

## Out of Scope

- 不含 Mythos 5（Glasswing 受邀制）
- 不含 Bedrock/Vertex 路由变体（无独立 Claude API id）
- 不含 Retired 模型
- 不含 claude-opus-4-1-20250805（deprecated，2026-08-05 retire，剩 1 月）
- 不加 dated snapshot 重复列（alias 优先）

## Technical Notes

- 真值源 = research/anthropic-models.md §6（每 id 有官方文档 URL + deprecations 表行）
- snapshot 语义：4.6+ dateless = pinned snapshot（overview 注脚）；pre-4.6 alias → dated 解析
- 排除项理由见 research §6.5 Caveats
