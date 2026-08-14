# 重做 aidog 中文文档 — PRD (主入口)

> 禁写具体文件路径与代码片段 (会很快过期) —— 例外: prototype 产出的能精确编码决策的片段 (状态机/schema/type shape) 可内联, 且须注明来自 prototype。

## 目标
- [ ] 按已确认 workflow 重建 aidog 中文文档，内容与当前代码一致并符合 Rspress v2
## 边界
- 重建 docs/docs/zh；新增 docs/theme 展示组件与 fixture；新增 command 文档生成/检查脚本；更新 Rspress 品牌和 docs CI；不改英文内容
## User Stories
1. 用户可按任务路径完成安装、配置、接入客户端和首个请求；用户可查阅全部模块与 API；维护者可更新组件、fixture 和生成字典
## 验收标准
- [x] 中文内容全部使用 MDX且五区导航完整；12模块使用无截图HTML演示并覆盖四状态；command 字典由源码生成并校验startup/Rust/TS契约；品牌配置指向aidog；yarn check:docs通过
## 验证方式
- 运行 yarn check:docs；运行 yarn workspace rspress-doc-template build；检查中文区无.md；人工桌面浏览器验收清单
## Testing Decisions
- [ ] 生成器check、断链资源检查、MDX/TS/CSS lint、Rspress生产构建
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein subtask list rebuild-aidog-zh-docs`)
