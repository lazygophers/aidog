# 组件暗色适配缺陷修复 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [x] 审计发现组件颜色 dark/light 适配缺陷 (1 HIGH + 1 MEDIUM + 6 LOW), 修为语义 token 驱动。
- [x] 成功: 无写死 #fff/黑阴影致某模式失效; 颜色随主题双模正确。

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [x] 范围内: HIGH `Sidebar.tsx:377` badge `color:#fff`→`var(--accent-foreground)`; MEDIUM `PopoverCards.tsx:55,57` `var(--status-error,#ff3b30)`/`var(--status-warning,#ff9500)`→`var(--color-danger)`/`var(--color-warning)`(与 line56 green 对齐); LOW 6处硬编码 `rgba(0,0,0,.x)` box/drop-shadow → `var(--shadow-md/lg)` 或 `var(--shadow-color)` 派生 (StatusLinePanel:173,254 / EnvEditor:277 / Sidebar:271 / PlatformListView:209,231 / SkillsView:133)。
- [x] 范围外: 不动确认例外 (TrayConfigTab macOS 模拟色 / 用户自选色数据 / 玻璃 sheen 高光 / shadcn overlay scrim / test 文件); 不改主题 token 定义。
- [x] 约束: bg 用语义 token 时 fg 必配对 foreground token (禁写死); shadow 改后视觉保持轻。

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] Sidebar badge light 下文字可见 (非白压浅底)。
- [ ] PopoverCards red/orange 走 --color-danger/--color-warning。
- [ ] 6 处黑阴影改用 shadow token/派生。
- [ ] `yarn build` 通过。

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list dark-color-fix`)
