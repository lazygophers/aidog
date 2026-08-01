# 转发热路径缓冲与拷贝治理 — 详细设计

## 现状（静态盘点）

### 一、正确性缺陷（红线 2，优先级最高）

`gateway/proxy/finish.rs:274-318`，关键行 **`:279`**：

```rust
let text = String::from_utf8_lossy(&chunk);
```

流式 chunk 边界可能切断一个多字节 UTF-8 字符 → `from_utf8_lossy` 把半个字符替换为 `U+FFFD`。**中文/emoji 内容在流式转发中会静默损坏**。

范围界定：passthrough 分支中继的是原始 `chunk.clone()`，**不受影响**；受影响的是 finish 路径的解析/累积文本（token 计数与落库正文）。

修法：跨 chunk 保留不完整字节序列（残留字节留到下一 chunk 拼接），而非每 chunk 独立 lossy 解码。

### 二、in-flight 无界缓冲（内存主线）

`gateway/proxy/stream.rs:54-55` 的 `upstream_body` / `client_body` **累积期无 cap**：

| 环节 | 位置 |
|---|---|
| cap 常量 | `stream.rs:5` `STREAM_BODY_MAX_BYTES = 16MB` |
| cap **只在消费点生效** | `stream.rs:29-35` `join_stream_body` |
| push 点（**无 cap**） | `finish.rs:275`、`finish.rs:333`、`passthrough.rs:273`、`passthrough.rs:278` |

**「累积期无 cap、落库期才截断」等价于无界** —— 50 路并发各累积一个超大响应，OOM 在到达消费点之前就发生了。这是 memory `symmetric-body-cap` 与 `streaming-snapshot-meta-only` 的同一模式复现。

修法：cap 前移到 **push 点** —— 超过阈值即停止累积（保留已累积部分 + 标记截断），而非累积完再截。

关联：`stream.rs:64` `sse_line_buf`（`:86` push_str，`:94` remainder 全留）同样无界；`stream.rs:9` / `passthrough.rs:192` 的 `cap_nonstream_body` 与 `stream.rs:14` 入口截断是**非流式侧已有**的对称防护，流式侧缺失。

### 三、报文深拷贝（CPU）

| 位置 | 内容 |
|---|---|
| `gateway/adapter/converter/request.rs:80` | `serde_json::from_value(body.clone())` |
| `gateway/proxy/forward.rs:288` | `let mut body = req_value.clone();` |
| `gateway/proxy/log.rs:45-61` | **clone 在 `is_terminal_log` 分支之前** —— 中间态日志（40+ 次/请求）也付全量深拷贝 |

`gateway/models/proxy_log.rs:7-79` 有 8 个大 `String` + `attempts: Vec<ProxyAttempt>`；`gateway/proxy/mod.rs:150-186` `LOG_QUEUE_CAP=4096`，`:158` 注释确认 **40+ 次/请求**。4096 × 该结构体 = 字节维度无界。

修法（memory `high-freq-path-min-diff`：高频路径最小 diff，禁改签名波及 N 调用点）：
- `log.rs:45-61` 把 clone **挪到 `is_terminal_log` 分支之内** —— 单行位置调整，零签名变更
- `LOG_QUEUE_CAP` 从条数改为**字节感知**或直接降数值（先量单条平均字节再定）

### 四、emit 无节流（CPU，驱托盘 5Hz）

`gateway/proxy/log.rs:246-258` 普通路径**已有** gate + 节流；**`:437-443` CONNECT 路径 emit 无 gate 无节流**。下游 `crates/aidog_core/src/tray_render.rs:355-390` 每次 emit 触发 `run_on_main_thread` 重建菜单 + 渲图，最高 5Hz。

修法：CONNECT 路径复用 `:246-258` 的同一 gate + 节流 idiom（不新造机制）。

顺带：`log.rs:41` 中间态队满即丢、`:47` 终态阻塞 send —— 这是既有设计，本 task **不改语义**，只改拷贝时机。

### 五、group_platforms 每请求 SQL

`db/group_platform.rs:188` 无缓存（对照 `:339-359` 同文件内已有缓存 idiom）；调用点 `gateway/router/candidates.rs:128` 在转发热路径。失效基建 `db/mod.rs:911-925` 已存在且被 `group_platform.rs:54,124` 调用 → **复用现成失效钩子，不新建机制**。

## 方案顺序（tracer-bullet）

1. **先建一条就红的复现用例**证明跨 chunk 多字节字符变 `U+FFFD` —— 无复现 = 修了也无法验证
2. 修 `from_utf8_lossy`（红线 2）
3. cap 前移到 push 点（内存主线）
4. `log.rs` clone 挪进分支 + CONNECT emit 补节流（CPU）
5. `group_platforms` 复用现有缓存 + 失效钩子
6. request/forward 的两处 `clone()` —— **先量再改**，若 profile 显示非热点则显式记「已查，无阻断项」

## 为什么不选别的

| 备选 | 否决理由 |
|---|---|
| 流式改 `Bytes` 零拷贝全链路 | 波及 N 个调用点，违反 `high-freq-path-min-diff`；先做低风险项 |
| 关掉中间态日志 | 改变既有可观测性语义，超本 task 边界 |
| 用 `String::from_utf8` + 报错 | 损坏内容变成转发失败，比 U+FFFD 更糟；正确解法是跨 chunk 拼接 |
| 托盘渲染降频到 1Hz | 治标 —— 源头是 emit 无节流，堵源头比降下游便宜 |

## 数据流（验证链路）

```
mock 平台构造含中文/emoji 的流式响应，人为切 chunk 于多字节字符中间
  → 复现用例 red → 修复 → green
  → 50 路并发 mock 流，footprint 采峰值 phys_footprint（in-flight cap 生效）
  → 同一组 mock 请求，改前/改后逐条比对 token 数与 est_cost（必须一致）
  → sample 采转发态 CPU%；托盘 emit 频次计数
```

**红线 1（转发延迟与首 token 时延）**：cap 前移与 clone 挪位均**不增加**热路径工作量，结构上只减不增；仍需在验收中量 TTFT 确认无回归。

## 可能性分支（不进当前方案，仅留痕）

- **`ProxyLog` 拆瘦身结构体（meta-only 入队 + 大字段旁路）** — 触发条件：若 clone 挪进分支后 `LOG_QUEUE_CAP × 单条字节` 仍超预算。同 memory `streaming-snapshot-meta-only` 的手法。代价是队列消费侧需重组，改动面大。
- **全链路 `Bytes` 零拷贝** — 触发条件：若 profile 证明 `body.clone()` 确为转发态 CPU 热点。
- **托盘状态改增量更新（不重建菜单）** — 触发条件：若 emit 节流后托盘仍是 CPU 热点。
