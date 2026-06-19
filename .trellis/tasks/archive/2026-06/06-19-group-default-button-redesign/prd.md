---
task: 06-19-group-default-button-redesign
title: 默认分组按钮语义化重设计
status: planning
created: 2026-06-19
---

# PRD — 默认分组按钮语义化重设计

## 背景

Groups 卡片头部的「设为默认分组」按钮当前是**星形 SVG 图标按钮**（`src/pages/Groups.tsx:1366-1377`），无文字标签，仅 hover 显 tooltip。星形约定俗成=收藏/书签，与「该分组 config merge 写入 `~/.claude/settings.json` + `~/.codex/config.toml` 成为默认入口」的真实语义无关，用户无法理解。

## 目标

把星形图标按钮替换为**带文字的状态按钮**，语义自明、无需 hover 即可读懂，并清晰传达当前状态（已是默认 / 未默认）。

## 范围边界

- **改**：Groups.tsx:1366-1377 单按钮 JSX（图标 + 文字 + 状态样式）。
- **新增 i18n 键**（8 locale）：`group.defaultConfigWritten`（已是默认态文案）。
- **复用**：现有 `group.setAsDefault`（未默认→幽灵按钮「设为默认」）、`group.isDefaultTitle`（title 兜底说明 merge 写入路径）、`IconHome`（`components/icons.tsx:115`，home 图标表 default 语义）。
- **不改**：`handleToggleDefault` 逻辑（`Groups.tsx:919` 单选 toggle：is_default→null 取消 / 否则设新默认）、`groupApi.setDefault`、`group.is_default` 徽章（`Groups.tsx:1330-1332`，名称旁保留，与按钮并存不冲突——徽章是状态展示，按钮是操作入口）。
- **不涉及**：Platforms 页、后端、router。

## 交付矩阵

| ID | 交付 | 验收 |
| --- | --- | --- |
| D1 | Groups.tsx 星形按钮 → 带文字状态按钮 | 见 R1-R4 |
| D2 | 8 locale 新增 `group.defaultConfigWritten` 键 | check:i18n 零缺失 |

## 需求

### R1 — 未默认态（幽灵按钮）
- `IconHome`（空心，stroke）+ 文字 `group.setAsDefault`（zh「设为默认」）。
- 样式：`btn btn-ghost`，与相邻图标按钮同一行高，文字 `fontSize: 11`。
- 点击 → `handleToggleDefault(group)`（设为默认）。

### R2 — 已是默认态（accent 填充徽章式按钮）
- `IconCheck`（实心 ✓）+ 文字 `group.defaultConfigWritten`（zh「默认配置已写入」）。
- 样式：accent 色填充背景 + accent 文字（`color: var(--accent)` + `background: color-mix(in srgb, var(--accent) 14%, transparent)` + `border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent)`），圆角，padding 与幽灵态一致。
- 点击 → `handleToggleDefault(group)`（取消默认）。
- title 仍挂 `group.isDefaultTitle`（说明 merge 写入路径，保留教育性）。

### R3 — 排他性可见
- 单选语义不变（后端 `groupApi.setDefault` 保证全局唯一默认）。UI 不额外加 radio——状态按钮的「唯一填充态」已隐含排他。

### R4 — 无障碍
- `aria-pressed={group.is_default}`。
- 文字始终可见 → 屏幕阅读器直接读「设为默认」/「默认配置已写入」。

## 设计决策

- **图标选 home（⌂）而非 pin/anchor**：home 在 aidog 全应用已表「默认/主」（IconHome 存在），语义对齐且零新增图标组件。
- **保留名称旁 `group.isDefault` 徽章**：卡片折叠态也能一眼见哪个是默认（按钮在快操作区，徽章在名称行，互补）。
- **不加 radio 点**：用户已否决（选项 A 中选）。

## 单 subtask

S1：Groups.tsx 按钮 JSX + 8 locale 键。单文件 + locale，main 直做（轻量、无并发、无 worktree 需求）。

## 验证门禁

```bash
yarn build         # tsc exhaustive + vite
yarn check:i18n    # 8 locale 零缺失
```

## 自检（start 前）

- [ ] R1-R4 全覆盖。
- [ ] 8 locale 键清单：ar-SA / de-DE / en-US / es-ES / fr-FR / ja-JP / ru-RU / zh-CN。
- [ ] 不动 handleToggleDefault / groupApi / 后端。
