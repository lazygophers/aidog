# 平台预设：百度千帆 Coding Plan Lite

## Goal

新增平台预设「百度千帆 Coding Plan Lite」，双协议端点：OpenAI 兼容 `https://qianfan.baidubce.com/v2/coding`、Anthropic 兼容 `https://qianfan.baidubce.com/anthropic/coding`。

## What I already know（现状）

- Protocol `qianfan` **已存在**（`src-tauri/src/gateway/models.rs:52` serde rename "qianfan"，label「百度千帆」）→ **无需改 models.rs / Protocol 枚举**。
- 前端 `src/pages/Platforms.tsx`：
  - `PROTOCOLS` 已有 `{ value:"qianfan", label:"百度千帆", keywords:["baidu","百度","千帆"] }`（line 52）。
  - `getDefaultEndpoints` 的 qianfan（line 226-227）**当前仅 1 端点**：`{ protocol:"anthropic", base_url:"https://qianfan.baidubce.com/anthropic/coding", client_type:"claude_code" }`。**缺 OpenAI 端点**。
  - GLM Coding Plan 参考：PROTOCOLS `{ value:"glm", label:"GLM Coding Plan", codingPlan:true, keywords:[...] }`(line 38)；getDefaultEndpoints glm(181-182) cp 分支给双端点 + `coding_plan: cp` + openai client_type "codex_tui"。

## Requirements

- R1 `PROTOCOLS` 加一条：`{ value:"qianfan", label:"百度千帆 Coding Plan Lite", codingPlan:true, keywords:["baidu","百度","千帆","qianfan","coding"] }`。
- R2 `getDefaultEndpoints` 的 qianfan：codingPlan 时给**双端点**：
  - `{ protocol:"openai", base_url:"https://qianfan.baidubce.com/v2/coding", client_type:"codex_tui", coding_plan:true }`
  - `{ protocol:"anthropic", base_url:"https://qianfan.baidubce.com/anthropic/coding", client_type:"claude_code", coding_plan:true }`
  - 非 codingPlan（普通「百度千帆」）保留原有端点不破坏（现有 anthropic coding 端点——或按千帆普通 base_url，保持现状）。
- R3 不破坏现有 qianfan 普通预设 / 其他平台。
- R4 colors/labels 已有（PLATFORM_NAMES qianfan「百度千帆」、PROTOCOL_COLORS qianfan #2932E1）——复用，无需新增。

## Acceptance Criteria

- [ ] 平台选择器搜「千帆/coding」可选「百度千帆 Coding Plan Lite」。
- [ ] 选中后默认生成 OpenAI(v2/coding)+Anthropic(anthropic/coding) 双端点，coding_plan 标记 true。
- [ ] 现有「百度千帆」普通预设不变；tsc 0。

## Out of Scope

- 不改 models.rs（qianfan 已存在）。
- 不加 quota.rs 真查支持（千帆 coding plan 配额真查非本次；可手动预算）。

## Technical Notes

- 仅改 `src/pages/Platforms.tsx`（PROTOCOLS + getDefaultEndpoints）。
- URL 构造规则：base_url 含完整路径，`provider_api_path()` 加 `/chat/completions`。确认 `v2/coding` + `/chat/completions` = `v2/coding/chat/completions` 符合千帆 OpenAI 兼容路径（如不符按千帆文档调整，回报）。
- **依赖**：与运行中 window-unit 任务共享 Platforms.tsx → 实施须等其合并后再起。
