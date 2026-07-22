# 前端页面全量迁移至 shadcn — PRD (主入口)

## 目标
- [ ] 全量迁移 18 pages + 40 组件文件 (src/pages/* + src/components/{platforms,settings,shared}/* + 顶层 Sidebar/PopoverCards/UpdatePromptModal/SortableList/App.tsx) 从原生 HTML 元素 + 自定义样式迁到 shadcn primitives
- [ ] 按域拆并行 subtask: settings 域 / platforms 域 / groups 域 / logs 域 / stats 域 / skills 域 / mcp+misc 域 / 顶层(Sidebar+App+PopoverCards)
- [ ] 每域内: 原生 button→Button, input→Input+Field, select→Select/Combobox, textarea→Textarea, 自定义 modal→Dialog/Sheet/AlertDialog, 自定义 dropdown→DropdownMenu, 自定义 tab→Tabs, hr/分割→Separator, 自定义 toast→sonner
- [ ] 保留所有业务逻辑 + Tauri invoke 调用 + i18n key 不变, 仅换 UI 层
## 边界
- 只换 UI 渲染层 (JSX + className), 不改业务逻辑 / Tauri command 调用 / 数据流 / props 契约
- i18n key 沿用 (src/locales/*.json 不改 key, 文案照旧), 仅可能新增 shadcn 组件内固定文案的 i18n (如 Dialog close aria-label)
- 样式从 src/styles/globals.css + popover.css 自定义类迁到 tailwind utility + shadcn token (bg-background/text-muted-foreground 等), 旧自定义 CSS 类迁移后清理
- modal/confirm 必须 createPortal (shadcn Dialog/Sheet 内置 Radix Portal 天然满足, 见 memory modal-window-center-rule), 禁原生 confirm
- 数值格式化仍走 utils/formatters.ts 不重写 (CLAUDE.md 约束)
- 导航仍 App.tsx 本地 state + utils/navGuard.ts 不引入 react-router (CLAUDE.md 约束)
- 不删 GET / GET /proxy 健康端点相关 UI (CLAUDE.md 约束, 若涉)
## 验收标准
- [ ] 18 pages + 40 组件全部迁完, 无残留原生 button/input/select/textarea (grep 验证, 例外: 非 UI 用途如 hidden input 标注保留)
- [ ] 所有 modal/confirm 经 shadcn Dialog/AlertDialog/Sheet (Radix Portal, createPortal 规则满足)
- [ ] 所有 i18n key 仍工作 (yarn check:i18n 过), 8 语言无缺译
- [ ] yarn build (tsc + vite) 过 + yarn test 全过 (18 测试文件含组件测试 PlatformCard/BalanceBar/CompactCard/StatChip/CostTrendChart 无回归)
- [ ] 视觉: 主题切换 (9 style × 4 color × light/dark) 在所有页面生效, Liquid Glass 等形态正确
- [ ] 无 space-x/y / 无手写 z-index overlay / Button 无 isLoading / cn() 用于条件类 (shadcn 规则全守)
- [ ] 手动巡检 (yarn tauri dev): 各页面核心操作 (加平台/改设置/看日志/统计图) 功能正常
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list shadcn-pages`)
