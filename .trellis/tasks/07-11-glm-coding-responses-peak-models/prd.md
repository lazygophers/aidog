# glm_coding preset: responses endpoint + models.peak 分支

## Goal

`glm_coding` 协议 preset 缺：(1) OpenAI Responses API 端点（用户报告「endpoints 缺少 response 的协议」）；(2) models 默认映射过时（haiku/sonnet/gpt 不对）；(3) **高峰期独立 models 映射**（用户要按时段切主力模型档，与倍率解耦）。

按用户拍板实现路径：**models 字段加 `peak` 分支**（仿 `coding_plan` 分支模式），pickBranch 按当前是否高峰选 default/peak 分支。Rust + TS 双侧改 + 路由层选模型时判高峰。

## What I already know

- 用户指定默认映射：
  - 非高峰（default 分支）：`{"default":"glm-5.2","opus":"glm-5.2","sonnet":"glm-4.7","gpt":"glm-5.2","haiku":"glm-4.5"}`
  - 高峰（peak 分支）：`{"default":"glm-4.7","opus":"glm-4.7","sonnet":"glm-4.6","gpt":"glm-4.7","haiku":"glm-4.5"}`
- 现状 models.default = `{"default":"glm-5.2","opus":"glm-5.2","sonnet":"glm-5.2","gpt":"glm-5-turbo","haiku":"glm-4.7"}`（要改 sonnet/gpt/haiku）
- 高峰窗口 `peak_hours[0]` = `{6-10 ×3.0, models:["glm-5.2","glm-5-turbo"]}`（model scope 白名单，**与本 task 的 models 映射分支是两个独立机制**，不冲突）
- `OpenAIResponses` 协议已存在（`protocol.rs:13`，serde `openai_responses`），converter 全链支持（`request.rs:25-26` to_responses / `:60` provider_api_path `/v1/responses`）
- GLM Coding Plan 实际端点（据智谱文档）：
  - openai 兼容：`https://open.bigmodel.cn/api/coding/paas/v4`（已有，chat completions）
  - anthropic 兼容：`https://open.bigmodel.cn/api/anthropic`（已有）
  - **responses**：base_url 待确认（推测 `https://open.bigmodel.cn/api/coding/paas/v4` 同 base，由 `provider_api_path` 切 `/responses`；或独立 `/api/coding/paas/v4` 路径）—— Open Question #1
- pickBranch 现状（`defaults.ts:112`）：`{ default?, coding_plan? }` 两分支，按 codingPlan bool 选。**无 peak 分支**。
- 路由层选模型（`candidates.rs:87-92,256-259`）：`resolve_model(&effective_models, source)`，effective_models 来自 `time_models`（已是按时段切 models 的机制，但数据源 `platform.extra.time_models` 用户级，preset 不带）。本 task 走 models.peak 分支而非 time_models（用户拍板）。
- Rust 侧 PlatformModels 解析（DB schema）：需查是否硬编码 default/coding_plan 两分支，peak 分支需扩。

## Requirements

### R1. endpoints 加 responses 协议端点
- glm_coding.endpoints.default 加第 3 个 endpoint 对象：`{protocol: "openai_responses", base_url: <待确认 GLM responses 路径>, client_type: "codex_tui" 或合适, coding_plan: true}`
- 不破坏现有 2 个 endpoint（openai / anthropic）

### R2. models 加 peak 分支（schema 扩 + 双侧解析）
- preset `models` 字段 schema 扩：`{ default: {...}, peak: {...} }`（仿 coding_plan 分支位）
- preset 填入用户给的两套映射（见上）
- **TS 侧** `pickBranch`（`defaults.ts:112`）扩第三参数支持 peak 分支选择；caller 传「当前是否高峰」bool
  - 「当前是否高峰」判定复用 `isCurrentlyPeak(platform.extra.peak_hours ?? preset.peak_hours, Date.now())`
- **Rust 侧** PlatformModels 解析 + 路由层 `resolve_model` 调用点（`candidates.rs:87,256`）改：命中高峰窗口时用 `models.peak` 替换 `models.default`
- 向后兼容：preset 无 peak 分支 → 回落 default（旧协议不受影响）

### R3. models.default 改映射
- sonnet: glm-5.2 → glm-4.7
- gpt: glm-5-turbo → glm-5.2
- haiku: glm-4.7 → glm-4.5
- default/opus 保持 glm-5.2

### R4. model_list 补 glm-4.5 / glm-4.6 / glm-4.7（若缺）
- 现状 model_list = `[glm-5.2, glm-5-turbo, glm-4.7, glm-5.1, glm-5]`，已含 glm-4.7
- peak 分支引用 glm-4.6 / glm-4.5 → 需补入 model_list（否则前端模型矩阵显示缺）

## Acceptance Criteria

- [ ] preset glm_coding.endpoints.default 含 3 个 endpoint（openai / anthropic / openai_responses），base_url 正确
- [ ] preset glm_coding.models = `{default: <用户给的非高峰>, peak: <用户给的高峰>}`
- [ ] preset glm_coding.model_list 含 glm-4.5 / glm-4.6 / glm-4.7
- [ ] TS pickBranch 支持 peak 分支，caller 传 isPeak
- [ ] Rust PlatformModels 解析支持 peak 分支，路由层命中高峰时切 peak models
- [ ] 非高峰期请求 → 用 default 分支映射；高峰期（6-10 UTC）请求 → 用 peak 分支映射
- [ ] 旧协议（无 peak 分支）→ 回落 default，行为不变（向后兼容）
- [ ] cargo build + cargo test + clippy 零新 warning
- [ ] yarn build 零 error
- [ ] JSON 合法

## Definition of Done

- preset + Rust + TS 三侧改完 + 全绿
- 单测覆盖：pickBranch peak 分支选择 / Rust 高峰切 models / 向后兼容回落
- 不影响其他协议

## Out of Scope

- opencode 数据补全（独立 task `07-11-opencode-preset-models`）
- peak_hours UI 预览行（独立 task `07-11-peak-hours-window-preview`）
- time_models 机制改动（本 task 走 models.peak 分支，不复用 time_models）
- peak_hours 倍率 / model scope 机制（已正确，不动）
- peak_hours 窗口值（6-10 UTC 正确，不动）

## Open Questions

- #1 GLM Responses API 实际 base_url？推测 `https://open.bigmodel.cn/api/coding/paas/v4`（同 openai，由 provider_api_path 切路径）。需查智谱文档或实测确认。默认按推测填，exec 时验证。
- #2 models.peak 分支与 peak_hours.model scope 的交互：高峰期 + 请求模型不在 peak_hours[0].models 白名单 → 倍率走兜底窗口（2.0）但 models 映射仍走 peak 分支？是的（两个机制独立：model scope 决定倍率窗口，models 分支决定映射）。

## Technical Notes

- 改动文件：
  - `src-tauri/defaults/platform-presets.json`（glm_coding section）
  - `src/domains/platforms/defaults.ts`（pickBranch 扩 peak + caller 传 isPeak）
  - `src-tauri/crates/aidog_core/src/gateway/router/candidates.rs`（路由层切 peak models）
  - Rust PlatformModels 解析点（待定位，可能 db.rs 或 models/）
  - 可能 `src-tauri/crates/aidog_core/src/gateway/peak_hours.rs`（导出 is_in_peak_window 已有，复用）
- 依赖：本 task 写 platform-presets.json 与 `07-11-opencode-preset-models` **同文件冲突** → git 串行（DAG 边：opencode → glm_coding）
- spec 相关：`.trellis/spec/backend/protocol-enum-extension.md`（OpenAIResponses 已存在无需扩）；`.trellis/spec/guides/cross-layer-rules.md`（Rust↔TS pickBranch 三参数对齐）
