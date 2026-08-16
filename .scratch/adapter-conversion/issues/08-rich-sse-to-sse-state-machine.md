# 08 — 富流式 SSE：to_sse 状态机

**What to build:** to_sse 从无状态函数升级为跨 chunk 状态载体（工具参数分片增量累加：ToolIndex/ToolID/ToolName 维护），中立事件流 → 各协议 SSE 输出工具调用/思考/thinking_delta 完整序列。Anthropic input_json_delta / OpenAI arguments 增量按目标协议格式正确分片输出。

**Blocked by:** 07 — parse 侧事件变体。

**Status:** ready-for-agent

- [ ] fixture：中立事件流（含 ToolCallStart/ArgsDelta 分片）→ Anthropic SSE 与 OpenAI SSE 输出逐 chunk 断言
- [ ] 同一工具多个 ArgsDelta 分片聚合后 arguments 为合法 JSON
- [ ] 多工具交错增量不串 index/id
- [ ] 纯文本流不回归；cargo test / make lint 全绿
