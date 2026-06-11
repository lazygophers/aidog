# PRD: tray 多平台两行列对齐

## 需求（图示 iStat Menus 式）
多平台同显两行：**第一行所有项标签横排、第二行所有项值横排，列对齐**（标签/值上下对齐成列）。修正之前"多 item 强制单行"的错误限制 —— 菜单栏多 item 两行**可行**。

## 现状（上个 task 的错误限制）
- lib.rs tray_segments：多 item 强制 single 横排（allow_two = len==1）—— **错**，需改
- set_tray_attributed_title：各段 append separator 拼接（单行）

## 决策
- **per-item line_mode 保留**：每项可选单/两行
- **两行项列对齐**：参与列对齐 —— 第一行该列 name、第二行该列 value
- **单行项**：占一列，第一行显 "名 值"，第二行该列留空（tab 占位）
- **列对齐用 NSTextTab**（NSParagraphStyle tabStops），每列一个 tab stop（按列宽估位置）
- 整体一个 attributedString 两行（含 1 个 \n），每行内 \t 分列；per-column 颜色三态/字号（setAttributes:range:）；垂直居中保留

## 渲染模型
- 收集 enabled items 按 order → 每列 (name, value, color, font, line_mode)
- 第一行：各列 [name(两行项) | "名 值"(单行项)]，\t 分隔
- 第二行：各列 [value(两行项) | ""(单行项占位)]，\t 分隔
- 若全部 single 且无两行项 → 退单行横排（separator）
- NSTextTab: tabStops 列位置（等宽字体按最大列字符估 / 固定间距）

## 验收
- 多平台两行列对齐（标签行/值行，如图）
- per-item 单/两行混合（单行项第一行"名 值"）
- per-column 颜色/字号；垂直居中
- cargo test + tsc；GUI 列对齐/渲染用户验

## Subtask
- ST1: lib.rs 渲染重构（两行多列 NSTextTab 列对齐 + per-column color/font + 单行项处理）
- ST2: 验证（cargo test/tsc + 列对齐逻辑 + GUI 用户验说明）
