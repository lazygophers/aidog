# 修正文档与源码一致性 — PRD (主入口)

> 禁写具体文件路径与代码片段 (会很快过期) —— 例外: prototype 产出的能精确编码决策的片段 (状态机/schema/type shape) 可内联, 且须注明来自 prototype。

## 目标
- [ ] docs/docs/zh 内容与当前源码、配置、测试、生成脚本一致；删除无法证明或过时描述。
## 边界
- 只改中文文档与必要文档生成/校验配置；不新增产品功能。
## User Stories
1. 用户阅读接入、功能、API、维护页时，看到的是 aidog 当前实现，而不是推测性产品介绍。
## 验收标准
- [ ] Claude Code/Codex 接入页以 AI 平台复制启动命令为主流程；默认端口统一 9890；Local API、Tauri command、设置、日志、分组、平台页均可追溯到源码。
## 验证方式
- 运行 yarn check:docs、docs build；必要时跑生成脚本 check。
## Testing Decisions
- [ ] 文档站构建通过；生成 command 文档 check 通过。
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein subtask list doc-truth-repair`)
