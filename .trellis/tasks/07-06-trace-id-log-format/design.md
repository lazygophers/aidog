# Design — trace-id 注入日志格式

## 架构决策

### D1: traceid 取值源改 span scope walk, 弃 thread-local 栈主导

**现状问题**: `logging.rs:7-15` `TRACE_ID_STACK` thread-local。tokio task 跨线程执行, thread-local 栈**不跨 task 继承** → spawn 出的异步分支内栈空 → `current_trace_id()` None → 兜底孤儿 id, 父子脱钩 (用户痛点)。

**改法**: `current_trace_id()` 改读 tracing span scope:
```text
tracing::subscriber::with_default(|subscriber| {
    subscriber.current_span().id() → ctx.span_scope(id) walk up
    → 找到第一个含 SpanTraceId extension 的 span → 返回其 id
})
```
跨 .await + 跨 spawn (spawn 时 instrument) 都自然继承, 不依赖 thread。

**thread-local 栈保留为 fallback**: 仅 span scope 无 id 时用 (启动早期 / 测试无 subscriber 时), 兜底走全局 root id 生成。

### D2: 新 id 生成器

```text
gen_trace_id() -> String  // 6 位 [0-9a-z], 用 rand::rng() 抽样
gen_child_id(parent: &str) -> String  // parent + "." + gen_trace_id()
```

替换现 `new_trace_id()` 的 8-hex (其调用点: `inject_trace_header` 兜底 + 此处 fmt 兜底, 一并改)。

### D3: spawn_traced helper

```text
pub fn spawn_traced<F>(name: &'static str, fut: F) -> JoinHandle<F::Output>
where F: Future + Send + 'static, F::Output: Send + 'static
{
    let parent = current_trace_id().unwrap_or_else(gen_trace_id);
    let child = gen_child_id(&parent);
    let span = tracing::info_span!("spawn", name = name, trace_id = %child);
    tokio::spawn(fut.instrument(span))
}
```

- `name`: 子任务语义标签 (人读, 不进 id)
- 子段 id 自动基于父前缀生成
- 父无 id 时现场 gen root (用户决策)

### D4: 自定义 FormatEvent

实现 `tracing_subscriber::fmt::FormatEvent<S>` for 自定义 struct, 控制字段顺序 + 着色:

```text
<time> <level> <file>:<line> <func> <msg> <traceid>
```

- console: `FormatEvent` impl 内 ANSI 着色 (各字段独立色, 见 PRD 决策表)
- file: 同 impl, ANSI escape 序列跳过 (传 `ansi: bool` 标志)
- traceid 取值: span scope walk → `SpanTraceId` extension → 兜底 `gen_trace_id()` 现场 root

`with_ansi(bool)` 已有机制, 但自定义 FormatEvent 需自行控制 ANSI escape 输出 (检查 `ctx.storage().ansi()` 或传 flag)。

### D5: 调用点改动清单

**tokio::spawn → spawn_traced** (改):
- `gateway/proxy/handler.rs:47` `handle.spawn`
- `gateway/proxy/log.rs:243` `tokio::spawn`
- `gateway/proxy/connect.rs:171` `tokio::spawn`
- `gateway/proxy/connect.rs:265` `tokio::spawn`
- `gateway/proxy/mod.rs:245` `tokio::spawn`
- `gateway/http_client.rs:136` `tokio::spawn`
- `gateway/http_client.rs:155` `tokio::spawn` (上游 axum::serve, 长生命周期 — 评估是否需 traced)
- `gateway/proxy/stream.rs:214` `handle.spawn`
- `gateway/mitm/tls.rs:269` `tokio::spawn`
- `gateway/mitm/tls.rs:276` `tokio::spawn`

**不动**:
- `mitm/ca.rs` Command::spawn (std::process, 非异步 task)
- 测试代码 spawn

**health.rs handle_root**: 包 `info_span!("health", trace_id=...)` + 一行 info log (B 缺口修复)

### D6: health.rs 修复 (B 缺口)

```text
pub async fn handle_root() -> Response {
    let tid = current_trace_id().unwrap_or_else(gen_trace_id);
    let span = tracing::info_span!("health", trace_id = %tid);
    async {
        // 原逻辑 + inject_trace_header
        tracing::info!("health probe");
    }.instrument(span).await
}
```

### D7: id 双轨映射 (proxy 请求顶级 = request_id base36)

**proxy 请求路径** (handler.rs:13): 顶级 trace_id = `request_id`(32-hex proxy_log.id) 的 base36 编码前 6 位。
- 映射方向: header/log 上看到 6 base36 → 反解需查 proxy_log.id (额外索引或暴搜); 但顺向 (proxy_log.id → base36 → grep 日志) 直接, 这才是诊断主路径
- 实现: 取 request_id 32-hex 的低 31 bit → `u32` → `format_radix(36)` pad 6 → `[0-9a-z]{6}`
- 碰撞: 31 bit ≈ 21 亿空间, 单进程同时活跃请求 ≤ 数百, 碰撞概率可忽略; 跨进程也低 (proxy_log.id 时间序 + 随机)

**非 proxy 路径** (命令 / 后台 / 健康端点): 独立随机 `gen_trace_id()` 6 [0-9a-z]。

**异步分支**: 无论父是 proxy 还是 command 路径, 子段统一 `gen_child_id(parent)` = `parent + "." + 6 [0-9a-z]`。

**handler.rs:13 改动**: `trace_id` 字段从 `request_id[..8]` 改为 base36(request_id) 前 6 — 保留 request_id 字段 (32-hex) 不变, 仍入 proxy_log 主键。

## 风险

| 风险 | 缓解 |
|---|---|
| 自定义 FormatEvent + ANSI 控制跨 console/file 复杂 | 抽 `format_event(write, ansi: bool)` 单函数两处调, 禁重复 |
| span scope walk 性能 (每 event walk 链) | span 链通常 ≤3 层, walk O(depth) 可接受; 热路径监控 |
| spawn_traced 漏改某调用点 → 该点 id 脱钩 | grep `tokio::spawn\|handle.spawn` 验收全清 (除 Command::spawn + 测试) |
| thread-local 栈与 span scope 双轨不一致 | span scope 为主, thread-local 仅 fallback, 日志兜底新生 root 不再走栈 |
| tracing-subscriber API 版本差异 (FormatEvent trait 签名) | cargo build 验证, 必要时查 tracing-subscriber 文档 |

## 验收 (PRD Acceptance 对照)

- [ ] debug build: 任意请求 header id grep 日志命中 ≥1 行 (含健康端点)
- [ ] 异步分支: 顶级 id grep 拿到全树 (`父.*` + `父.子.*`)
- [ ] release build 日志格式对称 (ANSI off, 字段顺序一致)
- [ ] 颜色: console 各 level + 字段 ANSI 着色, file 纯文本
- [ ] `cargo clippy --lib` 0 项目警告
- [ ] `cargo test --lib` 全过 + 新增 id 生成 + spawn_traced 单测
- [ ] grep `tokio::spawn\|handle.spawn` 验收清单对账 (除不动项)

## Subtask 拆分建议 (implement.md)

subtask 1 (基础): 新 id 生成器 (gen_trace_id / gen_child_id) + 替换 new_trace_id 调用点 + 单测
subtask 2 (核心): 自定义 FormatEvent + span scope walk 取值 + console/file 双层 + 着色
subtask 3 (异步): spawn_traced helper + 13 调用点改写 + health.rs B 修复 + thread-local fallback 调整

依赖: 1 → 2,3; 2,3 并行 (改 logging.rs 内不同函数, 文件集相交需串行 → 实际 1→2→3 串行更稳)
