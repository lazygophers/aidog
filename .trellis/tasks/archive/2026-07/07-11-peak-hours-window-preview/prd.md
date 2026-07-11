# peak_hours 窗口预览行：本地/UTC 时区可读时段

## Goal

peak_hours 编辑表单（`src/pages/platforms/formSections.tsx::PeakHoursSection`）每条窗口目前只渲染裸 number input（起 N / 止 N），用户无法直观看到「这个窗口实际覆盖的一天内时段」，尤其 local 模式下存储的 UTC+0 数值（如 6-10）经 `utcToDisplay` 换算后是 14-18，但 number input 仍只是整数，缺少人类可读的「14:00:00 - 17:59:59」表述，也看不出半开区间 `[start, end)` 的含义。

加一条预览行：把窗口的 `start_hour/start_minute/end_hour/end_minute`（UTC+0 存储）按当前 `tzMode`（local / utc）渲染成可读时段串，体现半开区间（含 start 整点、不含 end 整点 → 终点显示为 end-1 的最后一秒）。

## What I already know

- 真值源：`platform-presets.json` 的 `glm_coding.peak_hours` 实测 = `[{6-10 ×3.0}, {0-24 ×2.0 start_at=北京2026-10-01}]`，**UTC+0 语义正确**（= 北京 14-18，与智谱官方 FAQ「高峰期每日 14:00-18:00 UTC+8」一致）；判定逻辑 Rust `peak_hours.rs` + TS `peakHours.ts` 双侧 UTC+0 对称正确。本 task **不动数据 / 不动判定逻辑**。
- 现有 tzMode 切换：`formSections.tsx:375-395`（`utcToDisplay` / `displayToUtc` / `LOCAL_OFFSET_HOURS`），state 来自 `usePlatformForm.ts:208`（默认 `"local"`）。预览行复用同一 `tzMode`，不引入新 state。
- 半开区间语义：`hit`（`peak_hours.rs:94` + `peakHours.ts:21`）= `[start_min, end_min)`，故终点显示应为 `(end-1):59:59`（含该秒），而非 `end:00:00`。
- i18n：8 locale（zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES），新 key 8 文件全补。已有可复用 key：`platform.timezone_local`（「本地」）、`platform.timezone_utc`（「UTC+0」）。

## Requirements

- 每条窗口卡片内新增一行预览文本，显示该窗口在当前 `tzMode` 下的可读时段串。
- 串格式：`HH:MM:SS - HH:MM:SS（<tz 标签>）`，起点 = `start_hour:start_minute:00`，终点 = `(end_hour:end_minute - 1 秒)`（半开区间 `[start, end)` 的可读化）。
- 时区换算复用 `utcToDisplay`（hour 维度）；minute 维度不随时区变（仅 hour 偏移，minute 透传）。
- 时区标签复用 `platform.timezone_local` / `platform.timezone_utc` key。
- 同一 tzMode 下数值与 number input 显示一致（input 用 `utcToDisplay(start_hour)`，预览也用同一换算）。
- 边界情形显示（默认处理，见 Open Questions #1 确认）：
  - `end == 24`（如 GLM 兜底窗口 0-24）→ 终点 = 23:59:59
  - `start == end`（全天退化）→ 「00:00:00 - 23:59:59（全天）」
  - 跨天 `end < start`（如 22-06）→ 显示带「(次日)」标注
  - minute 精度窗口（`start_minute`/`end_minute` Some）→ 精确到秒显示

## Acceptance Criteria

- [ ] glm_coding 默认窗口导入后（6-10 UTC），local 模式（北京 UTC+8）预览行显「14:00:00 - 17:59:59（本地）」；切 utc 模式显「06:00:00 - 09:59:59（UTC+0）」。
- [ ] 兜底窗口 0-24 预览显「00:00:00 - 23:59:59（<tz>）」（全天）。
- [ ] 跨天窗口（如手填 22-06）预览正确显终点「05:59:59（次日）」。
- [ ] minute 精度窗口（如 01:30-02:45）预览显「01:30:00 - 02:44:59（<tz>）」。
- [ ] 切 tzMode（local ↔ utc）预览实时跟随换算。
- [ ] 8 locale 文件均补新 key（或复用 timezone_local/utc，无新 key 时记录原因）。
- [ ] `yarn build`（tsc + vite）零 error；无 Rust 改动。
- [ ] 预览行不破坏现有 number input 编辑语义（双向绑定不变）。

## Definition of Done

- 代码改完 + locale 补齐 + `yarn build` 绿
- 本地手测 4 种边界（glm 默认 / 全天 / 跨天 / minute 精度）截图或文字确认
- 不动 Rust / preset JSON / 判定逻辑（本 task 仅前端展示层）

## Out of Scope

- 改 peak_hours 存储值 / 改 tzMode 默认值 / 删时区切换器（用户已确认窗口值正确，仅加预览）
- 改 `peak_hours_desc` 通用文案（不变）
- Rust 侧任何改动
- PlatformCard 徽标 / Groups 指示（已用 `isCurrentlyPeak`，与本预览无关）

## Open Questions

- #1 跨天窗口预览表述（「22:00:00 - 05:59:59（次日）」 vs 「22:00:00 - 次日 05:59:59」 vs 简化只显「跨天」标签）—— 待用户拍板。

## Technical Notes

- 改动文件（预估）：
  - `src/pages/platforms/formSections.tsx`（加预览渲染 helper + 插入窗口卡片）
  - `src/locales/*.json` × 8（视情况补 key，或纯复用 `timezone_local/utc` 无新 key）
- 复用：`utcToDisplay` / `clampMinute`（已有，`formSections.tsx:388/49`）
- 不引入新依赖；chrono 不涉及（纯前端 Date 算术或直接 hour 偏移）
