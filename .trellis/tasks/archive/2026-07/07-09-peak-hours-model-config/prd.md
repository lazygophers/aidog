# 按时段切换模型配置 (peak_hours × models)

## Goal
平台支持「时段→模型配置」映射：不同时段用不同主力模型档（PlatformModels 5 槽）。路由按当前时段 first-match 命中 → 用该时段 models 替换 platform.models 调 resolve_model；未命中 → 用 platform.models 默认。peak_hours 时段窗口作为「快捷导入」入口（复制为时段 rule），用户也可自定义任意窗口。冲突时用户配置优先（数组顺序 first-match，用户项在前）。

## 用户原话
「针对时段（高峰时段只是快捷方式，如果高峰时段和用户配置时段冲突了，以用户配置的时段更高优先级），在某个时段，就用某个时段的模型配置，如果匹配不上就用默认的」

## Requirements (evolving)
- R1 数据模型：`platform.extra.time_models: TimeModelRule[]`
  - TimeModelRule = { windows: PeakWindow[], models: PlatformModels(5档), source?: "user"|"shortcut" }
- R2 Rust 路由改动：`router/selection.rs` resolve_model 前，按当前时段（复用 peak_hours hit 逻辑）匹配 time_models（first-match wins）→ 命中用 rule.models 调 resolve_model；未中 → platform.models default
- R3 前端 UI：formSections 新区块「时段模型配置」（在 ModelsSection 旁）
  - 列表式：每项 = windows 编辑器（复用 PeakWindow UI）+ 5 槽 models 编辑器（复用 ModelsSection 档位输入）
  - 「从 peak_hours 快捷导入」按钮：把 platform.extra.peak_hours windows 复制为新 rule windows（独立可编辑，不联动）
  - 拖拽排序（first-match 优先级；用户手动加默认在前，快捷导入追加）
- R4 确认 modal（删 rule / 快捷导入覆盖）走 createPortal
- R5 i18n 8 语言
- R6 peak_hours 倍率逻辑不动（独立维度：倍率算 cost，time_models 切模型；两维度正交）

## Acceptance Criteria (evolving)
- [ ] extra.time_models 存储读写（Rust serde + 前端 invoke）
- [ ] 路由：peak 时段命中 time_models[0] → 上游收到该 rule 的 resolve_model 输出；off-peak 未中 → platform.models default
- [ ] 冲突：两 rule 同时命中窗口 → 数组前者赢（first-match）
- [ ] 快捷导入：peak_hours windows 复制为新 rule（独立，后续改 peak_hours 不影响已导入 rule）
- [ ] UI：增删改 rule + 排序 + 档位编辑
- [ ] yarn build + cargo clippy + cargo test 绿
- [ ] 8 语言 i18n key 齐

## Out of Scope
- peak_hours 倍率逻辑改动（独立维度，不动）
- 模型池 model_list 按时段切换（本 task 仅切主力档 PlatformModels）
- endpoint 按时段切换
- 跨平台时段切换（本 task 单平台内）
- preset 默认 time_models（用户级 extra only，preset 不带）

## Technical Approach
- 数据：extra JSON blob 加 time_models 字段（与 peak_hours/breaker 同模式，parse_platform_time_models）
- Rust：
  - models/platform.rs: 加 TimeModelRule struct（windows: Vec<PeakWindow>, models: PlatformModels）
  - 新 parse fn（gateway/time_models.rs 或 extra 解析）：parse_platform_time_models(extra) -> Vec<TimeModelRule>
  - router/selection.rs:61 resolve_model 前插时段匹配：`let effective_models = resolve_time_models(&platform.extra, &platform.models, now); resolve_model(&effective_models, source_model)`
- 前端：
  - defaults.ts / api types: TimeModelRule TS 类型
  - formSections 新 TimeModelsSection（复用 PeakWindow editor + ModelsSection 档位输入）
  - PlatformEditForm 挂载
  - usePlatformForm state: time_models + setter

## Open Questions
- 快捷导入 = 复制独立（推荐，可编辑不联动）vs 引用联动？（用户答暗示独立）
- 槽位 = 5 档（对齐 PlatformModels + resolve_model，推荐）vs 9 档（preset 宽松）？
- 冲突优先级 = 数组顺序 first-match（推荐，用户拖序）vs 显式 source 字段 user>shortcut？

## Technical Notes
- PlatformModels: src-tauri/src/gateway/models/platform.rs:35（5 档 Option<String>）
- resolve_model: src-tauri/src/gateway/router/model_mapping.rs:6（请求模型名含槽位名 → 槽位值；不匹配 → default；无 default 透传）
- router 选 model: src-tauri/src/gateway/router/selection.rs:61
- extra JSON blob 解析模式: peak_hours.rs:91 peak_hours_for / parse_platform_peak_hours
- PeakWindow: start_hour/end_hour/multiplier/days_of_week?（复用窗口定义；time_models 不用 multiplier 字段，仅窗口）
- modal-window-center-rule memory

## Decision (ADR-lite) — 用户确认 (2026-07-09)
- **快捷导入语义**: 复制独立（peak_hours windows 复制为新 rule windows，可编辑不联动）
- **冲突优先级**: 数组顺序 first-match wins（用户拖序 = 优先级；用户手动加默认在前 > 快捷导入追加在后）
- **槽位范围**: 5 档（对齐 PlatformModels struct + resolve_model：default/opus/sonnet/haiku/gpt）
- **正交性**: time_models 切主力档（模型选择）；peak_hours 倍率算 cost（价格）。两维度独立，可共存
