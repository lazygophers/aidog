# Statusline 多行增强 + 每段配色

## Goal

增强 Claude Code statusline 结构化编辑器（`src/components/settings/editors.tsx` 的 StatusLinePanel / segment 系统）：① 更强的多行编排（每行独立管理、任意行数、行内对齐）② 每个 segment 可配真彩 hex 颜色 + 部分段按值语义自动上色，生成 ANSI truecolor 转义。

## What I already know（现状）

- statusline 是结构化 segment 编辑器，数据模型 `StatusLineSegment{id,type,enabled,newline,options}`（editors.tsx:1620）。
- segment 类型：model/context-bar/context-pct/git/cost/rate-limits/effort/vim/separator/custom（SEGMENT_DEFS:1654）。
- **多行现状**：已有 `newline` 布尔（段前插入新行），`generateStatusLineScript`(1854) 按 newline 拆 outputLines、每行 `echo`；modal 有「换行显示」开关(1944)。→ 行数已可变，但无「行」一级实体、无行内对齐。
- **颜色现状**：完全无。SegmentDef 无 color 字段，bash 输出无 ANSI。
- 脚本生成在 TS（generateStatusLineScript），Rust `generate_statusline_script`(lib.rs:1163) 只负责把 content 写到 `~/.aidog/` 并返回路径——**颜色/多行逻辑全在 TS，不需改 Rust**。
- preview：`generatePreview`(1892) 静态 mock；modal 内 `def.toPreview`。
- StatusLinePanel 文案目前**全硬编码中文**（既有 i18n 债，本任务不全量重构，仅新增文案走 t()）。

## Decisions（ADR-lite）

| 维度 | 决策 |
|------|------|
| 多行 | **更强多行**：行(row)成为一级概念——可加/删/重排行、段可在行间移动、每行可设对齐(left/center/right) |
| 颜色粒度 | **每段配色 + 语义色**：每 segment 加 hex 颜色；context-pct/context-bar/cost/rate-limits 等值类段可选「按值自动上色」 |
| 调板 | **真彩 hex**（24-bit），生成 `\033[38;2;R;G;Bm…\033[0m` |

## Requirements

### 多行（R1）
- R1.1 编辑器以「行」分组呈现 segment（基于现有 newline 语义升级为显式 row 视图）：可新增行、删除行、重排行。
- R1.2 段可在行内重排、跨行移动。
- R1.3 每行可设对齐 left（默认）/ center / right——脚本侧用 padding/printf 实现（依赖终端宽度 `.terminal.width` 若 input 提供，否则尽力/降级 left）。
- R1.4 任意行数（≥1）。生成脚本每行独立 echo，保持现有多行 echo 机制。

### 颜色（R2）
- R2.1 `StatusLineSegment` 加 `color?: string`(hex, 如 `#4A9EFF`) + `autoColor?: boolean`。
- R2.2 segment modal 加颜色控件：hex 颜色选择器（input[type=color] + hex 文本）+ 清除/默认；对值类段(context-pct/context-bar/cost/rate-limits)显示「按值自动上色」开关（开启时忽略固定 color，按阈值映射语义色）。
- R2.3 生成脚本：有色段用 `printf '\033[38;2;%d;%d;%dm%s\033[0m' R G B "<text>"` 或等价 echo -e 包裹；无色段保持原样。语义自动色在 bash 内按值算（如 pct>80→红、>60→橙、else 绿；cost 高→橙）。复用项目语义色阈值理念（参考前端 colorScale，但 statusline 是 bash，需内联阈值）。
- R2.4 preview（modal 内 + 面板整体 generatePreview）用真实 hex 渲染颜色（inline style color），语义段按 mock 值上色。

### 通用（R3）
- R3.1 新增 UI 文案走 t()（zh-CN+en-US，其余加 key fallback）；**不**重构既有硬编码（独立 i18n 任务）。
- R3.2 全走 CSS 变量、图标 icons.tsx 禁 emoji（颜色选择器除外，颜色值本身是数据）。
- R3.3 向后兼容：旧 segment 无 color/autoColor 字段 → 默认无色，行为不变。

## Acceptance Criteria

- [ ] 编辑器按行分组展示，可加/删/重排行，段可跨行移动，每行可设对齐。
- [ ] 任意行数生成多行 statusline 脚本，Claude Code 渲染为多行。
- [ ] 每 segment 可设 hex 颜色，生成 ANSI truecolor，终端显色。
- [ ] 值类段可开「按值自动上色」，bash 内按阈值上色。
- [ ] modal + 面板 preview 用真实颜色渲染。
- [ ] 旧配置向后兼容（无 color 字段不报错、不变行为）。
- [ ] 新文案 t()；typecheck 0；明暗双模式正常。

## Definition of Done

- 生成的 bash 脚本语法正确（ANSI 转义、多行 echo、对齐 padding 不破坏 jq 取值）。
- Tauri 契约不变（statuslineApi.generate / generate_statusline_script 签名）。
- 向后兼容旧 segment 配置。typecheck 0。
- 仅改 statusline 相关，不动其他编辑器/页面。

## Out of Scope

- 不改 Rust generate_statusline_script（仅写文件，逻辑在 TS）。
- 不全量重构 StatusLinePanel 既有硬编码中文 i18n（独立任务）。
- 不加非 statusline 功能。
- subagent statusline 复用同一 panel（scriptType 区分），同步受益，无需单独处理。

## Technical Notes

- 核心：`src/components/settings/editors.tsx` 1617-2100+（StatusLineSegment / SEGMENT_DEFS / generateStatusLineScript / generatePreview / SegmentEditModal / StatusLinePanel）。
- ANSI truecolor：`\033[38;2;R;G;Bm` 前景 + `\033[0m` 重置；bash 用 `printf` 或 `echo -e`。
- 对齐依赖 `.terminal.width`（Claude Code statusline input JSON 是否提供需在 toBash 内 `jq` 探测，缺则降级 left）。
- 向后兼容：StatusLineSegment 新字段全 optional。
