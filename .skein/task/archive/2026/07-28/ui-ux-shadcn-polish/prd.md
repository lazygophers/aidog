# UI/UX shadcn 全量适配 — PRD (主入口)

## 目标
- [ ] 18 页族前端 UI 一致适配 shadcn/ui + 暗色主题 + 响应式
- [ ] 消除 5 处功能性 CSS bug + 自研 toggle/modal/dropdown 残留 + hex/rgba 硬编码色兜底 + legacy class 叠加 + 布局溢出
## 边界
- 范围: 5 页族 7 批修复 (功能CSS bug/toggle→Switch/modal→Dialog/legacy class清/色token化/响应式/裸table→ui/table)
- 非目标: 不改业务逻辑/数据流/API; 不动后端 Rust; 不重设计视觉风格; TrayConfigTab macOS 菜单栏模拟色刻意保留
- 约束: 颜色走已有 token (--color-*/--accent-foreground/--border); modal 必 shadcn Dialog+createPortal; 每族改动限本族文件零跨族重叠
## 验收标准
- [ ] yarn build (tsc && vite build) 通过
- [ ] yarn test 全绿
- [ ] scripts/check-i18n.mjs 无新增裸 key
- [ ] 5 处功能 CSS bug 全修 (var(--danger)/success15/accent20/padding缺px×2)
- [ ] 自研 .toggle div 全站清零改 shadcn Switch (含键盘 a11y)
- [ ] 自研 modal/popover 迁 shadcn Dialog/Popover (TrayConfigTab/ModelTestPanel/ModelsMatrix)
- [ ] var(--x,#hex) 硬编码兜底与 color:#fff 全清 token 化
- [ ] 无跨族文件冲突, git diff 仅 UI 层
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list ui-ux-shadcn-polish`)
