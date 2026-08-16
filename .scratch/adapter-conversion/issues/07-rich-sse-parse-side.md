# 07 — 富流式 SSE：parse 侧产出新事件

**What to build:** 各协议 SSE 解析侧（parse_*_sse）识别并产出工具/思考事件变体：Anthropic content_block_start(tool_use)/input_json_delta/thinking_delta、OpenAI tool_calls delta / reasoning_content delta、Gemini 对应事件 → 统一中立 ChatStreamEvent 新变体。to_sse 消费侧暂不动（新变体可先丢弃或仅文本透传，行为不回归）。

**Blocked by:** 03 — 工具事件语义；06 — 思考事件语义。

**Status:** done

- [x] fixture：三家真实 SSE chunk 序列 → parse 后中立事件序列断言（工具/思考/文本混合）
- [x] 纯文本 SSE 现有行为不变（回归网证明）
- [x] cargo test / make lint 全绿
