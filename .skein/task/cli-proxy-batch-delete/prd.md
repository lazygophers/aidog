# CLI 代理 Provider 批量操作 — PRD (主入口)

## 目标
CliProxy 页加「选择模式」: 开关切入后行 checkbox 多选, 对选中 provider 执行批量删除 / 批量覆盖 models / 批量改 quota type。每个批量操作独立 modal, 不混编辑。释放逐条编辑的重复劳动 (同分组批量设同模型集 / 批量启停测余额类型)。

## 边界
- **范围内**: 选择模式开关 + checkbox 多选 + 全选 + 选中计数 + 3 批量操作 (删除/覆盖 models/改 quota type) 各独立 modal; 后端 3 batch command (原子 SQL); i18n 8 语言。
- **范围外 (非目标)**:
  - 批量改 status / group_id / name / base_url / api_key / wire_protocol (各 provider 各异或低频, YAGNI)
  - 批量操作跨分组 (选择仅在当前列表视图内, 无跨组聚合)
  - 批量测试余额 (现有逐条测, 批量测留后续)
- **已知约束**:
  - 物理删不可逆 → 批量删除必 confirm modal (列选中项)
  - cli_proxy_provider 无关联表 (group_id 是行内字段), 删 = `DELETE WHERE id IN(?)` 原子, 无级联
  - proxy_log.cli_proxy_provider_id 无外键, 删 provider 后日志留孤儿 id (与单删现状一致)
  - 批量改 models = 完全覆盖 (与单编辑 form 语义一致, 非追加)
  - 批量改 quota = 覆盖整 quota JSON = `{type:value}` (与单编辑存 `JSON.stringify({type})` 一致)

## 验收标准
- [ ] 后端 3 command (`batch_delete_cli_proxy_providers` / `batch_override_cli_proxy_models` / `batch_set_cli_proxy_quota`) 注册 invoke_handler, 各原子 SQL, 返 BatchReport
- [ ] BatchReport 提到 `aidog_core::gateway::models`, commands_platform import 改路径 (波及 1 处), commands_cli_proxy 共用
- [ ] 前端 CliProxy.tsx 选择模式开关: 切入显行 checkbox + 全选 + 选中计数 + 3 操作按钮 (0 选禁用)
- [ ] 3 批量 modal 均 createPortal(document.body), 独立不混编辑
- [ ] 批量删除 confirm modal: ≤5 选中列名称, >5 显「已选 N 个 provider」
- [ ] api/cliProxy.ts 加 batchDelete / batchOverrideModels / batchSetQuota 封装
- [ ] i18n 8 语言 (zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES) key 齐全
- [ ] cargo clippy 过 (0 warning); yarn build 过; check:i18n 过

## 索引
- 详细设计: [design.md](design.md)
- 任务/子任务/调度: task.json (`skein.py subtask list cli-proxy-batch-delete`)
