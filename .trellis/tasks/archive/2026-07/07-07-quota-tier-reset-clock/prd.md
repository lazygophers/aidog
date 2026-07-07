# quota tier 显绝对重置时间

## Goal

PlatformCard coding plan 配额档当前只显相对 countdown (`5h ·3h 17m`), 用户还需**绝对重置时间** (如 `14:30` clock time) 一眼判读何时重置。刷新配额后 countdown 已确认数据层 OK (tier.resetsAt 有值)。

## Requirements

- `src/domains/platforms/health.ts` 新增 `formatResetClock(resetsAt: string | null): string`:
  - null/无效/已过期 → ""
  - 当天 → `HH:mm` (如 "14:30")
  - 非当天 → `M/D HH:mm` (如 "7/8 14:30"), 数字格式跨 locale 通用避免"明天"i18n
  - 用 `toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false })` 取 clock
- `src/components/platforms/PlatformCard.tsx` 两处加绝对时间:
  - 紧凑态 (~line 485-509): 块加第 4 行显 clock, fontSize 8, color text-tertiary, whiteSpace nowrap。例 `⏱ 14:30` 或纯 `14:30`。countdown 行保留 (档名+countdown), clock 独立新行
  - 展开态 (~line 591-615): countdown 行 (IconClock) 旁或下加绝对时间, 显 `3h 17m 后 · 14:30` 或 `重置 14:30`
- 复用 formatResetClock (新 export from health.ts), PlatformCard 已 import health
- 进度条块 maxWidth 适当放宽 (紧凑态 110 → 120) 容纳 clock 行
- 无 resetsAt (countdown 空) 时 clock 行也隐藏 (一致性)
- i18n: 若用 label ("重置"), 加 `platform.resetClockAt` 8 locale; 纯 clock 无 label 则无新 key

## Acceptance Criteria

- [ ] yarn build 过
- [ ] check-i18n 零缺失
- [ ] formatResetClock: 当天 "14:30", 跨天 "7/8 14:30", null ""
- [ ] 紧凑态块显 clock (独立行 或 合并), 刷新后的平台可见
- [ ] 展开态显绝对时间 + countdown
- [ ] 无 resetsAt 平台 clock 不显 (不破坏空态)
- [ ] 不改 countdown 逻辑 (formatResetCountdown 不动)

## Out of Scope

- 改 countdown / formatResetCountdown
- 改进度条 / tier.level 色
- 改后端 estimate / window_start

## Technical Notes

- 现有: formatResetCountdown (health.ts:107) 相对, formatDateTime (utils/formatters.ts:76) 完整 toLocaleString (太长不用)
- tier.resetsAt = ISO string (真查后) | null (预估侧 window_start=0)
- 紧凑态当前 3 行: 进度条 / 主数 / 档名+countdown (PlatformCard.tsx:485-509)
- 展开态: StatChip 同款块 + IconClock countdown 行 (PlatformCard.tsx:591-615)
