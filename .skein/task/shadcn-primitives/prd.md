# shadcn 核心 primitives 落地 — PRD (主入口)

## 目标
- [ ] add shadcn 核心 primitives 覆盖现有 445 button/169 input/68 select/19 textarea + 各类弹窗/菜单/反馈组件需求
- [ ] 组件清单 (按现有用法反推): Button Input Textarea Select (含 Combobox 变体) Label Checkbox Switch Tooltip Dialog AlertDialog Sheet DropdownMenu ContextMenu Popover Tabs Badge Card Separator ScrollArea Avatar Progress Skeleton Sonner(toast) Alert Form(Field/FieldGroup) Collapsible Accordion
- [ ] 组件装到 resolvedPaths.ui (components.json 配置), 默认 base/radix 按 nova preset
- [ ] 对需 i18n 或项目特定变体的组件做 wrapper 封装 (src/components/ui/ 直放 shadcn 源码, 业务 wrapper 进 src/components/shared/)
## 边界
- 只 add + 封装组件库本身, 不改业务页面消费点 (归 shadcn-pages)
- wrapper 仅封装 i18n / 默认变体 / 项目约定 (如 modal 必须 createPortal 已由 shadcn Dialog 内置 Radix Portal 解决, 见 memory modal-window-center-rule), 不重写组件逻辑
- iconLibrary 按 nova preset (默认 lucide-react), 替换现有 src/components/icons.tsx 自研图标逐步 (icons 迁移归 shadcn-pages 按需)
- 不引入未使用组件 (YAGNI), 清单按现有原生元素用法反推, 新增按需走后续 task
- dnd-kit (SortableList) / qrcode / react-markdown 等非 shadcn 依赖保留不动
## 验收标准
- [ ] 清单内所有组件 npx shadcn@latest add 成功, 源码落 resolvedPaths.ui
- [ ] 每个组件可 import 且 TypeScript 类型完整 (yarn build 过)
- [ ] wrapper 封装 (如有) 满足: modal 类走 Radix Portal (createPortal 对称规则满足), i18n 文案可注入
- [ ] cn() 用于条件 className, 无手写模板字符串三元 (shadcn 规则)
- [ ] 无 space-x/y (用 gap-*), 无手写 z-index 于 overlay 组件 (shadcn 规则)
- [ ] Button 无 isPending/isLoading, 用 Spinner + disabled 组合
- [ ] 新增 icon 走 data-icon, 无 sizing 类 (shadcn 规则)
- [ ] yarn build + yarn test 过无回归
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list shadcn-primitives`)
