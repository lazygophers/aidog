# 批4 配置页迁移 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] PricingTab(463行)/CodexSettings(263行)/TrayConfigTab(763行)/PopoverConfigTab+子目录(1047行)萤火虫化
- [ ] 原生表单控件(Select/Checkbox/Radio/DatePicker)按 example 自定义组件或主题适配
- [ ] 拖拽布局(PopoverConfigTab 二维布局编辑器)萤火虫拖拽态

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内: src/pages/{PricingTab,CodexSettings,TrayConfigTab,PopoverConfigTab}.tsx + PopoverConfigTab/ 子目录
- [ ] 范围外: Settings 表单(editors 批2)
- [ ] 约束: 原生 input[type=date] 优先用 CSS accent-color + color-scheme 兜底(ponytail, 不引入日期库除非用户要)
- [ ] 约束: PopoverConfigTab 拖拽逻辑(SortableList)不动,只改视觉

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] yarn tsc --noEmit 0 error
- [ ] yarn test 全 pass
- [ ] yarn build 成功
- [ ] 原生 Select/Checkbox/Radio/DatePicker 明暗双模可读
- [ ] PricingTab 价格表萤火虫色阶
- [ ] TrayConfigTab 配置项 hover-lift

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list firefly-b4-config`)
