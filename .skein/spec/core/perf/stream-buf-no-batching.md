---
name: stream-buf-no-batching
title: 流缓冲禁留不完整尾巴，完整帧立刻下发
layer: core
category: perf
keywords: [stream,buffer,sse,batching,latency]
created: 1725080438
inclusion: auto
---

流缓冲仅留不完整尾巴（`\r` 无后续 `\n`），完整 SSE 帧立刻下发，禁攒批等待下一条数据。

- `SseLineReassembler` (`gateway/proxy/stream.rs:133`) 逐行 split 立刻下发
- 攒批会累积延迟，对实时应用 LLM 推理响应 p50+ 有损

## 关联

[[stream-buf-unified-cap]]
