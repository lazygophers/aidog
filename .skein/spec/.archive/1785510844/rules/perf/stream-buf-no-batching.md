---
title: stream-buf-no-batching
category: perf
keywords: []
status: active
inclusion: auto
---

## 流缓冲不得攒批原则

---
title: 流缓冲不得攒批原则
category: perf
keywords: [buffer,batching,latency,first-token,stream,SSE,performance]
status: active
related: [sse-chunk-stateless-defect,stream-buf-unified-cap]
---

## 硬约束

流式场景中尾行缓冲**只能留不完整的尾巴**，完整帧必须**立刻**交给下游处理。攒批会导致首 token 时延随缓冲深度衰减。

### 触发

SSE / WebSocket 等流协议的有状态缓冲（尾行缓冲 / 残行缓冲）。

### MUST

- **缓冲粒度 = 帧边界**（对 SSE = 一行）
  - 完整帧喂入后，**立刻**split 出来交给下游解析器
  - 下一 chunk 继续这个动作
  - **禁止** 攒够 N 字节或 N 行再处理

- **性能影响（硬数据）**
  - 客户端收到首 token 时延 = 网络延迟 + 服务端处理延迟 + **缓冲深度**
  - 100KB 缓冲在 1Gbps = 0.8ms 额外延迟
  - 缓冲深度越大，实时性越差

- **验证方法**
  - 加用例：喂入「一个完整帧 + 一个不完整帧」
  - 断言：完整帧**立刻**出现在下游，不延迟到下一次 feed

### 不选別的

| 做法 | 后果 |
|---|---|
| 攒够 N 字节再处理 | 首 token 时延 ∝ N，用户体验恶化 |
| 攒够 1 行再处理 | 下个 chunk 来临时延迟到缓冲够量 |
| 异步 flush | 引入不必要复杂性，缓冲深度问题仍存 |

## 代码形态

### 正例：完整行立即下发

```rust
pub(crate) fn feed(&mut self, text: &str) -> String {
    self.buf.push_str(text);
    
    // 取出所有完整行（以 \n 分界）
    let split_pos = if self.buf.ends_with('\n') {
        self.buf.len()
    } else {
        self.buf.rfind('\n').map(|p| p + 1).unwrap_or(0)
    };
    
    // split_off 残行，返回完整行给下游
    let remainder = self.buf.split_off(split_pos);
    let ready = std::mem::replace(&mut self.buf, remainder);
    
    // ✅ ready（完整行）立刻交给解析，不延迟
    ready
}
```

### 反例：攒批

```rust
// ❌ 这会导致时延衰减
pub(crate) fn feed(&mut self, text: &str) -> Vec<String> {
    self.buf.push_str(text);
    
    if self.buf.len() < BATCH_SIZE {
        return vec![];  // 攒批，等缓冲够深
    }
    
    let lines: Vec<_> = self.buf.lines().collect();
    self.buf.clear();
    lines
}
```

## 验收

- [ ] 缓冲 feed() 返回类型为完整帧（String / Vec 等），非 Option 空值
- [ ] 有用例验证「完整帧 + 残帧」输入时完整帧立刻出现
- [ ] cargo test 通过，无时延衰减告警

## 适用

- SSE / Server-Sent-Events 转发
- WebSocket 流式消息
- HTTP/2 server push
- 任何「客户端等待首 token」的流式场景

## 关联

[[sse-chunk-stateless-defect]] · [[stream-buf-unified-cap]]

## 流缓冲不得攒批原则

---
title: 流缓冲不得攒批原则
category: perf
keywords: [buffer,batching,latency,first-token,stream,SSE,performance]
status: active
---

## 硬约束

流式场景中尾行缓冲**只能留不完整的尾巴**，完整帧必须**立刻**交给下游处理。攒批会导致首 token 时延随缓冲深度衰减。

### 触发

SSE / WebSocket 等流协议的有状态缓冲（尾行缓冲 / 残行缓冲）。

### MUST

- **缓冲粒度 = 帧边界**（对 SSE = 一行）
  - 完整帧喂入后，**立刻**split 出来交给下游解析器
  - 下一 chunk 继续这个动作
  - **禁止** 攒够 N 字节或 N 行再处理

- **性能影响（硬数据）**
  - 客户端收到首 token 时延 = 网络延迟 + 服务端处理延迟 + **缓冲深度**
  - 100KB 缓冲在 1Gbps = 0.8ms 额外延迟
  - 缓冲深度越大，实时性越差

- **验证方法**
  - 加用例：喂入「一个完整帧 + 一个不完整帧」
  - 断言：完整帧**立刻**出现在下游，不延迟到下一次 feed

### 不选別的

| 做法 | 后果 |
|---|---|
| 攒够 N 字节再处理 | 首 token 时延 ∝ N，用户体验恶化 |
| 攒够 1 行再处理 | 下个 chunk 来临时延迟到缓冲够量 |
| 异步 flush | 引入不必要复杂性，缓冲深度问题仍存 |

## 代码形态

### 正例：完整行立即下发

```rust
pub(crate) fn feed(&mut self, text: &str) -> String {
    self.buf.push_str(text);
    
    // 取出所有完整行（以 \n 分界）
    let split_pos = if self.buf.ends_with('\n') {
        self.buf.len()
    } else {
        self.buf.rfind('\n').map(|p| p + 1).unwrap_or(0)
    };
    
    // split_off 残行，返回完整行给下游
    let remainder = self.buf.split_off(split_pos);
    let ready = std::mem::replace(&mut self.buf, remainder);
    
    // ✅ ready（完整行）立刻交给解析，不延迟
    ready
}
```

### 反例：攒批

```rust
// ❌ 这会导致时延衰减
pub(crate) fn feed(&mut self, text: &str) -> Vec<String> {
    self.buf.push_str(text);
    
    if self.buf.len() < BATCH_SIZE {
        return vec![];  // 攒批，等缓冲够深
    }
    
    let lines: Vec<_> = self.buf.lines().collect();
    self.buf.clear();
    lines
}
```

## 验收

- [ ] 缓冲 feed() 返回类型为完整帧（String / Vec 等），非 Option 空值
- [ ] 有用例验证「完整帧 + 残帧」输入时完整帧立刻出现
- [ ] cargo test 通过，无时延衰减告警

## 适用

- SSE / Server-Sent-Events 转发
- WebSocket 流式消息
- HTTP/2 server push
- 任何「客户端等待首 token」的流式场景

## 关联

[[sse-chunk-stateless-defect]] 和 [[stream-buf-unified-cap]] 是相关约束。
