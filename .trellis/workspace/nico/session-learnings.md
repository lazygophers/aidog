# 会话非平凡发现（统计聚合表 + 日志追踪改造）

> 待 `/cortex-context-digest` 正式入 cortex (.wiki)。相关已有记忆：group-stats-aggregation / perf-hotpath-optimization / backend-logging-conventions / streaming-sse-log-aggregation。

1. **SQLite GROUP BY 别名 vs 真实列优先级**（踩坑·启动崩）
   GROUP BY 中真实列名优先于 SELECT 输出别名。回填 SQL `SELECT CASE...AS model ... GROUP BY model` 的 `model` 绑定到 proxy_log 真实列而非 CASE 别名 → 两个 raw model 映射同一 actual_model 时聚合输出同一复合键 → 首次回填即 `UNIQUE constraint failed` 启动 panic。修：`GROUP BY 1,2,3,4` 位置引用。

2. **agg 写入挂 upsert_log 致 8x 重复计数**（架构·踩坑）
   upsert_log 单请求被 40+ callsite 调多次（insert+update+流式 flush），终态 gate 每次为真 → 裸 `+1` 多次。log_snapshot 去重在 `!enabled` gate 之后（关日志无 snapshot）不可复用。修：ProxyState 独立有界 FIFO 去重缓存（HashSet+VecDeque cap 8192），按 request id 首次终态只聚合一次。

3. **统计写入解耦日志开关**（架构决策）
   agg 写入须在 `if !ProxyLogSettings.enabled return` **之前** + 自算 est_cost，才能「关日志也有完整统计」。读取源（today/group/query hourly+daily/hourly_rate）全切 agg；minute/5min 保留 proxy_log 兜底。hourly_rate 不切 agg = 违背无日志统计 + 每次 platform_list N×2 全表扫 proxy_log（circle）。

4. **tokio-rusqlite SQL 日志跨线程追踪**（技巧·踩坑）
   所有 .call 闭包投递单一后台线程，tracing span 不跨线程 → trace/profile 回调无 span 上下文。解：DB 线程 thread-local + 调用方 chokepoint `Db::call_traced` 捕获环境 id（自定义 TraceIdLayer 把 span 的 request_id/trace_id 存 task-local）+ #[track_caller] Location 传入。耗时用 `conn.profile`（执行后给 (sql,Duration)）替代 legacy `conn.trace`（执行前无耗时）。rusqlite 0.32 trace/profile 回调是裸 fn 指针，不能捕获状态。禁固定 reqid（bg/-），兜底 new_trace_id()。

5. **#[track_caller] 在 async fn 是 no-op**（踩坑）
   Location::caller() 拿不到业务调用点。须把 db 公开方法从 `async fn` 改为 `#[track_caller] fn -> impl Future`，入口同步取 Location 再 async move 包体 → SQL 日志 fn= 才指向业务调用点而非 db.rs 内部行。

6. **chrono parse_from_str 格式串**（踩坑）
   解析本地小时桶字符串须用 `%H:%M:%S`，字面 `%H:00:00` 返回 None（需分秒占位构成完整 NaiveDateTime），否则 earliest 兜底 now → rate 虚高 24×。

涉及：db.rs / proxy.rs / logging.rs / lib.rs / migrations/011_stats_agg_hourly.sql。
