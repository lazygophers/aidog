# PRD — 火山方舟 CodingPlan 双端点智能粘贴识别 + openai 默认配置修复

## 背景
社区分享火山方舟（Volcengine Ark）CodingPlan Lite，单平台双协议端点：
- Anthropic：`https://ark.cn-beijing.volces.com/api/coding`
- OpenAI：`https://ark.cn-beijing.volces.com/api/coding/v3`

智能粘贴该分享文案时两个缺陷。载体 = 前端 `src/utils/platformPaste.ts`（解析）+ `src/pages/Platforms.tsx`（doubao preset / applyPaste 填表）。
参考记忆 `volces-dual-endpoint-substring-match`（火山双协议 base_url 最长子串识别）。

## 缺陷清单与期望（用户原文 → 期望 → 根因）

| # | 用户原文 | 期望 | 根因定位 |
|---|---|---|---|
| 1 | 没有准确识别这是两个不同的 base_url | 两条 URL 各自落到对应协议 endpoint（anthropic→/api/coding，openai→/api/coding/v3），不塌缩 | `platformPaste.ts:266-272` `guessProtocol`：`/api/coding` 无 `anthrop/openai/v1` → `unknown`；`/api/coding/v3` 的 `v3` 不匹配正则 `/v1` → 也 `unknown`。`Platforms.tsx:1637` applyPaste 把 `unknown→openai`，两条同判 openai，:1638 按协议去重 → 第二条 base_url 覆盖第一条，两端点塌缩成一个 |
| 2 | 平台的默认配置错误，openai 的都是有 v3 结尾的 | doubao 默认 endpoints 含标准 `openai` chat/completions 端点，base_url = `.../api/coding/v3`（v3 结尾）；保留现有 `openai_responses`（Codex） | `Platforms.tsx:233-238` doubao preset openai 侧只有 `openai_responses`（Codex），缺标准 `openai` 端点；用户用非 Codex 的 openai 客户端时无可用默认配置 |

## 用户决策（已确认，AskUserQuestion）
- **OpenAI 协议**：标准 openai + 保留 responses —— doubao preset 新增一个标准 `openai` 端点（`.../api/coding/v3`，client_type 适配 openai 客户端），同时**保留**现有 `openai_responses`（Codex `codex_tui`）端点。
- **排期**：与 Task A（06-28-06-28-coding-plan-tier-display-fix）并行，另开独立 worktree。

## 范围
- in：
  - doubao preset（getDefaultEndpoints）新增标准 `openai` 端点 `.../api/coding/v3`，保留 anthropic + openai_responses。
  - 智能粘贴双 base_url 识别：让火山两条 URL 各自映射正确协议 endpoint，不塌缩。修复方向二选一（实施阶段定夺，须复现确认）：
    - (a) 增强 `guessProtocol`：`/v\d+` 泛化识别（`/api/coding/v3` → openai）+ 火山 host `/api/coding` 段 → anthropic 的 host 感知；或
    - (b) matchPlatform 命中多端点 coding preset 时，按 host+path 最长子串把 pasted base_urls 映射到 preset endpoints（复用 hosts 派生，与 `volces-dual-endpoint-substring-match` 同模式），不靠 guessProtocol。
  - doubao `hosts` 派生（Platforms.tsx:405-423 从 getDefaultEndpoints 自动派生）随新增 openai 端点同步，确保最长子串匹配仍命中 doubao。
- out：
  - 后端 Rust（协议转换 / quota）—— 火山端点已有 adapter，本次不动。
  - 其他平台 preset。
  - 与 Task A 重叠文件零交集（Task A 改 engine.py + coding_plan.rs，本任务改 TS 前端），并行安全。

## 跨层约束
- 纯前端 TS 改动，无 Rust↔TS 边界变更。
- `getDefaultEndpoints` 是 hosts 派生单一事实源（Platforms.tsx:405-423）：base_url 只在 getDefaultEndpoints 改，hosts 自动派生，禁手写 hosts。
- 新增 openai 端点 base_url 必须 v3 结尾（`.../api/coding/v3`），与 openai_responses 同 base_url 但协议不同。
- client_type：标准 openai 端点用 openai 客户端身份（非 codex_tui），实施时核对 `defaultClientForProtocol` / 既有 openai preset 惯例。

## 验收
- 粘贴火山分享文案 → 表单出现 anthropic（/api/coding）+ openai（/api/coding/v3）两个独立 endpoint，base_url 各异不塌缩（bug1）。
- 新建 doubao 平台默认 endpoints 含标准 openai（v3 结尾）+ anthropic + openai_responses（bug2）。
- doubao 仍能被智能粘贴正确识别（hosts 最长子串命中）。
- `yarn build`（tsc + vite）全绿，无类型错误。
- 若有相关单测（platformPaste matchPlatform/guessProtocol）须同步并通过。
```

## 失败处理
- 双端点映射方案 (a)/(b) 复现后择优，若 (a) 泛化 `/v\d+` 误伤其他平台 → 退 (b) host 感知映射。
- 火山官方 client_type 不确定 → 标 `需要:`，main 转达用户。
