# 01 — 中立模型地基 + fixture 回归网

**What to build:** 协议互转的中立模型能承载 block 级内容：`Content` 提供 block 访问 helper（以 `Value` 承载，不强造强类型 enum）、`ChatStreamEvent` 定义工具/思考流式变体（ToolCallStart / ToolCallArgsDelta / ThinkingDelta / Finish，先定义不消费）、`ChatRequest` 确保参数字段语义完整。同时建 fixture 成对 json 回归（请求入 + 期望出），覆盖纯文本对话在 5 协议各方向的现有行为，锁住回归基线。

**Blocked by:** None — can start immediately.

**Status:** done (2026-08-16)

- [x] `Content` helper（blocks() / push_block() / as_text()）就位，纯文本路径行为不变
- [x] `ChatStreamEvent` 新变体已定义，编译通过且现有流式路径不受影响
- [x] fixture 成对 json 落盘，OpenAI↔Anthropic↔Gemini 纯文本各方向 round-trip 测试绿
- [x] cargo test / make lint 全绿
