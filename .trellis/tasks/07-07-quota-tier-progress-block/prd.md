# quota tier 进度条块样式

## Goal

PlatformCard coding plan 配额档 (5h/week/MCP) 当前单行 chip `[27%剩 5h ·26m]` 信息层级糊 (档名 + countdown 灰小字挤一行), 用户难判「限额级别 + 剩余时间」。换进度条块样式, 视觉判读最快。

## 用户确认方案 (mockup)

```
紧凑态 (卡片底部, 每档一块横排):
┌──────────┐ ┌──────────┐
│ ▓▓▓▓░░░░ │ │ ▓▓░░░░░░ │  ← 进度条 (剩余%, tier.level 着色)
│  27% 剩   │ │  18% 剩   │  ← 主数 + 剩 后缀
│ 5h ·26m  │ │ week ·2d │  ← 档名 + 倒计时
└──────────┘ └──────────┘
展开态同款更大版
```

## Requirements

- 改 `src/components/platforms/PlatformCard.tsx` 两处 quota tier 渲染:
  - 紧凑态: line ~470-495 (inline-flex chip)
  - 展开态: line ~571-579 (StatChip + countdown)
- 每档独立块 (flex column): 进度条 (剩余% 宽度) + 主数 (`{remainPct}%剩` 或 mcp `{remaining}/{limit}`) + 档名+倒计时行
- 进度条色源: 沿用现有 `tier.level` (danger/warning/success/neutral → 语义色), **不改色口径** (用户只抱怨布局)
- 倒计时复用 `formatResetCountdown`, 档名复用 `tierLabel`
- mcp_monthly 特例: value 显 `{remaining}/{limit}` (非 %), 进度条仍按 remainPct 宽度
- 不抽独立组件 (YAGNI, 仅两处用), 内联在 PlatformCard
- 不引入新依赖, 不改 BalanceBar (它是余额专用, 硬编码 currency)

## Acceptance Criteria

- [ ] yarn build 过
- [ ] check-i18n 零缺失 (无新 key 则不触发)
- [ ] 平台卡片 (含 coding plan quota 平台) 紧凑态显进度条块, 每档一块
- [ ] 展开态同款 (更大尺寸)
- [ ] 进度条宽度 = remainPct%, 色 = tier.level 语义色
- [ ] 档名 (5h/week/MCP) + 倒计时 (·26m) 清晰可读, 不再挤一行
- [ ] mcp_monthly 显 remaining/limit 文字 + 进度条按 remainPct
- [ ] 无 tier (quota.tiers.length==0) 仍隐藏, 不破坏空态

## Out of Scope

- 改 tier.level 色口径 (pace-based vs remainPct 分档) — 用户未抱怨颜色
- 改 BalanceBar 组件
- 改 statusline (那是 bash 脚本, 另一系统)
- popover 卡片 (PopoverCards.tsx 无 tier 渲染)

## Technical Notes

- 数据源: `quota.tiers: { name, remainPct, utilization, resetsAt, limit, remaining, level }[]` (health.ts:39)
- 紧凑态当前: `PlatformCard.tsx:470-495`
- 展开态当前: `PlatformCard.tsx:571-579` (StatChip)
- 进度条 CSS 模式参考 `BalanceBar.tsx:60-78` (height 4, bg-glass track, levelColor fill)
- tierLabel / formatResetCountdown 已 import (PlatformCard.tsx:11)
