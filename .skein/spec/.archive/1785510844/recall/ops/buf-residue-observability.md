---
title: buf-residue-observability
category: ops
keywords: []
status: active
inclusion: auto
---

## 缓冲残留处置·禁静默丢原则

---
title: 缓冲残留处置·禁静默丢原则
category: ops
keywords: [observability,buffer-residue,logging,stderr,stream,SSE,debuggability]
status: active
related: [sse-chunk-stateless-defect,stream-buf-unified-cap]
---

## 缺陷根因分析

SSE 流处理中，缓冲残留（流末有不完整帧）本身不是 bug —— 半帧因定义就不合法，丢弃是对的。**但静默丢弃正是这类 bug 长期难被发现的根因**。

### 典型路径：调试困难

1. 上游网络异常 / 恶意上游 / 边界 case → 流意外中断，缓冲有半行
2. 半行被无声丢弃，无日志
3. 用户感知 = 内容缺失（但没有错误信号指向问题源）
4. 调试人员无从追踪：是网络问题？上游问题？还是应用 bug？

## 原则：不静默丢

### MUST

- **在 Drop trait 或流末清理处记 WARN log** —— 任何缓冲残留 drop 时
  - 包含残留长度 + 上下文（如流 ID / 连接信息）
  - 即使是正常结束，也该记「这里有残留，符合预期」还是「异常中断有残留」

- **禁止无声丢弃**
  - 不是 unwrap/panic（那会中止程序）
  - 而是「记 warn + 丢」 = 可调试 + 不中止

- **选择理由**要写进代码注释
  - 为什么是 warn 而非 error？（因为流末有半帧是 OK 的，不代表数据处理失败）
  - 为什么要记？（静默丢导致问题难追踪）

### 代码形态

```rust
impl Drop for SseLineReassembler {
    fn drop(&mut self) {
        if !self.buf.is_empty() {
            tracing::warn!(
                len = self.buf.len(),
                "SSE stream ended with incomplete trailing line, discarding (not a valid frame)"
            );
        }
    }
}
```

### 反例

```rust
// ❌ 无声丢弃 —— 问题难追踪
impl Drop for SseLineReassembler {
    fn drop(&mut self) {
        // 注释都没有，直接丢弃
    }
}

// ❌ 硬 panic —— 正常流末也中止程序
impl Drop for SseLineReassembler {
    fn drop(&mut self) {
        if !self.buf.is_empty() {
            panic!("buffer not empty!");  // 过度反应
        }
    }
}
```

## 日志等级选择

| 场景 | 等级 | 理由 |
|---|---|---|
| 正常流末有残行（客户端断连/超时） | WARN | 不是错误，但异常模式需记录 |
| 上界被触发丢弃 | WARN | 恶意/异常上游，需告警 |
| 缓冲内部一致性破坏 | ERROR | 应用 bug，需重视 |

## 验收

- [ ] Drop impl 存在（或流末清理函数有相关 log）
- [ ] warn 日志含残留长度和必要上下文
- [ ] 单元测试：流末有残行时验证日志输出（如 `tracing::subscriber::with_default`）
- [ ] 代码注释说明「为什么不静默丢」

## 适用

- 任何有状态缓冲的流式处理
- 特别是帧边界缓冲（SSE / WebSocket 等）
- 异步处理中 drop 可能被调用多次的结构

## 关联

[[sse-chunk-stateless-defect]] （缓冲架构） · [[stream-buf-unified-cap]] （上界原则）

## 缓冲残留处置·禁静默丢原则

---
title: 缓冲残留处置·禁静默丢原则
category: ops
keywords: [observability,buffer-residue,logging,stderr,stream,SSE,debuggability]
status: active
---

## 缺陷根因分析

SSE 流处理中，缓冲残留（流末有不完整帧）本身不是 bug —— 半帧因定义就不合法，丢弃是对的。**但静默丢弃正是这类 bug 长期难被发现的根因**。

### 典型路径：调试困难

1. 上游网络异常 / 恶意上游 / 边界 case → 流意外中断，缓冲有半行
2. 半行被无声丢弃，无日志
3. 用户感知 = 内容缺失（但没有错误信号指向问题源）
4. 调试人员无从追踪：是网络问题？上游问题？还是应用 bug？

## 原则：不静默丢

### MUST

- **在 Drop trait 或流末清理处记 WARN log** —— 任何缓冲残留 drop 时
  - 包含残留长度 + 上下文（如流 ID / 连接信息）
  - 即使是正常结束，也该记「这里有残留，符合预期」还是「异常中断有残留」

- **禁止无声丢弃**
  - 不是 unwrap/panic（那会中止程序）
  - 而是「记 warn + 丢」 = 可调试 + 不中止

- **选择理由**要写进代码注释
  - 为什么是 warn 而非 error？（因为流末有半帧是 OK 的，不代表数据处理失败）
  - 为什么要记？（静默丢导致问题难追踪）

### 代码形态

```rust
impl Drop for SseLineReassembler {
    fn drop(&mut self) {
        if !self.buf.is_empty() {
            tracing::warn!(
                len = self.buf.len(),
                "SSE stream ended with incomplete trailing line, discarding (not a valid frame)"
            );
        }
    }
}
```

### 反例

```rust
// ❌ 无声丢弃 —— 问题难追踪
impl Drop for SseLineReassembler {
    fn drop(&mut self) {
        // 注释都没有，直接丢弃
    }
}

// ❌ 硬 panic —— 正常流末也中止程序
impl Drop for SseLineReassembler {
    fn drop(&mut self) {
        if !self.buf.is_empty() {
            panic!("buffer not empty!");  // 过度反应
        }
    }
}
```

## 日志等级选择

| 场景 | 等级 | 理由 |
|---|---|---|
| 正常流末有残行（客户端断连/超时） | WARN | 不是错误，但异常模式需记录 |
| 上界被触发丢弃 | WARN | 恶意/异常上游，需告警 |
| 缓冲内部一致性破坏 | ERROR | 应用 bug，需重视 |

## 验收

- [ ] Drop impl 存在（或流末清理函数有相关 log）
- [ ] warn 日志含残留长度和必要上下文
- [ ] 单元测试：流末有残行时验证日志输出（如 `tracing::subscriber::with_default`）
- [ ] 代码注释说明「为什么不静默丢」

## 适用

- 任何有状态缓冲的流式处理
- 特别是帧边界缓冲（SSE / WebSocket 等）
- 异步处理中 drop 可能被调用多次的结构

## 关联

[[sse-chunk-stateless-defect]] 阐述缓冲架构，[[stream-buf-unified-cap]] 涉及上界原则。
