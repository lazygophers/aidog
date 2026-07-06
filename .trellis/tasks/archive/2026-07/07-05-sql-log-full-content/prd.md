# SQL 日志输出完整原始内容而非占位符

## Goal

SQL trace 日志当前经 `truncate_sql_literals` 截断超长字符串字面量（致 body / 大 JSON 字段显示为占位 / 截断），调试时看不到真实值。用户要：SQL 日志输出完整原始内容。

## What I already know

- 写入点：`src-tauri/src/gateway/db/trace.rs::sql_profile_callback`，target=`sql`，`sql=%truncate_sql_literals(sql)`
- 截断器：`src-tauri/src/gateway/log_util.rs::truncate_sql_literals` (line 40)
  - 单引号字面量超过阈值截断
  - 总长 > `SQL_TOTAL_MAX` 末尾加 `…[truncated]`
- 触发：`Connection::profile` 回调（SQL 执行后，含耗时），DB 后台线程经 `call_traced` 设 `CURRENT_DB_CTX`
- 日志级别：`tracing::debug!`，受 `RUST_LOG` / `AppLogSettings.level` 控制

## Decision (ADR-lite)

- **C 放阈值**（用户裁定）：调大 `SQL_LITERAL_MAX` 与 `SQL_TOTAL_MAX`，不做开关、不改默认截断逻辑结构
- 新值：`SQL_LITERAL_MAX = 65536`（64KB，覆盖典型 prompt body）/ `SQL_TOTAL_MAX = 262144`（256KB）
- 仍保留 `…[truncated +N]` 末尾兜底，超 64KB 单字面量 / 256KB 整条仍截断（防日志爆炸，可在用户反馈后再放大）

## Requirements

- SQL trace 日志显示真实字面量值（不再 64 字符截断 / 4KB 总长截断）
- 不破坏 `truncate_sql_literals` 测试（更新断言为新阈值）

## Acceptance Criteria

- [ ] SQL 日志含完整字面量（如 `WHERE body = '<完整 prompt JSON>'`，> 64 字符可见）
- [ ] `truncate_sql_literals` 测试断言更新到新阈值，pass
- [ ] 超阈值（> 64KB / > 256KB）仍 `…[truncated]` 兜底

## Out of Scope

- 改 SQL trace 触发逻辑（仍走 profile 回调）
- 改 trace_id / request_id 关联（已正确）

## Technical Notes

- 改动点：`trace.rs::sql_profile_callback` 的 `sql=` 字段
- 候选实现：
  - A 直接去 `truncate_sql_literals` 调用 → 风险：日志爆炸
  - B 加 DB settings `sql_log_full` 开关，开启时不截断 —— 推荐
  - C 放大阈值常量 —— 折中
