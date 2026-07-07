# PRD: doubao preset endpoint 重复数据修复

## Goal

doubao 平台预设 `endpoints.default` 单数组塞 6 条（coding 三元 anthropic/openai/openai_responses + plan 三元 anthropic/openai/openai_responses），用户判「存在重复数据」。修复结构 + grep 全 protocol 找类似问题一并修。

## What I already know（auto-context 探明）

- 当前 doubao `endpoints.default` 6 条（`src-tauri/defaults/platform-presets.json`）:
  - `[0]` anthropic `/api/coding` claude_code
  - `[1]` openai `/api/coding/v3` codex_tui
  - `[2]` openai_responses `/api/coding/v3` codex_tui
  - `[3]` anthropic `/api/plan` claude_code
  - `[4]` openai `/api/plan/v3` codex_tui
  - `[5]` openai_responses `/api/plan/v3` codex_tui
- 全 protocol 扫 (protocol,base_url) 真重复 = **0**（无完全相同条目）
- 正确 schema 参考（7 例）: glm / kimi / minimax / minimax_en / bailian / qianfan / xiaomi_mimo 均 `endpoints` = `{default: [...], coding_plan: [...]}` 两 key 分离，coding_plan 分支每条带 `"coding_plan": true`
  - kimi: default = 普通端点（moonshot.cn），coding_plan = `[openai /api.kimi.com/coding/v1 coding_plan:true]`
  - glm: default = 普通，coding_plan = `[openai .../coding/paas/v4 codex_tui, anthropic .../api/anthropic claude_code]` 均 `coding_plan:true`
- **doubao 是唯一把 cp 三元混塞 default 的协议**（结构错位，非真数据重复）
- 消费侧已就绪: `proxy/endpoint.rs:53-56` coding_plan 端点按入站协议精确匹配 + 回退 openai coding

## Decision (ADR-lite)

**Context**: doubao `endpoints.default` 6 条 cp 三元混塞，3 protocol 各出现 2 次（coding 版 + plan 版），用户定义「重复 = 同 protocol 类型在同平台多次」。

**Decision**:
- 「重复」语义 = **结构错位**（用户确认: "base_url 的类型，在同一个平台存在多个相同的类型"）
- 拆分方向 = `default` + `coding_plan` 两 key 分离（对齐 7 例参考协议）
- 归属: **`/api/coding` 三元 → `coding_plan` 分支**（加 `coding_plan:true`）；`/api/plan` 三元 → `default` 分支
  - 理由: glm/kimi/minimax/bailian/qianfan/xiaomi_mimo 7 例 coding_plan 分支 URL 均含 "coding"（如 glm `/api/coding/paas/v4`，kimi `/api/coding/v1`），doubao `/api/coding` 命名与惯例一致
  - 用户超时未答，按推荐推进（7 例 URL 命名一致是强信号）

**Consequences**:
- doubao default 从 6 条 → 3 条（plan 三元），新增 coding_plan 3 条
- 每分支内同 protocol 只 1 次，重复消除
- 消费侧 `proxy/endpoint.rs:53-56` coding_plan 路由已就绪，无需改 Rust
- 全 protocol 扫确认仅 doubao 命中（无其他同类问题需修）

## Requirements

- 修 `src-tauri/defaults/platform-presets.json` protocols.doubao.endpoints: default 6 条 → default 3 条（plan 三元）+ coding_plan 3 条（coding 三元，每条加 `coding_plan:true`）
- models/model_list/name/desc/source_urls/client_type 不动
- 全 protocol 扫确认无其他同 protocol 重复（已扫，仅 doubao）

## Requirements（evolving）

- 修 doubao endpoints 结构
- grep 全 protocol 确认无其他同类问题

## Acceptance Criteria（evolving）

- [ ] `python3 -m json.tool` 有效
- [ ] doubao 结构与 7 参考协议之一对齐
- [ ] 全 protocol 扫无 cp 三元混塞 default
- [ ] cargo build/test 不回归

## Out of Scope

- 不改其他 protocol 条目（除非 grep 发现同类）
- 不改 models / model_list / name / desc
- 不改 DB 存量（preset = 模板，存量用户数据不动，与 codex 任务同策略）

## Technical Notes

- 改点: `src-tauri/defaults/platform-presets.json` protocols.doubao.endpoints
- 参考: protocols.{glm,kimi,minimax}.endpoints
- 消费: `proxy/endpoint.rs:53-56`（coding_plan 路由已就绪）
