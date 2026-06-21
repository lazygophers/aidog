# 让所有日志带 trace/request id（重点：SQL exec 日志 + 后台任务）

> 依赖：必须在 a1c6（删 is_final + Logs 列表 id）落地后启动——都改 db.rs，必须串行。

## 问题（用户实测日志为证）
- ✓ 请求 span 日志带 `request_id`（32hex）+ `trace_id`（8hex）。
- ✓ tauri command span 日志带 `trace_id`。
- ✗ **`DEBUG exec sql sql=...` 行全部无 id**——无法判断某条 SQL 属于哪个请求/操作。
- ✗ 后台定时轮询（平台余额 MIN(created_at)/SUM(est_cost) 那串）无 id（无 span）。

## 根因
SQL 日志来自 `db.rs` 的 rusqlite `conn.trace(Some(sql_trace_callback))`，回调是模块级**裸 fn 指针**，且执行在 **tokio-rusqlite 的单一 DB 后台线程**上，与调用方 tracing span 不在同一线程/任务 → 拿不到 span 字段。tracing 的 span 上下文是 task-local/thread-local，不跨到 DB 线程。

## 目标
每条 SQL exec 日志带上：(1) 发起它的请求/操作 id（request_id 优先，无则 command trace_id）；(2) **调用函数/位置**（哪段 Rust 代码发的 SQL）；(3) **执行耗时**。后台任务也归到一个有标识的 span。
目标格式示例：`exec sql [fn=db::today_stats:1497 req=29ecf7b6... dur=2.1ms] sql=SELECT...`

## 耗时与调用函数（关键技术点）
- **耗时**：现用 rusqlite `conn.trace`（legacy sqlite3_trace）在执行**前**触发，**拿不到耗时**。改用/增设 `conn.profile(Some(cb))`（sqlite3_profile，需 rusqlite `trace` feature，已开），回调签名 `(sql: &str, duration: Duration)`，执行**后**触发，一次拿到 SQL + 耗时。注意：profile 与 trace 二选一或并存（避免重复打印同一 SQL——优先 profile，去掉 trace 打印，只留 profile 输出含耗时）。保留现有字段截断 helper。
- **调用函数/位置**：DB 后台线程无调用栈。复用下方 thread-local：在 db 调用 chokepoint 用 `#[track_caller]` + `std::panic::Location::caller()`（或 `module_path!`/函数名）把调用点 file:line 塞进 thread-local，profile 回调读出拼进日志。

## 候选方案（实现 agent 先调研选型，给理由）
1. **thread-local 透传**（推荐方向）：在 DB 线程设一个 `thread_local! CURRENT_DB_TRACE_ID`。需要一个 db 调用 chokepoint：包一层 `Db::call_traced(id, closure)` 或在现有 `self.0.call()` 外包 helper，进入闭包(在 DB 线程)时 set thread-local = 传入 id，trace 回调读它，闭包结束清空。调用方把当前 span 的 id 传进来。
   - 难点：调用方如何拿「当前 request_id」。可在 ProxyState/请求上下文显式传，或用 tracing 的 `Span::current()` 取字段（tracing 不直接暴露字段值，需自定义机制）。最务实：给热路径 db 调用显式传 id 参数；非请求路径(后台/启动)传 None 或固定标签。
   - tokio-rusqlite 单线程串行执行闭包 → thread-local 不会串味（同一时刻只跑一个闭包）。
2. **task-local + 自定义 connection wrapper**：用 tokio task_local 存 id，但 DB 线程是独立 thread 非该 task，task-local 不传播 → 需手动桥接，等价方案 1。
3. **放弃 rusqlite trace 回调，改在 Rust 侧关键 db helper 内 tracing::debug! SQL**：可天然带 span，但要逐处加、覆盖不全 + 重复造轮子。不推荐。

## 约束 / 注意
- 不破坏既有 SQL 日志的字段截断（log_util）、不破坏 a1c6 改动。
- 后台轮询（平台余额）应包进一个有名 span（如 `balance_poll{trace_id=...}`），让其 SQL 也有归属；或显式传一个 "bg" 标识。
- SQL 日志噪音大：顺带评估是否给后台轮询 SQL 降级到 trace 级 / 或加节流（可选，先问 main 是否要）。
- 性能：thread-local set/clear 每 call 一次，开销可忽略；别引锁竞争。

## 门禁
- `cargo clippy -- -D warnings` 零 warning；`cargo test` 不回归。
- 运行时验证：跑 app 发一个请求，grep 日志确认该请求触发的 `exec sql` 行**带上了对应 request_id**；后台轮询 SQL 带 bg 标识。贴实际日志行。

## 范围确认（启动前问 main）
- 「所有日志」是否包含后台定时任务——是（用户日志里后台轮询无 id 也被点名）。
- 是否同时给后台轮询 SQL 降噪（量很大）——待定。
