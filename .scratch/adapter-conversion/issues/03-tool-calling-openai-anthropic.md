# 03 — 工具调用 OpenAI↔Anthropic（核心）

**What to build:** function calling 三件套（tools 定义 + assistant tool_use + user tool_result，id 关联）在 OpenAI↔Anthropic 双向非流式可用：中立工具格式以 OpenAI function 格式为基准；Anthropic 入站 parse tools/tool_use/tool_result，出站反向序列化；OpenAI arguments(JSON 字符串) ↔ Anthropic input(对象) 转换正确。

**Blocked by:** 01 — 地基与回归网。

**Status:** ready-for-agent

- [ ] fixture：OpenAI 带工具请求 → Anthropic 出站（tools/tool_use/tool_result 逐字段映射断言）
- [ ] fixture：Anthropic 带工具请求 → OpenAI 出站（arguments 序列化为 JSON 字符串）
- [ ] 多工具并发调用（多 tool_use block / 多 tool_calls）round-trip 不乱 id
- [ ] 无工具请求行为不变（守卫式，回归网证明）
- [ ] cargo test / make lint 全绿
