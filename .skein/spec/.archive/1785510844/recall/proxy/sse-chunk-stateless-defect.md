---
title: SSE 流处理·逐 chunk 无状态解析缺陷
layer: recall
category: proxy
keywords: [SSE,stream,chunk,stateless,silent-data-loss,line-reassembler]
status: active
inclusion: auto
created: 1785503390
related: [stream-buf-unified-cap,hot-path-buffers,rule-50]
---

## 触发场景

流式转发（协议转换分支）中逐 chunk 调用无状态的 SSE 解析函数（如 `adapter::parse_upstream_sse`），当 SSE 事件行被网络 chunk 边界切断时触发。

## 缺陷：逐 chunk 无状态解析导致完整行静默丢失

> 协议转换分支的 chunk 循环逐 chunk 独立调用 `parse_upstream_sse(&text, ...)` 进行帧解析。**该函数对单个 chunk 无状态** —— 一条 SSE 事件行若被 chunk 边界切成两半，前半在 chunk A 尾无结束换行、后半在 chunk B 头无 `data:` 前缀，两半都不构成合法帧，**双双被丢弃**。客户端收到的内容里缺了这一段，且无任何错误信号（静默丢数据）。

典型表现：
- 内容缺失（非乱码、非截断，而是行级丢失）
- 无日志警告
- 客户端与上游内容不符，但无法追踪丢失点

**最易漏的一点**：同一个 chunk 循环里往往有多条消费路径（usage 累计 / 内容转换 / 日志）。本仓 usage 路径早就做了尾行缓冲，内容路径漏了，缺陷因此潜伏很久。**跨 chunk 状态处理必须逐条消费路径核对，禁假定「另一条做了这条也做了」。**

## 正解：尾行缓冲 + 无状态解析分离

### MUST 架构

- **尾行缓冲层**：在无状态解析函数外层加一个有状态的行重组器（如 `SseLineReassembler`）
  - 每个 chunk 喂入后提取完整行（以 `\n` 作分界）
  - 不完整尾行暂存在缓冲里，等下个 chunk 拼接
  - 完整行直接交给无状态解析函数

- **完整行立即下发**（硬约束）：缓冲不能攒批
  - 每次 feed() 后若有完整行，**立刻**交给解析函数
  - 攒批会导致首 token 时延随缓冲深度退化
  - 用例必须验证「完整行 + 残行」输入时完整行立刻出现，不延迟

- **上界与口径统一**（硬约束）：见 [[stream-buf-unified-cap]]
  - 若上游永不发换行，缓冲会无限涨 → 需上界截断（本仓 `SSE_LINE_BUF_MAX_BYTES = 1MB`）
  - 同一代码库内多条流处理路径若都做缓冲，**禁各定一份上界**：一处定义其余处引用

- **流末残留不静默丢**（硬约束）：上游断流时缓冲可能有半行
  - 半行本身不是合法帧，解析必失败
  - 选择「记 warn + 丢弃」而非「无声丢弃」：静默丢正是这类 bug 难以被发现的根因
  - 在 Drop impl 中实现，防止流突然断裂时无迹可循

### 不选别的理由

| 备选 | 否决 |
|---|---|
| 让解析函数内部持状态 | 它被多处调用，塞进可变状态污染所有调用方，并发下还要加锁 |
| 攒够 N 字节再解析 | 首 token 时延退化 |
| 把转换分支也改成 passthrough | 那是删功能，协议转换本就是产品能力 |

## 验收基准

- [ ] 缓冲层与无状态解析完全分离
- [ ] 完整行立即下发，有用例证明（喂「完整行 + 残行」，断言完整行立刻出现）
- [ ] 上界值单处定义，多处引用
- [ ] 有超上界的回归用例（buffer 达上界后 warn + 清空）
- [ ] Drop impl 记 warn（若流末有残行）

### 红用例的证明力判据

复现用例常需一个 helper 模拟生产调用链。判据只有一条：**helper 调的是不是生产的同一批函数、序列是否逐步一致**。

- 一致 → 仍是真实镜像，证明力成立
- 若 helper 重新实现了一份逻辑，或直接调被测实现 → 证明的就不是生产路径的缺陷，证明力失效

本仓实例：`naive_per_chunk_parse` 的 `Utf8ChunkReassembler::feed → SseLineReassembler::feed → parse_upstream_sse` 与 `finish.rs` 转换分支逐步一致，故成立。

## 适用场景

- SSE/Server-Sent-Events 协议转换
- 逐 chunk 处理的流式数据（WebSocket upgrade、HTTP/2 stream 等）
- 任何「解析器对单次 feed 无状态，但数据会跨 feed 边界分割」的场景

## 关联

[[stream-buf-unified-cap]] （上界单一真值源） · [[hot-path-buffers]] （mpsc 队列背压与缓冲） · [[rule-50]] （异步日志队列）
