---
title: style
category: style
keywords: [log,trace,traceid,ansi,format,spawn_traced,logging.rs,gateway]
status: active
inclusion: auto
---

## 日志格式与 traceid 取值链契约

日志格式与 traceid 取值链必须保持对称，两者配合构成诊断链。应用层（logging.rs）负责格式化和 thread-local 栈管理，网关侧（gateway 模块）负责 span instrumentation。

## 何时被读

- 改 `src-tauri/crates/aidog_core/src/logging.rs` 的格式器、新加 `tokio::spawn` 异步任务、新加诊断 id、改 traceid 取值链时
- 改 gateway 侧 proxy/command dispatch 的 span 包装

## 日志字段顺序 (MUST)

- **MUST 5 段严格顺序**: `time` → `level` → `file:line func` → `msg` → `traceid`
- **禁增减字段** (新加诊断字段统一塞 traceid 段或 msg 内, 不开新段)
- **console 与 file 共用 `FormatEvent` impl**, 仅 ANSI 标志不同, 字段顺序对称

### msg 段字段渲染契约 (MUST)

- **msg 段 MUST 包含 event 全部业务字段**（`key=value` 按 tracing 记录顺序），**禁丢字段**
- **`message` 字段** → msg 主体（字符串去引号）
- **其他业务字段**（fn / req / dur / sql / method / path 等）→ msg 段尾部追加 `{key}={value} ` 序列
- **trace_id 字段例外**：5 段格式 traceid 段单独取，event 显式带 trace_id 时**去重**

## traceid 取值链 (MUST)

每行 MUST 含 traceid，取值三级兜底 MUST：`trace_id_from_span_scope` → `current_trace_id()` → `gen_trace_id()`

## 异步分支 id 传播 (MUST)

- **新加 `tokio::spawn` MUST 走 `spawn_traced(name, fut)` helper** (`logging.rs::spawn_traced`, line 320)
- **禁裸 `tokio::spawn(fut)`** 无 instrument — 异步分支丢父子关联

## 代码位置

- **应用层格式器**：`src-tauri/crates/aidog_core/src/logging.rs:16-330`
  - `TRACE_ID_STACK` 定义：line 16
  - `current_trace_id()` 函数：line 35
  - `spawn_traced` helper：line 320
- **gateway span 包装**：各模块内（proxy/handler.rs / router/mod.rs 等）新加命令时跟进 info_span!

## 验收基准

- [ ] header `x-aidog-trace` id grep 日志命中 ≥1 行
- [ ] console ANSI 序列存在, file 纯文本无 ANSI
- [ ] traceid 每行必有（无 `-` / `unknown` / 空）
- [ ] grep `tokio::spawn\|handle.spawn` 残留仅 spawn_traced / Command::spawn / 测试

## 关联

[[platform-auto-disable-codes]]、[[remote-defaults-sync-chain]]（同类 span instrumentation 链路）
