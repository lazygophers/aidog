# peak_hours 导入默认配置按钮 (覆盖)

## Goal
平台编辑表单 peak_hours 倍率区块加「导入默认配置」**独立按钮**：点击用 **preset 该协议 peak_hours 默认值** 覆盖用户当前 `platform.extra.peak_hours`（全量替换）。当前 preset 全空 → 按钮禁用 + tooltip 提示，为未来 preset 填充铺路。

## Requirements
- R1 新增 `getDefaultPeakHours(protocol)` (defaults.ts，第 6 个共享 docPromise 函数) 暴露 preset peak_hours 默认。
- R2 PeakHoursSection 加 `protocol` prop +「导入默认配置」按钮（位于 section `action` 槽，紧邻时区切换）。
- R3 按钮态：
  - preset 有默认（非空数组）→ enabled，点击弹确认 modal → 用户确认后 `setWindows(deep copy default)`（**全量替换**，丢弃当前 windows）。
  - preset 无默认（空/absent）→ disabled + tooltip「该平台无默认高峰配置」。
- R4 确认 modal 走 `createPortal(document.body)`（遵 modal-window-center-rule memory + CLAUDE.md liquid glass transform 退化规则），禁原生 confirm。
- R5 i18n 8 语言补 key：`platform.peak_hours_import_default` / `platform.peak_hours_no_default` / `platform.peak_hours_overwrite_confirm`（标题 + 正文 + 确认/取消按钮）。

## Acceptance Criteria
- [ ] `getDefaultPeakHours("deepseek")` 返 preset 内 deepseek.peak_hours（当前 = []，deep copy 防源污染）。
- [ ] preset 有值协议：按钮 enabled，点击 → modal → 确认 → windows 被全量替换为默认（旧 windows 丢弃）；取消 → 不变。
- [ ] preset 无值协议（当前全部）：按钮 disabled，hover 显 tooltip「该平台无默认高峰配置」。
- [ ] modal 居中（createPortal document.body，不受 liquid glass transform 祖先影响）。
- [ ] yarn build 绿；新增/改动 await 链无遗漏（grep 调用点）。
- [ ] 手动：往 src-tauri/defaults/platform-presets.json 临时给某协议加 peak_hours → 该平台按钮可点导入 → 还原。

## Definition of Done
- lint/clippy/yarn build 绿（前端无 lint，Rust 无新增；本 task 纯前端 + TS）。
- 8 语言 i18n key 齐（zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）。
- 改动面 ≤ 3 文件（defaults.ts + formSections.tsx + PlatformEditForm.tsx + i18n 8 locale json）。

## Out of Scope
- preset 填充实际 peak_hours 值（仅建机制，为未来铺路；用户答 Q1 选 A）。
- 模型配置×时段切换（另一 task 07-09-peak-hours-model-config）。
- disable_during_peak 开关（已存在，不动）。
- 合并模式（用户明示「覆盖」= 全量替换，不做 append/merge）。

## Technical Approach
- 真值源：`src-tauri/defaults/platform-presets.json` protocols.<p>.peak_hours（当前全 absent）。
- 前端入口：defaults.ts 新增 `getDefaultPeakHours`（与 getDefaultModels 同模式：loadDoc → entry?.peak_hours ?? []，deep copy 防 mutate 污染源）。
- 按钮落点：formSections.tsx PeakHoursSection `action` 槽（现有时区切换旁），加 props `protocol: Protocol`。
- 确认 modal：createPortal(document.body)，复用 UnsavedModal 风格（若无通用 Modal 组件则就地小弹窗）。
- caller：PlatformEditForm.tsx:203 传 `protocol={protocol}`（已在 scope，line 29）。

## Decision (ADR-lite)
- **Context**: preset peak_hours 当前全空，用户要「导入默认配置」覆盖按钮。
- **Decision**: 默认 = preset 平台默认（对齐 Rust OnceLock + defaults.ts:47 类型），当前空 → 按钮禁用 + tooltip；覆盖 = 全量替换；确认 modal 走 createPortal。
- **Consequences**: 当前 preset 全空 → 按钮对所有平台禁用（即时无可用），但机制就绪，未来 preset 填充某协议即自动启用。无数据丢失风险（disabled 不可点 + 确认 modal 双保险）。

## Technical Notes
- defaults.ts:47 类型已含 `peak_hours?: PeakWindow[]`（无需改类型）。
- PeakWindow 类型 fields: start_hour/end_hour/multiplier/days_of_week?。
- 模式参考：getDefaultModels (line 108-113) 的 deep copy + pickBranch；peak_hours 无 cp 分支（preset 顶层 per-protocol），不走 pickBranch。
- modal-window-center-rule memory: modal 必 createPortal(document.body) 脱离 transform 祖先。
- cross-layer: 前端判定对称已有 isCurrentlyPeak (utils/peakHours.ts) ↔ Rust is_in_peak_window；本 task 不涉及后端。
