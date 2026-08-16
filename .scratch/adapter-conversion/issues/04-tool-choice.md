# 04 — tool_choice 映射

**What to build:** 客户端指定的工具选择策略（auto / none / required / 具名工具）跨协议保留：OpenAI tool_choice ↔ Anthropic {type:auto/any/tool,name} 双向；具名工具名不丢。

**Blocked by:** 03 — 中立工具格式与 OA 双向链路。

**Status:** ready-for-agent

- [ ] fixture：四种 tool_choice 形态 OA 双向映射断言
- [ ] 未指定 tool_choice 时不输出该字段（守卫式）
- [ ] cargo test / make lint 全绿
