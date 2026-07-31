# SKEIN rules 规则索引 (章节粒度: 一行一条规则)

类目: arch(3), perf(14) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | inclusion | anchors | status/出链 | summary |
|---|---|---|---|---|---|---|---|
| arch/mock-platform-bypasses-forward-pipeline.md#mock 平台绕开真实转发流水线，无法验证 finish.rs 挂载的 cap/累积逻辑 | arch | mock 平台绕开真实转发流水线，无法验证 finish.rs 挂载的 cap/累积逻辑 | mock,StreamAggregator,STREAM_BODY_MAX_BYTES,finish.rs,loadgen,footprint | auto | src-tauri/crates/aidog_core/src/gateway/proxy/handler.rs,src-tauri/crates/aidog_core/src/gateway/proxy/mock.rs,src-tauri/crates/aidog_core/src/gateway/proxy/stream.rs | active | - |
| arch/mock-platform-bypasses-forward-pipeline.md#关联 | arch | 关联 | mock,StreamAggregator,STREAM_BODY_MAX_BYTES,finish.rs,loadgen,footprint | auto | src-tauri/crates/aidog_core/src/gateway/proxy/handler.rs,src-tauri/crates/aidog_core/src/gateway/proxy/mock.rs,src-tauri/crates/aidog_core/src/gateway/proxy/stream.rs | active | proxy-hotpath-buffers s9-bigbody-footprint（`.scratch/perf-20… |
| arch/mock-platform-bypasses-forward-pipeline.md#硬约束 | arch | 硬约束 | mock,StreamAggregator,STREAM_BODY_MAX_BYTES,finish.rs,loadgen,footprint | auto | src-tauri/crates/aidog_core/src/gateway/proxy/handler.rs,src-tauri/crates/aidog_core/src/gateway/proxy/mock.rs,src-tauri/crates/aidog_core/src/gateway/proxy/stream.rs | active | `platform_type=mock`（`gateway/proxy/mock.rs::handle_mock`）在 … |
| perf/hot-path-buffers.md#mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝 | perf | mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝 | mpsc,capacity,try_send,背压,深拷贝,热路径,TOCTOU | auto | src-tauri/crates/aidog_core/src/gateway/proxy/log.rs | active | mpsc 队列热路径丢弃分支：先 `Sender::capacity() == 0` 判队满再 return，避免为「确… |
| perf/stream-buf-no-batching.md#代码形态 | perf | 代码形态 | - | auto | - | active | ### 正例：完整行立即下发  ```rust pub(crate) fn feed(&mut self, text: … |
| perf/stream-buf-no-batching.md#代码形态 | perf | 代码形态 | - | auto | - | active | ### 正例：完整行立即下发  ```rust pub(crate) fn feed(&mut self, text: … |
| perf/stream-buf-no-batching.md#关联 | perf | 关联 | - | auto | - | active / →sse-chunk-stateless-defect,stream-buf-unified-cap | [[sse-chunk-stateless-defect]] · [[stream-buf-unified-cap]] |
| perf/stream-buf-no-batching.md#关联 | perf | 关联 | - | auto | - | active / →sse-chunk-stateless-defect,stream-buf-unified-cap | [[sse-chunk-stateless-defect]] 和 [[stream-buf-unified-cap]] … |
| perf/stream-buf-no-batching.md#流缓冲不得攒批原则 | perf | 流缓冲不得攒批原则 | - | auto | - | active | - |
| perf/stream-buf-no-batching.md#流缓冲不得攒批原则 | perf | 流缓冲不得攒批原则 | - | auto | - | active | - |
| perf/stream-buf-no-batching.md#硬约束 | perf | 硬约束 | - | auto | - | active | 流式场景中尾行缓冲**只能留不完整的尾巴**，完整帧必须**立刻**交给下游处理。攒批会导致首 token 时延随缓冲深… |
| perf/stream-buf-no-batching.md#硬约束 | perf | 硬约束 | - | auto | - | active | 流式场景中尾行缓冲**只能留不完整的尾巴**，完整帧必须**立刻**交给下游处理。攒批会导致首 token 时延随缓冲深… |
| perf/stream-buf-no-batching.md#适用 | perf | 适用 | - | auto | - | active | - SSE / Server-Sent-Events 转发 - WebSocket 流式消息 - HTTP/2 serv… |
| perf/stream-buf-no-batching.md#适用 | perf | 适用 | - | auto | - | active | - SSE / Server-Sent-Events 转发 - WebSocket 流式消息 - HTTP/2 serv… |
| perf/stream-buf-no-batching.md#验收 | perf | 验收 | - | auto | - | active | - [ ] 缓冲 feed() 返回类型为完整帧（String / Vec 等），非 Option 空值 - [ ] 有… |
| perf/stream-buf-no-batching.md#验收 | perf | 验收 | - | auto | - | active | - [ ] 缓冲 feed() 返回类型为完整帧（String / Vec 等），非 Option 空值 - [ ] 有… |
| perf/stream-buffer-no-batching-delay.md#流缓冲不得攒批 | perf | 流缓冲不得攒批 | - | auto | - | active | - |
