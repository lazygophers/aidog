# 06 — thinking / reasoning 双向

**What to build:** 思考链跨协议保留：请求侧开关映射（Anthropic thinking{type,budget_tokens} ↔ Gemini thinkingConfig.thinkingBudget ↔ OpenAI reasoning_effort）；响应侧内容双向（thinking block ↔ thought part ↔ reasoning_content）；Anthropic thinking block 的加密 signature 经中立模型 extra 透传，回传 Anthropic 时带回（缺失时降级不回传，不报错）。

**Blocked by:** 01 — 地基与回归网。

**Status:** ready-for-agent

- [ ] fixture：三家请求开关映射双向断言
- [ ] fixture：thinking 内容 block 双向转换断言（非流式）
- [ ] signature 透传：Anthropic→中立→Anthropic round-trip 带 signature 不丢；无 signature 时不回传 thinking 不报错
- [ ] cargo test / make lint 全绿
