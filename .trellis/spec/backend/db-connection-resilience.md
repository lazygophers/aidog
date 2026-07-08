---
updated: 2026-07-08
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# DB Connection Resilience

何时被读: 改 `Db` 结构 / DB 调用路径（`call_traced` / `call_read_traced`）/ 任何 `tokio_rusqlite::Connection` 句柄持有者
谁读: trellis-implement sub-agent / main
不遵守的代价: 连接偶发死亡 → `?` 直传 → route 落 400 给客户端 / 写入静默失败（sample: request_id=abe076efa8c34e6fb96955faad947c56）

---

## 根因（tokio_rusqlite 0.6.0 已知行为，库层不可改）

- `Connection` 内部 `event_loop`（`tokio-rusqlite-0.6.0/src/lib.rs:402-422`）后台线程跑闭包 `f(&mut conn)`；闭包 panic → 线程 unwind 退出 → `receiver` drop → crossbeam channel **永久关闭**。
- 此后该句柄所有 `.call(...)` 返 `Error::ConnectionClosed`，**永久报废，无自愈机制**。
- 触发条件: 闭包 panic / channel drop / 应用关闭。无 panic 堆栈样本时源头不可精确定位。

## 契约（MUST）

- `call_traced` / `call_read_traced` 检测 `Error::ConnectionClosed` **MUST 自动重连重试 1 次**，禁 `?` 直传上层落用户可见错误（route 400 / 写入失败）。
- **写连接**（`call_traced`）重连: `reopen_write_conn` → 替换 `Arc<Mutex<AsyncConnection>>` 槽位 → 重试。`Mutex` 包裹是槽位可整体替换的前提。
- **读连接**（`call_read_traced`）重连: `pool.pick()` 轮询下一条只读连接 → 重试。读池多连接独立，死一条不影响其他。
- **重试上限 = 1**，禁死循环；非 `ConnectionClosed` 错误透传照常传播；重开失败返回首调错误（禁掩盖其他 DB 错误）。
- **内存库**（`is_memory=true`）**MUST 跳过重连**（重开读空库丢数据），直接透传 `ConnectionClosed`。
- **FnOnce 重取**: 用 `Arc<Mutex<Option<F>>>` cell 让闭包内自取 f；channel 已关闭时闭包未实际运行就被 drop，f 仍在 cell 可重取；闭包已消费 f（线程运行中 panic）cell 空 → 放弃重试透传首调错误。
- 重连发生 **MUST 输出 warn 日志**（含 `caller = file:line`），未来再发可反向定位 panic 源头。

## 验证（可 grep / 可 test）

- `grep -n "ConnectionClosed\|reopen_write_conn\|pool.pick" src-tauri/src/gateway/db/mod.rs` 确认重连分支存在。
- `cargo test -p aidog --lib call_traced_reopens_after_panic_kills_thread call_read_traced_retries_on_dead_pool_slot call_traced_skips_reopen_for_memory_db` 全过。

## 反例（禁）

- 禁在 handler 层才重试 route（只覆盖 route 路径，写连接死亡无法兜底；Db 层统一兜底全覆盖）。
- 禁内存库重连（重开读空库丢全部数据）。
- 禁重试无上限（连接持续死亡时死循环）。
- 禁静默重连无 warn 日志（未来无法反向定位 panic 源）。
