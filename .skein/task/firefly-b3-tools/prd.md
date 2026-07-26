# 批3 工具页迁移 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] Logs/Mcp/Skills 3 工具页 + 子目录(ListView/DetailPanel/McpView/SkillsView/McpModals/SkillModals/primitives)萤火虫化
- [ ] 列表项 reveal stagger + hover-lift
- [ ] 详情抽屉(DetailPanel Sheet)萤火虫玻璃签名

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内: src/pages/{Logs,Mcp,Skills}/ 全部子文件 + src/pages/RequestLog.tsx
- [ ] 范围外: ModelTestPanel/SkillDetailView/SkillInstallView(批5)
- [ ] 约束: Logs/DetailPanel Sheet 已是 Radix Portal,核查 createPortal 等价
- [ ] 约束: 5+7 个 portal 弹窗(McpModals/SkillModals)逐个 createPortal 核查
- [ ] 约束: Mcp/Skills facade 外壳(15-26行)不动,改在子目录

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] yarn tsc --noEmit 0 error
- [ ] yarn test 全 pass
- [ ] yarn build 成功
- [ ] ListView 列表 reveal stagger
- [ ] DetailPanel Sheet 玻璃签名 + 流光描边
- [ ] 12 弹窗 createPortal 核查通过

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list firefly-b3-tools`)
