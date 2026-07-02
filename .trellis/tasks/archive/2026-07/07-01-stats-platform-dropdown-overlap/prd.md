# PRD — Stats 平台下拉平台名重叠不可读

> 用户报 (/trellisx-flow + 截图): Stats (使用统计) 页平台下拉的平台名字重叠, 预期应清晰看到平台名。截图: `/Users/luoxin/.claude/image-cache/96a0dd46-757b-40db-a9f7-4555767078d3/1.png`

## 现状 (main 调研)

- 下拉组件 = `SearchableFilter` (`src/pages/Stats.tsx:713`) + `FilterOption` (同文件底部)
- Stats 页 3 个 SearchableFilter 并排: model (width 170, :279)、platform (width 140, :301)、protocol 等
- platform filter options = `platforms.map(p => ({ value, label: p.name }))` (Stats.tsx:196)
- 下拉面板: absolute, `width: Math.max(width, 320)` (即 320px), zIndex 20, flexDirection column, gap 6; 选项列 column gap 2, maxHeight 250 overflowY auto
- FilterOption button: className="input" + 内联 `display:block, width:100%, textAlign:left, padding:11px 14px, lineHeight:1.5, fontSize:15, overflow:hidden, textOverflow:ellipsis, whiteSpace:nowrap`
- trigger button: className="input" + 内联 `width:100%, textAlign:left, overflow:hidden, textOverflow:ellipsis, whiteSpace:nowrap, fontSize:14`
- 全局 `.input` class (`src/styles/globals.css:260`): padding 8px 12px, fontSize 13, 无固定 height, 无 line-height
- globals.css 无 `button {}` reset

## 根因推断 (静态 CSS 看不出, 需 agent 读码 + 运行时确认)

静态逻辑 (padding + lineHeight 1.5 + nowrap ellipsis) 不该重叠。候选根因:
1. **全局 button reset 缺** → browser default button 样式 (border/padding/background) 与 `.input` + 内联叠加, 但不至于文字重叠
2. **trigger button 文字与背景箭头/占位重叠** (若 select.input 的 background-image 箭头应用到 button? button 非 select, 不该)
3. **open 面板选项 line-height 被 `.input` class 的 `transition: all` 或 backdrop-filter 影响渲染** (低概率)
4. **平台名含超长串 + 某容器 width 不足致换行叠到下行** (但 whiteSpace:nowrap 应截断不叠)
5. **open 面板 z-index 20 被同级图表/canvas 覆盖部分**, 视觉上像选项叠在图表文字上 (非选项自身重叠)
6. **FilterOption button 没显式 line-height, 继承链异常** (全局 body/某容器 line-height 极小 → 多行叠)

最可能: #5 (面板被覆盖) 或 #6 (line-height 继承异常)。

## 修复 (待 agent 定位后定)

agent 任务:
1. 读 SearchableFilter + FilterOption + globals.css 全局 button/line-height 样式链
2. 用 chrome devtools 或读码定位实际重叠源 (若需运行时复现, 尝试 `yarn dev` 起 vite 在浏览器看 Stats 页 SearchableFilter; Tauri invoke 会失败但渲染样式可见)
3. 最小修复 (候选):
   - FilterOption button 显式 `lineHeight: 1.5` (已有, 复核继承覆盖) + `minHeight` 兜底
   - open 面板 z-index 提到更高 (如 1000) 避免被图表盖
   - 若 trigger 重叠: 显式 lineHeight + height auto
4. 复跑 `yarn build` 0 err + `check-i18n` 零缺失

## 验收

1. Stats 平台下拉 open, 平台名垂直列表清晰可读, 无重叠
2. trigger button 当前选中平台名清晰显示 (ellipsis 截断可接受, 但不重叠)
3. model / protocol 同类下拉同源组件, 一并验证 (修一处应解全部)
4. `yarn build` + `check-i18n` 全绿

## 非目标

- 不改 SearchableFilter 搜索/过滤逻辑
- 不改 Stats 其他模块

## 风险

- 运行时视觉复现需 dev server (Tauri invoke 失败但样式可见), agent 可能需 main 补 chrome 截图
- 根因未定位前修复方向是候选, agent 需先确认再改

## 调度

- bug fix, 槽位 1/2 (deeplink-share parent), start 占第二槽
- 根因定位 + 修复合一 (单 implement agent)
