# 批2 settings 编排+子件迁移 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] Settings.tsx(648行)编排容器 + AppSettings.tsx tab 导航萤火虫化
- [ ] 12 settings 子件组件级重构: editors.tsx(全字段编辑器)/Header/AnchorNav/UnsavedModal/Scheduling/Notification/CodingTools/Mitm/Middleware/CcSwitchImport/Sub2ApiImport/NotificationEventList
- [ ] 表单控件(Input/Select/Checkbox/Radio/Switch/Slider)萤火虫聚焦态 + 保存按钮 ripple

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内: src/pages/{Settings,AppSettings,CodexSettings}.tsx + src/components/settings/*.tsx
- [ ] 范围外: PricingTab/TrayConfigTab/PopoverConfigTab(批4)
- [ ] 约束: editors.tsx 全字段编辑器是核心,只改视觉层不动字段逻辑/令牌F-S 编辑器
- [ ] 约束: UnsavedModal 必须 createPortal(document.body)(memory modal-window-center-rule)
- [ ] 约束: AnchorNav 锚点定位不动,只改激活态视觉

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] yarn tsc --noEmit 0 error
- [ ] yarn test 全 pass
- [ ] yarn build 成功
- [ ] Settings tab 激活态萤火虫色 + 下划线流光
- [ ] editors 输入框聚焦 ring 萤火虫(var(--ring))
- [ ] UnsavedModal createPortal(document.body) 核查
- [ ] 保存/重置按钮 ripple

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list firefly-b2-settings`)
