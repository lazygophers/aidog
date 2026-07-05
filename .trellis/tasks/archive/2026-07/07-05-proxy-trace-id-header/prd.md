# proxy 响应头注入 X-AiDog-Trace trace-id (debug 模式 MITM/非 MITM 都加)

## Goal

`make run` (debug build) 启动时，proxy 返回给客户端的响应头**必须**含 `X-AiDog-Trace: <trace-id>`，便于客户端报错（如 h2 CANCEL）时关联 AirDog 侧 proxy_log / span。

## What I already know

- trace-id 系统已存在：
  - `crate::logging::current_trace_id()` — 读线程活跃 span 链最内层 trace_id/request_id
  - `crate::logging::new_trace_id()` — 8-hex 短 id（uuid v4 simple[..8]）
  - `TraceIdLayer` 维护线程本地栈
- request_id（代理请求 span）= 32-hex（proxy_log.id），优先于 trace_id
- debug 模式 = `cfg!(debug_assertions)`（make run = yarn tauri dev = debug build）
- 响应头注入点（AirDog 构造 Response 处）：
  - MITM 路径：`passthrough.rs::handle_passthrough` + `forward_passthrough_to_orig_host`，`forward.rs` 各 forward 函数，`connect.rs` MITM 分支
  - 非 MITM：`connect.rs::blind_relay_after_connect` —— **盲字节透传，无法注入 header**（加密 TLS 字节流，AirDog 看不见/改不了 HTTP 层）
  - 健康端点 / 静态 models：`health.rs`、handler.rs 静态分支
- 「无论是否是 MITM」= 凡 AirDog **构造**的响应都注入；blind_relay 是 TCP 透传非构造响应，注入不可行

## Decision (ADR-lite)

- **blind_relay 豁免**（用户裁定）：非 MITM CONNECT TCP 透传路径不注入，物理不可行
- trace-id 取值：当前 span request_id 优先 → 否则 trace_id → 否则 `new_trace_id()` 兜底
- 健康端点 / 静态 models / group-info / count_tokens 等 AirDog 直构响应**也注入**（一致诊断体验）
- gate：`cfg!(debug_assertions)`（仅 dev / `make run`），release 不注入

## Requirements

- debug build 时，所有 AirDog 构造的 proxy 响应 header 含 `X-AiDog-Trace: <id>`
- release build 不注入
- id 取值链：request_id → trace_id → new_trace_id
- blind_relay 路径豁免（注释标明物理限制）

## Acceptance Criteria (evolving)

- [ ] debug build 下，CC 客户端收到响应含 `X-AiDog-Trace` header
- [ ] 同一请求的 header id 与 proxy_log.request_id 关联（同 id 或可映射）
- [ ] release build 无该 header
- [ ] blind_relay 路径不阻塞（豁免）

## Out of Scope

- 改 trace_id 系统本身
- blind_relay 注入（物理不可行）

## Technical Notes

- 注入点（按响应构造站点枚举）：
  - `proxy/passthrough.rs`: `handle_passthrough` 响应 builder（stream/non-stream 两支）
  - `proxy/passthrough.rs`: `forward_passthrough_to_orig_host` 响应 builder
  - `proxy/forward.rs`: 各 forward 响应构造
  - `proxy/handler.rs`: 健康端点 / 静态 models / group-info / count_tokens 响应
- 实现：抽 helper `fn maybe_inject_trace_header(headers: &mut HeaderMap)` 用 `cfg!(debug_assertions)` gate
- 复用前任务 agent 已加的 `request_id` tracing 字段（worktree WIP）
