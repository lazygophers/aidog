# 浮窗强制深色+全不透明 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 浮窗固定纯深色(不随主体 light/dark 变)、背景 100% 不透明(去玻璃透视)、置顶、失焦自动关。置顶+失焦关已在(app_setup always_on_top + startup Focused(false)→hide + HidesOnDeactivate),本 task 仅补①强制dark ②100%不透明。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: (1) popover.tsx:105 `applyTheme(s.themeMode)`→`applyTheme("dark")` 固定深色; (2) popover.css `.popover-root` 背景改 100% 不透明 var(--bg-floating) 实心 + 去 backdrop-filter(实心后玻璃模糊无效且徒增合成开销)。
- [x] 范围外: 不动置顶/失焦逻辑(已在)、不动 locale、不动主窗主题、不动卡片布局。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [x] popover 恒 dark 配色(主窗切 light 时浮窗仍深); 背景实心不透视; `yarn build` 过。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list popover-dark-opaque`)
