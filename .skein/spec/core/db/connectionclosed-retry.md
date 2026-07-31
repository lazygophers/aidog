---
name: connectionclosed-retry
title: DB ConnectionClosed 必须重连重试
layer: core
category: db
keywords: [db,connection,call_traced,reconnect,pool,rusqlite]
created: 1725080438
inclusion: auto
---

## 根因

`tokio_rusqlite` 0.6.0 特性：`Connection` 后台 event_loop 线程 panic → channel 永久关闭 → 所有 `.call(...)` 返 `Error::ConnectionClosed`，无自愈。

## 硬约则

- `call_traced`/`call_read_traced` 检测 `ConnectionClosed` MUST 自动重连重试 1 次
- **写连接**（`call_traced`）重连：`reopen_write_conn` → 替换 `Arc<Mutex<AsyncConnection>>` 槽位
- **读连接**（`call_read_traced`）重连：`pool.pick()` 轮询下一条只读连接
- **内存库**（`is_memory=true`）MUST 跳过重连，直接透传错误
- 重连发生 MUST 输出 warn 日志（含 `caller = file:line`）

## 验证（file:line）

- `crates/aidog_core/src/gateway/db/mod.rs:526,1031`：重连入口
- `db/mod.rs:89-91`：重连上下文注释
- `db/mod.rs:1027-1046`：`reopen_write_conn` 实现

## 关联

[[crash-safe-db-split]] [[sqlite-read-cache-config]]
