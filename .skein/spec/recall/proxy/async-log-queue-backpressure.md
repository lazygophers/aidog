---
layer: recall
created: 1785514813
title: async-log-queue-backpressure
category: proxy
keywords: [async,logging,queue,backpressure,throughput,buffer]
status: active
inclusion: auto
---
layer: recall
created: 1785514813

## 异步日志队列反压

## 触发场景
高频热路径中需要异步写入数据库（如 proxy_log upsert），不能阻塞请求处理；需要保证最终结果不丢且落库顺序保证。

## 陷阱：同步写会阻塞热路径 + 异步不保证持久性

> proxy_log 原先热路径内同步调 `upsert_log(db).await` → 所有请求必须等 DB 写入 → 吞吐量取决于 DB 速度（~10ms/写）。改成纯异步后需解决「背压 + 持久性 + 竞态」。

- ❌ 纯 try_send(queue.full 即丢) — 中间态可丢（UPDATE 会覆盖），但终态丢失 → 统计 / cost / emit 缺失
- ❌ 纯 send().await 全阻塞 — 没有背压分级，队满时热路径全卡住 → 吞吐量没改善

## 正解：方案 B（单 writer + 有界 queue + 分级背压 + 串行快照）

### 架构骨架
```
热路径 (request handler)       后台 writer task
─────────────────────         ───────────────
log.status = 0                spawn_log_writer:
upsert_log(state, log)        - mpsc::Receiver drain
  └─> try_send(Upsert)        - 逐条 process_upsert
       队满→丢（中间态可）     - 单 consumer FIFO 保序
       ✅ 立即返回             - snapshot 生命周期
                              (INSERT→UPDATE→DELETE)

log.status = 200              都走 send().await 阻塞等待
upsert_log(state, log)        背压保证最终结果不丢
  └─> send(Upsert).await      ✅ 落库完成后返回
       队满→阻塞等 writer 腾位
```

### MUST 背压分级（硬约束，关键）

- **中间态（status==0 或 response_body=="[stream]" 占位）**：`try_send()` 满则丢 — 后续 UPDATE 会覆盖整行，中间占位丢失不影响最终数据
  ```rust
  // 中间态日志（流式或未完成）
  if !is_terminal_log(log) {
    let _ = state.log_tx.try_send(msg);  // 丢可接受，终态覆盖
    return;  // 不阻塞热路径
  }
  ```

- **终态（status != 0 且 body != "[stream]"）**：`send().await` 必落 — 保证最终结果 / 统计 / cost / emit 不丢
  ```rust
  // 终态日志（真实 HTTP 结果）
  if state.log_tx.send(msg).await.is_err() {
    tracing::warn!("log writer closed, terminal dropped");
  }
  ```

### MUST 单 consumer FIFO 串行化（硬约束，消除竞态）

- **writer 单独 spawn_log_writer 任务**，唯一 consumer 从 Receiver drain — **禁多 consumer**
  ```rust
  tokio::spawn(async move {
    while let Some(msg) = rx.recv().await {
      match msg {
        LogMsg::Upsert(log, settings) => process_upsert(&db, &log, &settings).await,
        LogMsg::Connect { ... } => process_connect_log(&db, ...).await,
      }
    }
  });
  ```

- **snapshot 读-改-写完全串行于 writer 内** — 消除竞态
  - INSERT 时生成 snapshot row
  - UPDATE 时写当前 snapshot + 新 estimate
  - DELETE 时清 snapshot（离页拦截 + 落库确认后）
  - 同一 consumer 处理保证顺序性

### MUST 队列容量阈值（容纳突发 + 防 OOM）

- `LOG_QUEUE_CAP = 4096` 单位消息 — 极端高并发 (>5000 req/s) 理论偏紧，生产建议加 `log queue depth` metrics 采样 tx capacity 使用率
- 中间态 try_send 失败无日志（预期），终态 send().await 失败才 warn（通常表 writer panic）

## 反例 / 常见错误

| 错误                          | 为什么错                                        | 正确做法                                  |
| ----------------------------- | ----------------------------------------------- | ----------------------------------------- |
| 中间态也用 send().await 阻塞  | 队满时热路径卡住，没有异步收益                  | 中间态 try_send，终态 send().await        |
| 多个 consumer 并发处理 msg    | snapshot 竞态（A 读 INSERT → B 改 UPDATE → A 写，覆盖 B） | 单 spawn_log_writer consumer，禁多并发   |
| snapshot 逻辑分散在热路径+db  | 难跟踪 snapshot 完整生命周期，易漏 DELETE       | 快照逻辑全内聚到 process_upsert           |
| 忽略背压，所有 try_send       | 终态数据丢失 → stats / cost / emit 污染         | 区分中间态/终态，终态必须 send().await   |

## 落库路径升级 checklist

```rust
// 新增高频异步操作时参考此模式：
// 1. 定义枚举消息类型
pub(crate) enum YourMsg {
  Upsert(Box<YourLog>, Settings),
  Delete { id: u64 },
  #[cfg(test)]
  Barrier(tokio::sync::oneshot::Sender<()>),
}

// 2. 背压判定：is_terminal() 分流
fn is_terminal_log(log: &YourLog) -> bool {
  log.final_status != 0  // 状态明确了就是终态
}

// 3. 热路径：中间态 try_send，终态 send().await
pub async fn upsert_async(state: &Arc<YourState>, log: &YourLog) {
  let msg = YourMsg::Upsert(Box::new(log.clone()), ...);
  if is_terminal_log(log) {
    let _ = state.tx.send(msg).await;  // 阻塞保证不丢
  } else {
    let _ = state.tx.try_send(msg);    // 中间态可丢
  }
}

// 4. Writer 单 consumer FIFO 处理
tokio::spawn(async {
  while let Some(msg) = rx.recv().await {
    match msg {
      YourMsg::Upsert(log, _) => { /* 完整序列化逻辑 */ },
      ...
    }
  }
});
```

## 验证

```bash
# 背压分级（中间态 try_send vs 终态 send）
cd src-tauri && grep -n "try_send\|send().await" src/gateway/proxy/log.rs | grep upsert_log

# 单 consumer（唯一 spawn_log_writer）
cd src-tauri && grep -n "spawn_log_writer" src/gateway/proxy/mod.rs  # 1 处 spawn

# snapshot 串行化（process_upsert 内完整生命周期）
cd src-tauri && grep -n "INSERT\|UPDATE\|DELETE" src/gateway/proxy/log.rs | grep -E "process_upsert"
```

## 适用

- proxy_log 异步写入（已实现 s1）
- 其他高频日志 / 统计 / 聚合表的异步更新（future 可参考）

## 关联

[[trellis-11]] （proxy 统计不污染） · [[trellis-00]] （DB 表设计）

## 案例

- log-async-write task (commit 529e571b) — proxy_log 改为单 writer + 有界 mpsc，背压分级保证热路径不阻塞 + 终态不丢（拆库后 writer 需同步 log.db handle 投递）
