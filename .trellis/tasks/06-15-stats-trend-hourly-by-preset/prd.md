# 请求趋势按 preset 联动 granularity

## 背景
Stats 页 preset 默认 `today` + granularity 默认 `daily` → 当天趋势仅 1 个 bucket，退化为单点无意义。用户要按小时绘制。

## 方案（用户选定：preset 联动，保留手动覆盖）
`src/pages/Stats.tsx`：
1. granularity 默认 `"daily"` → **`"hourly"`**（配合默认 preset=today）。
2. 新增 `changePreset(p)`: `setPreset(p)` + `setGranularity(p === "today" ? "hourly" : "daily")`。
3. preset 按钮 `onClick` 由 `setPreset` → `changePreset`。
4. 保留 granularity `<select>` 手动覆盖能力（用户切完 preset 后仍可手调）。

语义：切 preset 自动给合理粒度（today=24 点 hourly，7d/30d=daily），手动 select 不受限。

## 验证
- `npx tsc --noEmit` 过。
- 视觉：进页 today → 趋势按小时 24 点；切 7d → 按天 7 点；切 30d → 按天 30 点；手动切回 hourly 仍生效。

## 非目标
- 不改后端（hourly 已支持）。
- 不动 i18n key（granularity 选项已有）。
