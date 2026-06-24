# PRD — SQLite 读写分离连接池 (修复代理满载 UI 卡顿)

## 目标
消除 UI 操作在代理高频写日志时的卡顿/无反馈。根因: 当前 SQLite 用单个 `tokio_rusqlite::Connection`(单后台线程 + 单物理连接), 所有 DB 操作(代理写日志 + UI 读/stats 查询)经一条 channel **全串行**, UI 读排在重写之后。引入**读写分离连接池**, 让 UI 读不再阻塞于代理写。

## 根因定位 (已接地)
- `src-tauri/src/gateway/db/mod.rs:173-177`: `pub struct Db(pub AsyncConnection, Arc<DbCache>)`, `AsyncConnection = tokio_rusqlite::Connection`(单后台线程顺序执行所有 call 闭包)。
- `mod.rs:282 call_traced`: 唯一 DB chokepoint, 135 处调用全经此/`self.0.call`。
- `mod.rs:234-237` pragma: 已 `journal_mode=WAL` + `busy_timeout=5000` + `synchronous=NORMAL`。WAL 天然支持「单写 + 多读并发」, 但当前只有一条连接, 没吃到这红利。
- `mod.rs:280` 注释: `call_traced` 的 SQL profile thread-local 上下文**依赖单线程串行**保证 — 引入多连接需保证每连接独立线程(thread-local 天然隔离, 仍成立)。

## 用户决策 (brainstorm 已定)
1. **方案方向**: 读写分离连接池 — 保留 1 条写连接(WAL 仅允许单写), 新增 N 条**只读**连接池供 UI 读/stats 查询并发。
2. **验收口径**: 代理满载(写日志密集)下, Platforms / Logs / Stats 页操作仍秒级响应; 现有 `cargo test` / `clippy` / `yarn build` 全绿。

## 行为规格
- `Db` 结构扩展: 写连接(现有) + 读连接池(N 条只读 `AsyncConnection`, 轮询分发) + 共享 `Arc<DbCache>`(不变)。
- 新增读路径入口(如 `call_read_traced`), 与 `call_traced` 同形(同闭包签名/trace 语义), 内部轮询选一条读连接。
- **读路由**: 纯 SELECT 的 UI 热查询方法路由到读池 — 优先迁 `query_stats.rs` / `stats_today.rs` / `stats_agg.rs` / `usage_stats.rs` 读侧 / group/platform 列表读 / `proxy_log.rs` 日志查询读。**写 / DDL / 含写副作用的方法保持写连接**。
- 读连接: 以 `SQLITE_OPEN_READ_ONLY` 打开, 设同样 `journal_mode=WAL`(读连接看 WAL 需 WAL 模式) + `busy_timeout` + `foreign_keys` + 注册同一 `sql_profile_callback`。
- **DbCache 不变**: 内存缓存 + 写时失效逻辑(`invalidate_*`)仍挂在写路径, 与连接数无关。
- **池大小**: 常量(默认 8), 单点定义可调; 动态扩容(空闲回收/加锁扩容)本轮不做, 留后续。

## 关键约束 / 风险
- 🔴 **`:memory:` 必须 fallback (硬约束)**: 测试大量用 `Db::new(":memory:")`。`:memory:` 下每条物理连接是**独立内存库**, 读池会读到空库。**当 path 为 `:memory:`(或 in-memory) 时, 读池退化为复用写连接**(readers 全 clone 写连接 sender / 或池大小=1 指向写连接), 保证测试与单库语义不破。`file::memory:?cache=shared` 不作为本轮方案(复杂度/平台差异), 直接 fallback 写连接。
- WAL 读快照: 只读连接看到最后已提交快照, UI 读允许微秒级陈旧 — 可接受(本就异步)。
- profile/trace thread-local: 每条 `AsyncConnection` 自带独立后台线程 → thread-local 天然隔离, `CURRENT_DB_CTX` 不串味; 读路径同样 set/clear。
- 读写一致性: 迁到读池的方法必须是**纯读无副作用**; 逐个核对, 有疑问保守留写连接。
- WAL checkpoint / VACUUM(`maintenance.rs`): 锁库期间读连接可能短暂 busy → busy_timeout 兜底, 维持现状。

## 范围边界
- **本轮**: 读写分离连接池基础设施 + 迁移**热 UI 读路径**(stats/query/列表)。不要求一次迁完 135 处(保守: 纯 SELECT 且属 UI 卡顿相关的先迁)。
- 不改 DbCache 语义 / 不改 schema / 不改 pragma 既有值。
- 不引入外部连接池库(r2d2/deadpool)除非必要 — 优先手写轻量轮询池(N 条 tokio_rusqlite::Connection)。

## 验收标准
1. `Db` 含读连接池, 读热路径(stats/query/列表)经只读连接, 写/DDL 经写连接。
2. `:memory:` 路径下读池 fallback 写连接, **全部现有 `cargo test` 通过**(无库不一致回归)。
3. `cargo clippy` 零 warning; `cargo build` 通过; `yarn build` 通过(若涉前端, 一般不涉)。
4. 代理满载场景: 写日志密集时 UI 读查询不被串行阻塞(设计层面成立 + 可选压测/并发测验证读不阻塞于写)。
5. 新增读连接同设 WAL/busy_timeout/foreign_keys/profile, 与写连接 pragma 一致。
