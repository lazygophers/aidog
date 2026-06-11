# Research: macOS 菜单栏 tray 可配技术边界（关键）

- **Query**: NSStatusItem button attributedTitle 能配什么？多 item / 颜色 / 字号 / 顺序 / 布局 / 对齐 / 位置
- **Scope**: 内部代码 + objc2/objc2-app-kit/objc2-foundation 本地 crate 源码核实
- **Date**: 2026-06-11

## 现状（代码）

单一 status item，单段 attributedTitle：`src-tauri/src/lib.rs:1322` `set_tray_attributed_title`。
- 拿底层 NSStatusItem：`tray.with_inner_tray_icon(|inner| inner.ns_status_item())`（lib.rs:1330-1334），主线程闭包（AppKit 约束满足）。
- 现用 `NSAttributedString::initWithString_attributes`（单段统一属性，lib.rs:1372），属性 = `{NSFont(menuBarFontOfSize 9pt), NSParagraphStyle(居中+固定行高10pt), NSBaselineOffset(-2)}`。
- 两行靠 `\n` + 段落固定行高实现（lib.rs:1245 `TRAY_TITLE_MULTILINE=true`，1350 line_h=10pt）。

依赖版本（`src-tauri/Cargo.toml:40-42`，Cargo.lock 核实）：
- objc2 `0.6.4`、objc2-foundation `0.3.2`、objc2-app-kit `0.3.2`、tray-icon `0.23.1`（tauri 2 透传）。

## API 能力核实（本地 crate 源码，权威）

### NSMutableAttributedString 多段拼接 — ✓ 可用
`objc2-foundation-0.3.2/src/generated/NSAttributedString.rs`：
- `appendAttributedString(&self, attr_string: &NSAttributedString)` (line 447)
- `addAttribute_value_range(...)` (line 404) — unsafe
- `addAttributes_range(...)` (line 417) — unsafe
- `setAttributes_range(...)` (line 362) — unsafe
- `replaceCharactersInRange_withString` (line 354)、`mutableString` (line 396)

→ **可构造多段、每段不同属性的 attributed string**：用 `NSMutableAttributedString`，逐段 `initWithString_attributes` 后 `appendAttributedString`，或整串后 `addAttribute_value_range` 按 NSRange 上色。

### 每段不同颜色 — ✓ 可用（需开 feature）
`objc2-app-kit-0.3.2/src/generated/NSAttributedString.rs:24` `NSForegroundColorAttributeName`（已用 NSFontAttributeName 同机制）。
- NSColor 构造：`colorWithRed_green_blue_alpha`（NSColor.rs:175）、`systemRedColor/systemGreenColor/systemBlueColor/systemOrangeColor`（551-563）、`labelColor`（414，跟随明暗自适应）。
- **⚠️ 现 Cargo.toml objc2-app-kit features 未含 `NSColor`**（lib.rs 也没 import NSColor）→ 实现需在 `Cargo.toml:42` 加 `"NSColor"` feature。

### 每段不同字号 — ✓ 可用
`NSFontAttributeName` 已用（lib.rs:1360）。每段可设不同 `NSFont::menuBarFontOfSize(x)` 或 systemFont。

### 顺序 — ✓ 可用（= 字符串拼接顺序）
多平台横排 = 单 attributed string 内拼接（"A 12.3 | B 45%"）。顺序由用户配置的 order 决定拼接序。

### 换行 / 布局（单行 vs 两行）— ✓ 受限可用
- 单行拼接（多平台横排）✓
- `\n` 两行 ✓（现已实现，靠固定行高塞进 ~22pt 菜单栏；行数越多越挤，>2 行不现实）
- 段落对齐 `NSTextAlignment::Center/Left/Right`（已用 setAlignment，lib.rs:1349）✓ —— 但单 status item 宽度自适应文字，"对齐"视觉意义有限。

## 「可配 / 不可配」清单

| 维度 | 可配? | 说明 / 实现机制 |
|---|---|---|
| 多平台横排同显 | ✓ | 单 status item 内 NSMutableAttributedString 拼接多段 |
| 每段文字颜色 | ✓ | NSForegroundColorAttributeName + NSColor（需开 NSColor feature） |
| 每段字号 | ✓ | NSFontAttributeName，每段独立 NSFont |
| 每段字重/斜体 | ✓ | NSFont systemFontOfSize_weight / 字体描述符 |
| 段间顺序（排序） | ✓ | 拼接顺序 = 用户 order |
| 段间分隔符（\| / 空格 / 自定义） | ✓ | 拼接时插入分隔字符（可单独配色） |
| 单行 / 两行 | ✓(两行) | `\n` + 固定行高；>2 行不可行（菜单栏高度 ~22pt） |
| 段落对齐 left/center/right | ✓(弱) | NSTextAlignment；单 item 宽度贴合文字，视觉差异小 |
| 背景色（每段） | ✓ | NSBackgroundColorAttributeName（app-kit 已暴露，line 29） |
| 单项开关（启用/隐藏） | ✓ | 拼接时跳过 disabled 项 |
| 图标/emoji 混排 | ✓(部分) | emoji 可直接进字符串；自定义图标需 NSTextAttachment（未验证，复杂） |
| **绝对水平位置（菜单栏左/中/某像素）** | ✗ | 系统管理，app 不能指定 x 坐标；item 落在右侧状态区，相对位置由系统/其他 app/用户⌘拖动决定 |
| **item 之间插其他 app 的图标** | ✗ | 单 app 单 status item（本项目设计），无法跨 app 排布 |
| **跟随系统强制某色不被菜单栏反色** | ⚠️ | 深色/浅色菜单栏下自定义固定色可能对比度差；用 labelColor/systemXColor 可自适应，但"用户指定的固定 hex"在另一主题下可能不可读 |

## 关键结论（给 design / 用户澄清）

1. **"位置"语义必须澄清**：用户说的"位置"技术上只能是
   - ✓ 段间**顺序**（order）
   - ✓ 段落**对齐**（弱效果）
   - ✗ 菜单栏内**绝对位置**（系统管控，做不到）
   → design 应把"位置"收敛为"排序 + （可选）对齐"，不承诺绝对定位。

2. **"颜色"可配但有明暗主题陷阱**：用户配固定 hex 在另一菜单栏主题下可能不可读。建议提供 (a) 预设语义色（systemRed/Green/Orange，自适应）+ (b) 跟随系统（labelColor）+ (c) 自定义 hex（标注"深色菜单栏下可能不清晰"）。

3. **行数上限 2**：单行横排 或 两行；超过不可行。多平台多 → 优先单行横排 + 分隔符，或两行分组。

4. **多平台横排会变长**：菜单栏宽度有限，平台越多文字越长，可能挤占其他 app 图标或被系统截断。需 design 考虑"最多显示 N 项 / 紧凑格式（缩写名）"。

## Caveats / Not Found

- NSTextAttachment 嵌入自定义彩色图标到 attributedString —— API 存在但未在本仓库验证，复杂度高，建议 MVP 不做（emoji 够用）。
- 多色 attributedTitle 在菜单栏真机渲染效果（尤其暗色模式对比度）未 GUI 实测，沿用代码注释惯例"GUI 实际渲染留用户验证"。
- a391ad4d agent 正改 lib.rs tray 区，本文件基于当前可见 HEAD 状态（commit 3c3ef6e）。
