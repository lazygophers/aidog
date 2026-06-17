# Implementation Plan — 小米 MiMo coding plan 平台变体

## 单一交付（轻量），改 `src/pages/Platforms.tsx`，worktree 隔离

### 改动清单

1. **PROTOCOLS 数组**（`:41` 普通小米条目后追加）：
   ```ts
   { value: "xiaomi_mimo", label: "小米 MiMo Coding Plan", codingPlan: true, keywords: ["xiaomi coding", "小米编程", "mimo token plan", "token plan"] },
   ```
   放在 `{ value: "xiaomi_mimo", label: "小米 MiMo", ... }` 之后（对齐 glm/kimi/qianfan 紧邻排布）。

2. **getDefaultEndpoints xiaomi_mimo**（`:220`）改为 cp 分支（参考 glm `:168` / kimi `:176`）：
   - cp=true（Token Plan，默认 cn 集群）：
     ```ts
     { protocol: "anthropic", base_url: "https://token-plan-cn.xiaomimimo.com/anthropic", client_type: "claude_code", coding_plan: true },
     { protocol: "openai", base_url: "https://token-plan-cn.xiaomimimo.com/v1", client_type: "codex_tui", coding_plan: true },
     ```
   - cp=false（按量，保持现有）：anthropic `api.xiaomimimo.com/anthropic` + openai `api.xiaomimimo.com/v1`。
   - 单 openai + 单 anthropic 端点；openai 端点隐含覆盖 openai_responses/openai_completions 变体（converter 处理），不拆多条。

3. **getDefaultModels xiaomi_mimo**（`:395`）：cp 分支可同用 `mimo-v2.5-pro`（除非 research 指出 Token Plan 专属模型 id 不同——保持 `mimo-v2.5-pro`）。

### 验证（exec agent 在 worktree 内）

- `cd <worktree> && yarn build` 绿。
- 下拉新增「小米 MiMo Coding Plan」可选；选中填 token-plan-cn 双端点 + coding_plan 标记。
- url-construction-rule：base_url 含版本前缀（/v1、/anthropic），最终 = base_url + /chat/completions（或 /v1/messages），禁额外拼接。
- **openai 侧 `api-key:` 鉴权核查**：检 aidog 上游请求对 openai 协议是否硬编 `Authorization: Bearer`。若小米 token-plan openai 端点拒 Bearer 需 `api-key:` 头 → 记录为已知限制（不在本 task 改后端，回传标注），anthropic 端点优先（x-api-key 已兼容）。本 task 不扩展到后端鉴权适配，仅前端预设。

## 失败处理

- build 失败 → worktree 内定点修，≤2 轮，仍败回传标 `需要:`。
- 若发现 openai 端点鉴权确需后端改 → 不在本 task 做，回传建议另起 task。
