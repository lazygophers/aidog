# 批6 100%覆盖验收 — PRD (主入口)

## 目标
要解决什么 / 用户价值 / 成功长什么样:
- [ ] 全量页面 100% 覆盖核对: 每页至少 1 次 reveal/counter/glow/流光 命中
- [ ] 全量测试 + build + i18n 8 语言 check-i18n 通过
- [ ] dev 启动视觉抽检(截图比对明暗双模)

## 边界
范围内 / 范围外 (非目标) / 已知约束:
- [ ] 范围内: 全 app src/pages + src/components + src/styles
- [ ] 约束: 不改业务逻辑,只核对视觉迁移完整性
- [ ] 约束: 发现遗漏页补迁,不算新范围

## 验收标准
可执行、可核对的完成断言 (逐条):
- [ ] yarn tsc --noEmit 0 error
- [ ] yarn test 全 pass
- [ ] yarn build 成功
- [ ] node scripts/check-i18n.mjs 0 缺译
- [ ] grep 覆盖核对: 每页至少 1 个 reveal/counter/hover-lift/流光类命中(脚本化)
- [ ] 全 app 无裸 #fff/#000 硬编码色(grep 核查走 var)
- [ ] dev 启动无 console error

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list firefly-b6-verify`)
