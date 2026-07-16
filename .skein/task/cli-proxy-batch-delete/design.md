# 设计 — CLI 代理 Provider 批量操作

## 架构
复用 commands_platform/src/batch.rs 模式 (group-batch-ops s1): 独立 batch command + 原子 SQL + BatchReport。cli_proxy_provider 无关联表 → 比 platform 简单 (无 group_platform 级联清), 直 `IN(?)` 单语句原子。

## 数据流
```
前端 选择模式 → checkbox 多选 ids
  → 3 按钮各开 modal (删除 confirm / models textarea / quota select)
  → invoke batch_*_cli_proxy_*(ids, payload) → BatchReport{applied, skipped}
  → 刷新列表 + 关选择模式 + toast
```

## 后端 (commands_cli_proxy/src/batch.rs 新建)
BatchReport 先提 `aidog_core::gateway::models` (camelCase serde, applied+skipped), commands_platform/batch.rs import 改路径 (1 处), commands_cli_proxy 共用。

3 command (各 `#[tauri::command]` + tracing):
- `batch_delete_cli_proxy_providers(ids: Vec<u64>) -> BatchReport`: `DELETE FROM cli_proxy_provider WHERE id IN(?)`, applied = affected
- `batch_override_cli_proxy_models(ids: Vec<u64>, models: Vec<String>) -> BatchReport`: `UPDATE cli_proxy_provider SET models=? WHERE id IN(?)` (models serde JSON 序列化, 同 row_to_provider 反序列化)
- `batch_set_cli_proxy_quota(ids: Vec<u64>, quota: String) -> BatchReport`: `UPDATE cli_proxy_provider SET quota=? WHERE id IN(?)`

db fn (aidog_core::gateway::db::cli_proxy.rs): 3 个 `batch_*` call_traced, 复用 IN 占位符模式 (idx 递增, 见 memory sql-in-placeholder-idx-increment)。或直接 command 内 inline SQL (参照 batch.rs:36 平台模式 — 平台 batch 在 command 内直写 SQL)。**选 command 内 inline** (与 commands_platform/batch.rs 一致, db 层不加 fn, ponytail)。

models 列读写: schema 是 TEXT 存 JSON Array。UPDATE 需 `serde_json::to_string(&models)`; 验证 row_to_provider 反序列化路径一致。

注册: commands_cli_proxy/src/lib.rs invoke_handler 加 3 command。

## 前端 (src/pages/CliProxy.tsx)
- state: `selectMode: boolean`, `selectedIds: Set<number>`
- 头部「选择」按钮切 selectMode; 切入: 行首显 checkbox + 全选 checkbox (头) + 选中计数; 切出清 selectedIds
- 操作栏 (selectMode 时显, 0 选禁用): 「删除」「改模型」「改余额类型」3 按钮
- 3 modal (createPortal, 复用现有 modalOverlay/btnDanger/btnGhost 样式):
  - 删除: confirm, ≤5 列名称 else「已选 N 个 provider」, 危险按钮
  - 改模型: textarea (同单编辑 modelsText, 换行分割)
  - 改余额类型: select (none/newapi, 同单编辑 quotaTypeOf)
- api: src/services/api/cliProxy.ts (或 platforms 同文件?) 加 3 封装 invoke

## 取舍
- **BatchReport 提 core vs 重定义**: 提 core (干净, 波及 commands_platform 1 行 import 改路径)。重定义虽隔离但语义重复, 反 reuse。
- **command 内 SQL vs db fn**: command 内 inline (与 commands_platform/batch.rs 一致, batch 操作语义集中在 command 层, db 层保持单行 CRUD)。
- **选择模式开关 vs 常驻 checkbox**: 用户选开关 (省常驻空间, 批量操作低频)。

## 技术选型
无新依赖。复用 rusqlite params + IN 占位符 + serde。
