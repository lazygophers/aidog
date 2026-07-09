# platform-presets 全面检修

## Goal

修复 `src-tauri/defaults/platform-presets.json`（60 协议真值源，手维护）长期遗留的三大类问题，并完成 peak_hours 架构升级，使「按模型维度的高峰倍率」（如 GLM 仅 5.2/5-turbo 高峰 3 倍）可表达、可见、生效。当前 peak_hours 只能 per-protocol 时间 + 倍率，无 model scope，导致 GLM 规则被错误地应用到全协议所有模型；同时 glm/glm-coding 被错误合并，非标准 slot 散布。

## What I already know

### 真值源
- `src-tauri/defaults/platform-presets.json`（顶层 `protocols`，60 协议，含 `last_updated`）。手维护，禁机器生成覆盖。Rust 经 `peak_hours.rs::default_peak_hours` `include_str!` + OnceLock 解析。
- 前端 4 async 函数（`src/domains/platforms/defaults.ts`）共享单次 RPC 缓存。

### 现状（已扫描验证）
- **endpoints/models/model_list 无空值**（60 协议全有）；用户反馈「缺失」= 信息不全（model_list 漏模型 / base_url 过时等），需逐协议核对官方文档，非「空字段」。
- **非标准 slot 分布**（19 协议）：codex / gemini / glm / glm_en / kimi / minimax / minimax_en / deepseek / doubao / byteplus / openrouter / siliconflow / siliconflow_en / aihubmix / dmxapi / modelscope / shengsuanyun / novita / nvidia / crazyrouter。含 `fast` / `thinking` / `coder` 等非 `{default,sonnet,opus,haiku,gpt,fable}` 白名单 slot。
- **peak_hours schema 缺 model 维度**：`PeakWindow = {start_hour,end_hour,multiplier,days_of_week?,start_minute?,end_minute?,days_of_month?}`，无 `models` 字段 → GLM「仅 5.2/5-turbo 3 倍」无法精确表达，当前放协议级 = 全模型生效（错）。
- **peak_hours 业务代码已有部分适配**：`stats_today.rs::resolve_multiplier`（统计层）、`router/mod.rs::is_in_peak_window`（disable_during_peak 路由排除）、前端 `formSections.tsx::PeakHoursSection`（编辑 UI）+ `PlatformCard`（高峰徽标）+ `utils/peakHours.ts::isCurrentlyPeak`（对称判定）。缺 model scope 维度的消费。

### glm 三问题
- peak_hours 放协议级（应 per-model）
- glm / glm-coding 合并（CLAUDE.md 2026-07-08 决策：删 cp 分支用 endpoint `coding_plan` flag 去重 UI 双显）
- models.default 含 `fast` slot

### GLM 官方规则（用户原文）
> GLM-5.2/GLM-5-Turbo 作为高阶模型，对标 Claude Opus，调用时将按照「高峰期 3 倍，非高峰期 2 倍」系数消耗额度。（作为限时福利，GLM-5.2/GLM-5-Turbo 将在非高峰期仅作为 1 倍抵扣，持续到 9 月底。）注：「高峰期」为每日的 14:00～18:00（UTC+8）。

即：高峰 3 倍 + 非高峰（福利期到 9 月底）1 倍 + 仅 5.2/5-turbo 受影响。UTC+8 14-18 = UTC+0 06-10。

## Decisions（用户已拍板，2026-07-09）

| # | 决策 | 选项 |
|---|---|---|
| D1 | glm / glm-coding 关系 | **恢复双协议**：新增独立 `glm-coding` 协议条目（自带 peak_hours + 高阶模型），改 CLAUDE.md 决策。endpoint `coding_plan` flag 机制并存。 |
| D2 | peak_hours model scope | **加 `models` 字段**：PeakWindow 增可选 `models: string[]`（absent = 全平台生效）。跨层：Rust `peak_hours.rs` + `estimate.rs` + 前端 `defaults.ts` / `peakHours.ts` / `PeakHoursSection` UI。 |
| D3 | 非标准 slot | **清非标准 slot**：models 只留 `{default,sonnet,opus,haiku,gpt,fable}`。19 协议 JSON 清理 fast/thinking/coder。 |
| D4 | 本轮范围 | **全协议扫**：60 协议 endpoints/models/model_list 逐个核对官方文档补齐 + D1/D2/D3 全做。 |

## Requirements

### R1. peak_hours schema 扩展（跨层）
- `PeakWindow` 增三字段（均 `#[serde(default)]` 向后兼容）：
  - `models: Option<Vec<String>>`（absent/null = 全平台模型生效）
  - `starts_at: Option<i64>`（Unix 秒，absent = 立即可用；`epoch_sec < starts_at` → 窗口未启用跳过）
  - `expires_at: Option<i64>`（Unix 秒，absent = 永久；`epoch_sec ≥ expires_at` → 窗口失效跳过）
- Rust `peak_hours.rs`：`hit` / `resolve_multiplier` / `is_in_peak_window` 判定顺序 = 生效期（starts_at/expires_at）→ 时间 → model 过滤（请求 model ∈ window.models 时才命中，absent 视为命中）。
- 消费链全适配：`estimate.rs::est_cost`（估算层）、`stats_today.rs`（统计层）、`router/mod.rs`（disable_during_peak）、`proxy/handler.rs`（log）。
- 前端 `defaults.ts::PeakWindow` 类型 + `peakHours.ts::hit` 对称 + `PeakHoursSection` UI 暴露 model scope + starts_at/expires_at 编辑入口。
- 跨层对称（[[cross-layer-rules]] guide 强制）。

### R2. glm / glm-coding 双协议
- 新增 `glm-coding` 独立协议条目：
  - client_type / endpoints（base_url）/ models（5.2/5-turbo 等高阶）/ model_list / peak_hours（高峰 3 倍 仅 5.2/5-turbo）。
- `glm`（普通）：移除 peak_hours（普通版无高阶倍率），清理 fast slot。
- 改 CLAUDE.md「coding_plan 分支已删」决策段：glm-coding 独立协议 + endpoint flag 机制并存。
- 前端 constants / `matchPlatform` / 协议列表 / PROTOCOLS 同步。

### R3. 清非标准 slot
- 19 协议 models JSON：删 `fast` / `thinking` / `coder` 等非白名单 slot，保留 `{default,sonnet,opus,haiku,gpt,fable}`。
- 核对每个被删 slot 的模型是否应并入 `default` 或新建标准 slot（如 coder 类是否该 fable/gpt 承载）。

### R4. 全协议数据核对
- 60 协议逐个核对官方文档（research subtask 分批）：
  - endpoints base_url 是否最新
  - model_list 是否漏模型（用户反馈「缺失」核心）
  - models slot 值是否准确
- 缺失/过时项补齐，记录每协议 source URL。

### R5. 非高峰福利期（GLM 9 月底截止，自动切换）
- 用户决策（OQ1）：PeakWindow 加 `starts_at`/`expires_at` 字段，窗口到时间自动启用/失效。
- GLM 配置（design §1.3）：高峰 3 倍窗口（永久）+ 非高峰 2 倍窗口（`starts_at` = 2026-10-01 00:00 UTC+8 = Unix 1759276800）。
- 9-30 前非高峰 2 倍窗口未启用 → 默认 1.0（限时福利 1 倍抵扣）；10-01 起启用 → 非高峰 2 倍。first-match 高峰窗口排前覆盖重叠时段。
- 自动切换，无需手工改 preset。

### R6. UI 可见性（用户「看不到」）
- peak_hours 配置在 PlatformCard / Form 可见当前生效模型 scope + 倍率。
- 高峰徽标显示受影响模型（非全平台时标注「仅 X 模型」）。

## Acceptance Criteria

- [ ] `PeakWindow.models` 字段跨层（Rust serde / TS 类型 / hit 逻辑）对称，absent = 全平台向后兼容。
- [ ] GLM 规则精确表达：仅 glm-5.2 / glm-5-turbo 高峰 3 倍，其他 GLM 模型 1 倍；跨层一致。
- [ ] glm / glm-coding 双协议独立，各自 endpoints/models/peak_hours，UI 无双显冲突。
- [ ] 19 协议无非标准 slot（仅白名单 6 个）。
- [ ] 60 协议 endpoints/models/model_list 核对完成，过时/遗漏项补齐，每协议记 source。
- [ ] `cargo test`（peak_hours/time_models 新增 model scope 用例）+ `cargo clippy` 0 新增 warning + `yarn build` 0 错误 + `check-i18n` 0 缺失。
- [ ] CLAUDE.md 决策段更新（glm-coding 独立 + peak_hours model scope）。

## Definition of Done

- 跨层对称契约（[[cross-layer-rules]]）
- 新增/扩展测试覆盖 model scope hit 逻辑（Rust + 可选 TS）
- platform-presets.json `last_updated` 更新
- CLAUDE.md / spec 同步
- 8 locale i18n key 补齐（R6 新 UI 文案）

## Out of Scope

- peak_hours 时区切换 UI 改造（`peakHoursTz` 已存在，本轮不动）
- price_sync 自动同步 model_list（另 task）
- 福利期自动定时切换（R5 若定硬编码则 in，自动切换 out）

## Open Questions

- OQ1: R5 福利期 9 月底截止如何处理？（硬编码截止日期 / 仅当前数据态注释提醒 / 写 journal 定期手工改）
- OQ2: 全协议核对工作量大（60 协议），是否分批优先级？（先头部协议 anthropic/openai/gemini/glm/kimi/deepseek，再长尾聚合站）
- OQ3: glm-coding endpoints base_url（是 GLM 的 `/api/paas/v4` 同源仅模型不同，还是独立 coding 域名）？需查 GLM coding plan 文档。
- OQ4: coder slot 删除后，原 coder 映射的模型（如 minimax coder→MiniMax-M2.5）是否该并入某标准 slot 还是直接弃？

## Technical Notes

- 跨层 guide：`.trellis/spec/guides/cross-layer-rules.md`（PeakWindow 改必同步）
- 前端约定：`.trellis/spec/frontend/conventions.md`
- platform-presets.json schema 见 CLAUDE.md「平台默认配置」段
- peak_hours.rs OnceLock 解析（改 JSON 即生效，Rust 改 schema 需重编译）
- 现有 peak_hours 适配点：`stats_today.rs:230` / `router/mod.rs:73,81` / `formSections.tsx:430`
