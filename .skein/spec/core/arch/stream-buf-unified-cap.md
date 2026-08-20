---
name: stream-buf-unified-cap
title: 流缓冲上界单一真值源原则
layer: core
category: arch
keywords: [buffer,cap,single-source-of-truth,stream,stateful,SSE]
created: 1725080438
inclusion: auto
---

## 硬约则

同一隐患在代码库内多条执行路径出现时，上界值**禁止多处定义**。一处常量定义，其余路径引用 → 防止行为割裂。

### 触发

流式处理有多个入口路径（如 SSE 的 usage 累计路径 + 内容转换路径），都实现有状态缓冲（如尾行缓冲）。若上游数据异常（永不发换行 / 恶意超长行），缓冲无限涨需截断。

### MUST

- **一处实现上界常量** —— 如 `SSE_LINE_BUF_MAX_BYTES = 1MB` 或 `STREAM_BUF_CAP`（`gateway/proxy/stream.rs:16`）
- **其余路径引用此常量** —— 禁硬编码、禁各写一份数值（`:97`/`:153`/`:272` 三处引用）
- **禁止理由** —— 一个截断一个不截断 → 功能割裂 → 问题只修一半

### 验收

- [ ] grep 常量名，仅在一处定义（`const SSE_LINE_BUF_MAX_BYTES = 1MB` 或 `pub const STREAM_BUF_CAP`）
- [ ] 其余引用处为 `if buf.len() > SSE_LINE_BUF_MAX_BYTES { ... }`，禁魔法数字
- [ ] 超上界用例各路径都有（usage 侧 + 内容侧，行为对称）

## 案例

**正例** —— usage 侧定义 `SSE_LINE_BUF_MAX_BYTES = 1MB`，内容侧转换分支引用同一常量

```rust
// gateway/proxy/stream.rs
const SSE_LINE_BUF_MAX_BYTES: usize = 1024 * 1024;

// gateway/proxy/stream.rs - feed_sse_usage
if self.buf.len() > SSE_LINE_BUF_MAX_BYTES {
    warn!(...);
    self.buf.clear();
}

// gateway/proxy/stream.rs - SseLineReassembler
if self.buf.len() > SSE_LINE_BUF_MAX_BYTES {
    warn!(...);
    self.buf.clear();
}
```

**反例** —— 两条路径各定义一份上界

```rust
// ❌ feed_sse_usage
const USAGE_BUF_MAX: usize = 1MB;  // 一处定义

// ❌ SseLineReassembler
const CONTENT_BUF_MAX: usize = 1MB;  // 重复定义 → 后续改错一处

if self.buf.len() > CONTENT_BUF_MAX { ... }
```

## 适用

- 任何多路径并发处理同一数据流的缓冲
- 多个解析器共用一个上界（如 SSE / WebSocket 等流协议）
- 跨模块缓冲（db 池、HTTP 客户端缓冲等）

## 关联

[[stream-buf-no-batching]] [[hot-path-buffers]]
