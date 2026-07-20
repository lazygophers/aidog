# platform 表拆独立平台库 — 调研收敛

## F1. SQL JOIN 已全消除（关键利好）
proxy-log-db-split s3 已把 4 表所有 JOIN 改 Rust 内存 map 合并。grep `JOIN` 命中均为注释（"去 JOIN" / "跨库禁 JOIN" / "替代旧 LEFT JOIN"）。
- `db/group_platform.rs:167/193/240/310/347` — gp 关联单表查 + 批量 platform map 内存重组
- `db/platform.rs:47/60` — LEFT JOIN platform 取名 → 批量 id→Platform map
- `db/stats_today.rs:94/101` — stats_agg_hourly(log.db) ↔ platform(主库) 跨库禁 JOIN，Rust 合并
- `db/settings.rs:140` — 去双 JOIN 内存解析
- `db/proxy_log.rs:399/418` — cli_proxy_provider 跨库禁 JOIN，proxy_log handle 取行后单独取 name
- `db/query_stats.rs:205` — platform_id 已 eff_pid，单表 GROUP BY
**结论**：拆库零 JOIN 重写成本，纯 handle 路由切换。

依据：`src-tauri/crates/aidog_core/src/gateway/db/*.rs`（2026-07-16 grep）

## F2. 主库现状（清理后）
`sqlite_master`：platform / group / group_platform / setting / model_price / middleware_rule / mcp_server / cli_proxy_provider + sqlite_*。
拆后主库剩：setting / model_price / middleware_rule / mcp_server。
依据：`sqlite3 ~/.aidog/aidog.db ".tables"`（2026-07-16，migration 050 DROP proxy_log/stats_agg_hourly 后）

## F3. Db 双槽模式可克隆
`Db` tuple 第 4/5 元 = proxy_log 写槽 + 读池。`open_proxy_log_conn` / `call_proxy_log_traced` / `ReconnectCtx.proxy_log_path` 全套可逐字克隆为 platform 对。
内存库 fallback：`is_memory` 短路 handle = 主内存 conn clone。
依据：`gateway/db/mod.rs:195-407`

## F4. 数据迁移幂等模式（notification 049 已验证）
`init_tables` Phase 1 主库闭包 read 全行 + DROP TABLE → Phase N 目标库闭包 INSERT。
幂等：主库表 DROP 后续启动 read 空 → INSERT for 空转。
依据：`gateway/db/schema.rs::migrate_main_notification_out`（本会话 commit f6ef9f4）

## F5. model_test_results 不存在
原 task desc 误记。主库 `sqlite_master` 无此表，grep 无 `CREATE TABLE model_test_results`。
**已从范围排除**。

## F6. 备份路径
`backup/scheduler.rs` 现 backup aidog.db + log.db（平级）。加 platform.db 同级仿 log.db 加入。
依据：`backup/scheduler.rs` + `db/maintenance.rs:80`（log.db 表走 call_proxy_log_traced 注释）
