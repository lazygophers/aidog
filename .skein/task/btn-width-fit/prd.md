# 按钮宽度自适应审计 — PRD (主入口)

## 目标
- [ ] 确保所有按钮文字不溢出/超宽/被截断(尤其 8 语言 i18n 长文案如德语/俄语)。**审计已完成**(read-only Explore): base `button.tsx`(`inline-flex whitespace-nowrap` 自撑) + 全局 `.btn`(nowrap) 均健全, 无需动; ~35 个 `size="icon"` 全放 SVG/单字形非 i18n 文本, 清; grid/固定宽容器内 button 全已 ellipsis 或自撑, 清。**唯一真实溢出点**: `src/components/Sidebar.tsx` 固定宽 200px 侧栏内 nav label span `flex:1` 缺 `minWidth:0`+ellipsis, 长译文撑破 200px → `nav` 的 `overflowY:auto` 致 overflow-x 计算为 auto → 横向裁切/滚动条。

## 边界
- [ ] 范围内: 仅改 `src/components/Sidebar.tsx` 三处 label span, 加 `minWidth:0, overflow:"hidden", textOverflow:"ellipsis"` 使长文案在 200px 内省略号截断而非溢出:
  - nav item label span (约 :404, `flex:1 textAlign:start`)
  - section-header button (约 :308, `justifyContent:space-between`)
  - DropdownItem (约 :197, `width:100%`)
- [ ] 范围外: 不动 base button.tsx / .btn(健全); 不动 ModelsMatrixSection.tsx:333 time-window 按钮(by-design ellipsis + title tooltip, 显示 describeWindows 计算串非 i18n, 可接受); 不动其它已 clear 的调用点。不改侧栏宽度 200px 本身。

## 验收标准
- [ ] Sidebar.tsx 三处 label span 加 minWidth:0+ellipsis; 长 DE/RU nav 文案在 200px 内省略号截断不溢出/不出横向滚动条; 短文案显示不变; `yarn build` 过。

## 索引
- [ ] 任务/子任务/调度: task.json (`skein subtask list btn-width-fit`)
