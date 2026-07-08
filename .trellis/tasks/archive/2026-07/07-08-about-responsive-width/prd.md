# About 页面响应式宽度适配

## Goal

About 页面容器固定 `maxWidth: 720` 无 `width: 100%`（`About.tsx:96`），致：① 宽窗口不撑满（右侧大片空白）② 窄窗口内容溢出（横向滚动）。改为动态适配页面宽度，保留合理上限居中。

## What I already know

- 根因（`src/pages/About.tsx:96`）：
  ```tsx
  <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 720 }}>
  ```
  - 无 `width: "100%"` → 宽窗口容器不撑满（maxWidth 仅设上限，未撑开）
  - 无 `boxSizing: "border-box"` + 无 overflow 处理 → 窄窗口内容（工具卡片/冲突诊断表格/版本字符串）min-width 超容器致横向溢出
- App.tsx 主容器（`App.tsx:159`）已用 `width: "100%"` + padding 12；About 是唯一固定 maxWidth 无 width:100% 的页面
- 其他页面（Stats/Platforms/Settings）无固定 maxWidth，自然撑满
- 用户反馈：窄窗口溢出 + 宽窗口不撑满（两者都要修）

## Decision (ADR-lite)

**Context**: About 容器固定宽度无响应式。
**Decision**:
- 容器加 `width: "100%"` + 保留 `maxWidth: 720`（上限居中）+ `margin: "0 auto"`（居中）+ `boxSizing: "border-box"`
- 内容溢出处（冲突诊断表格/工具卡片/长路径字符串）加 `overflowX: "auto"` / `flexWrap: "wrap"` / `minWidth: 0`（flex 子项防溢出）
**Consequences**: 宽窗口容器撑满至 720 上限居中；窄窗口内容溢出转为容器内滚动/换行，不顶破布局。

## Requirements

1. `About.tsx:96` 根容器：`maxWidth: 720` → `{ width: "100%", maxWidth: 720, margin: "0 auto", boxSizing: "border-box", display: "flex", flexDirection: "column", gap: 20 }`
2. 溢出内容处理（逐处核查 About.tsx 内可能溢出的子元素）：
   - 工具卡片行（`flex` + `space-between`）：确保 `flexWrap: "wrap"` + `minWidth: 0` 子项
   - 冲突诊断表格/路径列表：加 `overflowX: "auto"` 或 `wordBreak: "break-all"`（长路径字符串）
   - 版本号/路径文本：`minWidth: 0` 防 flex 顶撑
3. 不改其他页面（仅 About.tsx）
4. 不引入新依赖 / CSS 模块（inline style 复用现状，About 无 .css 文件）

## Acceptance Criteria

- [ ] 宽窗口（>720px）：About 容器撑满至 720px 上限，水平居中，无右侧大片空白
- [ ] 窄窗口（<720px）：About 容器缩至 100% 宽，内容不横向溢出顶破布局（换行或容器内滚动）
- [ ] 工具卡片/冲突诊断/路径长字符串不顶破容器
- [ ] `yarn build` clean（tsc + vite）
- [ ] `node scripts/check-i18n.mjs` 过（若动文案 key，本 task 预期不动）
- [ ] 主仓零改动（worktree 内改）

## Out of Scope

- 不改其他页面（Settings/Platforms/Stats 等）
- 不加 CSS 模块 / Tailwind / 新样式系统（保持 inline style）
- 不加响应式断点媒体查询（maxWidth:720 + width:100% 已够；若用户后续要断点再议）
- 不改 about-claude-codex-install 的本地环境 section 逻辑（仅宽度适配，section 功能不动）

## Technical Notes

- 真值源 = `src/pages/About.tsx`
- 🔴 **跨 task 文件冲突**: 本 task + about-claude-codex-install（check 中）共改 About.tsx → **必须串行**，等 about-claude-codex-install finish 后 start
- flex 子项防溢出标准模式：父 `minWidth: 0` + 子 `flex: 1` + `minWidth: 0`（flex 默认 min-width: auto 致溢出）
- 长路径字符串：`overflowX: "auto"` 容器内滚动 或 `wordBreak: "break-all"` 换行（按可读性选）
