# platform 表拆独立平台库 — PRD (主入口)

## 目标
- 把 `platform` / `group` / `group_platform` / `cli_proxy_provider` 4 表从主库 `aidog.db` 拆到独立 `platform.db`（第 3 个 SQLite 文件库）。
- 动机：连接竞争分流 —— `platform` 频繁 status 更新（每次请求可能更新 auto_disabled_strikes / last_real_query_at / last_error）与主库 `setting`/`middleware_rule`/`model_price`/`mcp_server` 读写在同一写连接 Mutex + 同一 WAL 上竞争。拆库 = 独立 Mutex + 独立 WAL，消除元数据写锁与平台热写互锁。
- 成功长相：4 表物理落 `platform.db`，主库仅留 4 张 app 配置表；所有访问点走 `call_platform_traced`/`call_read_platform_traced`；cargo build/clippy/test 全绿；内存库 fallback 行为不变；存量用户数据无损迁移。

## 边界
**范围内**
- Db 结构加 platform 写槽 + 读池（clone proxy_log 双槽模式）。
- 4 表 DDL 从 `run_migrations_early`/`run_migrations_late` 搬到 `run_migrations_platform_early`/`run_migrations_platform_late`。
- 数据迁移：主库 4 表 read → DROP → platform.db INSERT（同 notification migration 049 幂等模式）。
- 58 个查询/CRUD 访问点（platform.rs/group.rs/group_platform.rs/platform_lifecycle.rs/cli_proxy.rs/settings.rs 部分/import_export 等）改走 platform handle。
- migration 046 CPA 清理跨库调整（cpa_pids 预查改 platform handle）。
- maintenance / retention 按表归属分流（memory: migration-maintenance-by-table-owner）。

**范围外（非目标）**
- 不改任何 SQL 语义 / 不重写去 JOIN 逻辑（proxy-log-db-split s3 已全消除 JOIN，4 表访问已是单表 + Rust 内存合并）。
- 不动 log.db（proxy_log / stats_agg_hourly / notification 留 log.db）。
- 不动主库剩余 4 表（setting / model_price / middleware_rule / mcp_server）。
- 不改 Tauri command 层签名 / 前端 API（纯 Rust 内部 handle 路由）。
- 不做 ATTACH 跨库 JOIN（memory `sqlite-cross-db-no-join` 已记此路坑）。

**已知约束**
- 内存库 fallback：`:memory:` 下 platform handle 复用主内存连接（同 proxy_log idiom），测试无感。
- `auto_vacuum=INCREMENTAL` 仅空库可设；platform.db 首次打开必然空库 → 直接设。
- 跨库 `platform_id` 引用：log.db.proxy_log.platform_id 已是数值游离引用（无 FK），拆库后不变 —— 跨库解析 platform 名继续走 Rust 内存 map（现 idiom）。
- `model_test_results` 表不存在（原 task desc 误记，已排除）。

## 验收标准
- [ ] `~/.aidog/platform.db` 存在，含 4 表；主库 `aidog.db` 仅剩 setting/model_price/middleware_rule/mcp_server + sqlite_*。
- [ ] `call_platform_traced` / `call_read_platform_traced` 接线，所有 4 表访问点零 `call_traced`（主库）残留。
- [ ] 存量用户：主库 4 表数据完整迁入 platform.db（行数一致），主库 4 表 DROP。
- [ ] migration 幂等：二次启动 read 空 Vec（主库表已 DROP）→ INSERT for 空转。
- [ ] 内存库 `cargo test -p aidog_core --lib db::` 全绿（fallback 路径不破）。
- [ ] `cargo clippy -p aidog_core` 无新增告警（历史 98 告警不计）。
- [ ] migration 046 CPA 清理跨库不崩（cpa_pids 从 platform.db 预查）。
- [ ] backup/restore 路径覆盖 platform.db（scheduler.rs 备份逻辑同步）。

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 任务/子任务/调度: task.json（`skein subtask list config-db-split`）
