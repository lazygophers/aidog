# 批5 次级页+shared+框架迁移 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] 7 次级页萤火虫化: About(540)/CliProxy(833)/Notifications(128)/ModelTestPanel(192)/RequestLog(328)/SkillDetailView(330)/SkillInstallView(346)
- [ ] 4 次级 shared: FilterDropdown/CopyButton/TestResultBody/loading-button
- [ ] 4 框架组件: Sidebar/PopoverCards/SortableList/UpdatePromptModal + App.tsx 主壳

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内: 上述全部
- [ ] 约束: Sidebar 侧栏导航激活态萤火虫流光(高频可见,优先级最高)
- [ ] 约束: App.tsx 主壳布局不动,只改背景层/侧栏衔接视觉
- [ ] 约束: About.tsx(540行,含版本信息/更新日志)只改卡片视觉
- [ ] 约束: CliProxy.tsx(833行)代理配置表单只改视觉层

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] yarn tsc --noEmit 0 error
- [ ] yarn test 全 pass
- [ ] yarn build 成功
- [ ] Sidebar 激活项萤火虫流光描边 + hover-lift
- [ ] 7 次级页 glass-surface 统一 + reveal
- [ ] 4 次级 shared 萤火虫化

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list firefly-b5-misc`)
