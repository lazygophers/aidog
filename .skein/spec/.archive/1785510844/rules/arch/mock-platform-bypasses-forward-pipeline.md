---
title: mock-platform-bypasses-forward-pipeline
category: arch
keywords: [mock,StreamAggregator,STREAM_BODY_MAX_BYTES,finish.rs,loadgen,footprint]
status: active
inclusion: auto
anchors: src-tauri/crates/aidog_core/src/gateway/proxy/handler.rs,src-tauri/crates/aidog_core/src/gateway/proxy/mock.rs,src-tauri/crates/aidog_core/src/gateway/proxy/stream.rs
---

## mock 平台绕开真实转发流水线，无法验证 finish.rs 挂载的 cap/累积逻辑

## 硬约束

`platform_type=mock`（`gateway/proxy/mock.rs::handle_mock`）在 `handler.rs:410-429` 显式短路，
**完全绕开** `forward_attempt`/`finish.rs` 的真实转发流水线，因而也绕开
`StreamAggregator`/`push_upstream`/`push_client`/`join_stream_body`/`STREAM_BODY_MAX_BYTES` 等
一切挂在该流水线上的 cap / 累积逻辑。`log.response_body` 在 mock 分支里直接写死字面量
`"[mock stream]"`，事后不会被任何真实路径覆盖。

### 触发

任何要验证"流式响应体累积/截断/内存 cap"类改动（如 `STREAM_BODY_MAX_BYTES` push 点截断）的
loadgen/压测场景，若默认或图省事选了 `platform_type=mock` 作为被测对象。

### MUST

- 验证"真实转发路径"（`finish.rs`/`passthrough.rs`）里的行为（cap、累积、截断标记、SSE 行重组
  等），必须让流量走**非 mock 协议**（例如指向本地自建假上游 HTTP 服务器的普通协议平台），
  mock 平台无论请求体多大都摸不到这条路径。
- 若只能/只允许用 mock（如某些量测硬约束明确写"只用 mock 平台"），要如实认清：这类约束下
  **无法验证任何挂在 finish.rs 转发流水线上的改动**，这是架构性事实非负载量级问题，禁在报告
  里把"没测出差异"简单归因为"body 不够大/噪声盖过信号"。
- 判定 cap 是否被触发：查 `proxy_log.response_body`，若恒为 mock 占位符 `"[mock stream]"`
  且从无 `join_stream_body` 的截断标记 `"[truncated: ...]"`，即为 cap 未触达（不管请求体多大）。

### 验收

- [ ] 涉及 finish.rs 转发路径改动的验证脚本，确认未误用 `platform_type=mock` 作压测对象
- [ ] 如约束强制只能用 mock，报告里已明确说明这是架构性不可观测（非负载/噪声问题）

## 关联

proxy-hotpath-buffers s9-bigbody-footprint（`.scratch/perf-200mb/results/
proxy-hotpath-s9-bigbody-footprint.md`）
