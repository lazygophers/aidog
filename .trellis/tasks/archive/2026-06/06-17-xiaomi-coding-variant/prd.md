# 小米 MiMo coding plan 平台变体

## Goal

在平台下拉里增加「小米 MiMo coding plan」选项（当前只有普通小米 MiMo），对齐 glm/kimi/qianfan 的 coding plan 变体模式，选中后自动填 Token Plan 订阅端点。

## What I already know

- coding plan 变体机制：`PROTOCOLS` 数组（`src/pages/Platforms.tsx:17`）加一条 `{ value, label, codingPlan: true, keywords }`（参考 glm `:25`、kimi `:28`、qianfan `:40`）。`getDefaultEndpoints(proto, cp)` / `getDefaultModels(proto, cp)` 用 `cp` 切端点/模型。
- xiaomi_mimo 现状：`:41` 仅普通条目；`getDefaultEndpoints` `:220` 仅 anthropic（按量 host）+ openai（按量，上轮新增）；无 coding plan 分支。
- Token Plan 端点（归档 research `archive/2026-06/06-17-xiaomi-coding-plan/research/xiaomi-coding-plan-api.md`）：
  - cn: openai `https://token-plan-cn.xiaomimimo.com/v1`，anthropic `https://token-plan-cn.xiaomimimo.com/anthropic`
  - sgp/ams 同构（`token-plan-sgp/ams`）
  - key `tp-`，鉴权 `api-key:` 头（anthropic SDK `x-api-key` 实测兼容；openai 侧默认 `Bearer` 与官方 `api-key:` 不同，需验证）
- 配额：Token Plan 无 API-Key 配额接口 → 走 manual_budget 兜底（见记忆 [[xiaomi-mimo-token-plan-no-api]]）。

## Decision (ADR-lite)

- **Context**: 下拉缺小米 coding plan 变体；Token Plan 分 cn/sgp/ams，端点有 openai+anthropic 双协议。
- **Decision**: ①默认集群 **cn**（用户选中后可自改 base_url 切 sgp/ams）；②coding 变体填 **anthropic + openai 双端点**（token-plan-cn host），加 `coding_plan: true`；普通变体保留双按量端点 → 小米共 4 套端点全支持；③配额走 manual_budget（无 API）。
- **Consequences**: openai 侧官方用 `api-key:` 头，aidog 默认 `Bearer`，exec 需验证是否需后端适配（按量 anthropic 已工作，x-api-key 兼容）。
- **端点协议变体（用户澄清）**: endpoint 声明 `protocol: "openai"` 已隐含支持 openai 两种变体（openai_responses / openai_completions），由 aidog converter 处理——与 glm/kimi coding 端点同模式，**不为变体拆多条端点**，单 openai + 单 anthropic 端点即可。

## Requirements (evolving)

- [ ] 平台下拉新增「小米 MiMo coding plan」（codingPlan: true）。
- [ ] 选中后 getDefaultEndpoints 填 token-plan 集群端点 + coding_plan 标记。

## Acceptance Criteria (evolving)

- [ ] 下拉可见并可选「小米 coding plan」变体。
- [ ] yarn build 绿。

## Out of Scope

- 小米 Token Plan 实时配额查询 API（无公开接口，走 manual_budget）。

## Technical Notes

- 触点：`src/pages/Platforms.tsx`（PROTOCOLS + getDefaultEndpoints + getDefaultModels 的 cp 分支）。
- 参考记忆：[[xiaomi-mimo-token-plan-no-api]]、[[coding-plan-client-type-whitelist]]、[[aidog-add-platform-skill]]、[[url-construction-rule]]。
