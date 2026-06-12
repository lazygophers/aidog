# S1 DB 异步化（tokio-rusqlite）

> Parent: [06-12-p0-p1](../06-12-p0-p1/prd.md) · 基座 subtask，最先执行，独立验证零回归后才解锁 S2。

## Goal

将 `Db(std::sync::Mutex<Connection>)` 替换为 tokio-rusqlite async 连接，解除 rusqlite blocking 对 tokio worker 的阻塞。功能零回归。

## Requirements

- R1.2a Cargo.toml 引入 tokio-rusqlite（版本/feature 见 research），保留 bundled。
- R1.2b db.rs `Db` 重定义为 async（tokio_rusqlite::Connection），全部 `lock()` 点（~56）转 `.call(|c| ...).await`。
- R1.2c 错误类型映射到现有 `Result<_, String>` 契约不变。
- R1.2d lib.rs ~50 Tauri command + proxy.rs + estimate.rs + quota.rs + price_sync.rs 全部 DB 调用方转 async。
- R1.2e open 后设 `busy_timeout`（如 5000ms）+ `synchronous=NORMAL`（WAL 下安全）。

## Acceptance Criteria

- [ ] `cargo build` 0 warning 0 error。
- [ ] DB 操作不再持有 std Mutex 跨 worker；blocking SQL 在 tokio-rusqlite 后台线程执行。
- [ ] 所有 Tauri command 返回值/错误字符串与改造前一致（契约不变）。
- [ ] 内存 DB async 单测覆盖：增删改查 + 事务 + OptionalExtension 路径。
- [ ] 手动冒烟：proxy 转发 + 日志写入 + 统计聚合 + 配额查询行为不变。

## Out of Scope

- proxy_log mpsc 批量 flush（属 S2，本 subtask 只把现有同步写转 async 等价）。
- 任何 schema 变更。

## Technical Notes

- 遵 [backend/db-conventions.md](../../spec/backend/db-conventions.md)：schema/命名/软删除不动。
- 迁移指引: research/tokio-rusqlite-migration.md（research agent 产出）。
- 风险最高 subtask：独立 PR，失败单独回退。
