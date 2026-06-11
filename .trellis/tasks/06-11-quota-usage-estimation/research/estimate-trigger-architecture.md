# Research: 预估触发架构（不阻塞响应路径）

- **Query**: proxy upsert_log 后如何异步更新预估，不阻塞请求响应；resolve_price 调用；并发安全
- **Scope**: internal
- **Date**: 2026-06-11

## 现状：proxy 触发点

`handle_proxy`（proxy.rs:127）中拿到 token 的两处：
- **非流式**（proxy.rs:466-495）：`extract_usage` 算出 `(input_tokens, output_tokens, cache_tokens)`（:469）→ 写 log（:474-476）→ `upsert_log`（:487）→ **`return` 响应（:489-494）**
- **流式**（proxy.rs:497-592）：流结束后从原子计数器 load tokens（:578-580）→ `upsert_log`（:581）→ 返回。注意流式 `upsert_log` 在 `Body::from_stream` 构造**之后、return 之前**执行，**但流体实际推送给客户端发生在 return 后**（axum 消费 stream）。即流式分支的 token 在 stream 闭包累加（:550-562），最终 upsert 在 stream builder 之后同步执行——此处 token 计数器此刻可能尚未 load 到最终值。需设计核实流式 token 时序（推测: :578 load 发生在 stream 构造时，可能拿到的是流尚未消费的 0 值——这是既有潜在问题，预估依赖 token 须确认）。

### 可用上下文（预估所需参数都在 scope 内）
- `route.platform.id`（proxy.rs:341 `log.platform_id`）
- `route.platform.platform_type`（proxy.rs:381 `platform_protocol`）— resolve_price 需要的 platform_type 字符串
- `route.platform.base_url` / `api_key`（校准时 quotaApi.query 需要）
- `actual_model`（proxy.rs:323/339）— resolve_price 需要的 model_name（应用实际上游 model）
- `coding_plan` 标记（proxy.rs:326 从 route endpoint 解出）— 判断该平台是 balance 还是 coding plan 预估

## resolve_price 调用（db.rs:1074）
```
resolve_price(db: &Db, model_name: &str, platform_type: &str,
              fallback_input: f64, fallback_output: f64) -> Result<ResolvedPrice, String>
```
- 返回 `ResolvedPrice{input_cost_per_token, output_cost_per_token, cache_read_input_token_cost, source}`（models.rs:762；单位 = 每 token 的金额，db.rs:1134 fallback 把 per-1M 除 1e6）
- 余额预估增量 = `input×input_cost + output×output_cost + cache×cache_read_cost`
- platform_type 字符串：`serde_json::to_string(&platform.platform_type)`（带引号 JSON）——但 resolve_price 内 `pd.get("pricing").get(platform_type)` 期望的 key 格式需核实（pricing JSON 里 key 是 `"openai"` 还是带引号）。看 db.rs:1088 用裸 platform_type 做 key → **传入应是不带引号的协议名**（如 `"openai"`），非 serde_json 序列化串。设计需确认转换。

## 异步架构建议（不阻塞响应）

### 推荐：tokio::spawn 后台任务
- 项目已用 `tokio::spawn`（proxy.rs:51 启服务）。在 upsert_log 后 spawn 一个独立 task 做预估更新，**先 return 响应再后台算**。
- spawn 需 `'static` + Send：clone 出 `state.db`(Arc) / platform_id / platform_type / model / tokens / coding_plan 标记，move 进 task。
- 非流式：在 :487 upsert 后、:489 return 前 spawn（response 已构造好，spawn 不阻塞 return）。
- 流式：token 在 stream 闭包内才确定，**预估 spawn 必须放进 stream 终止逻辑**（流消费完才有真实 token），不能放 :581（那时 token 可能未累加完）。设计需重构流式 token 收尾点。

### 并发安全（多请求同平台）
- `state.db` 是 `Mutex<Connection>`（db.rs:43 `Self(Mutex::new(conn))`），所有写串行化，无数据竞争。
- 但多请求并发预估同一 platform 行存在**读改写竞争**（read est_balance → 减 token → write）：两请求各读旧值各减各写 → 丢更新。
  - 解法：预估写用**单条 SQL 原子自减**：`UPDATE platform SET est_balance_remaining = est_balance_remaining - ?, estimate_count = estimate_count + 1 WHERE id = ?`（在 Mutex 持锁期间原子，无 read-modify-write 间隙）。
  - coding plan utilization 同理用 `est_coding_plan` ... 但 JSON 字段无法 SQL 内自增，需 read-modify-write → 必须整个序列在同一持锁临界区内完成（一次 lock 内 SELECT+UPDATE），或用 application 级 per-platform 锁。设计需裁决 coding plan JSON 的原子更新方式。

## Caveats
- 流式 token 收尾时序（proxy.rs:578 load vs stream 实际消费）是既有疑点，预估依赖 token 准确性，**设计/实现必须先确认流式分支 token 在何处才确定**。
- resolve_price 的 platform_type key 格式（裸串 vs JSON 引号串）需实现时核实 db.rs:1088。
