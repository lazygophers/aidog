# 02 — 入站参数提取（max_tokens / temperature / top_p）

**What to build:** 客户端在请求里设置的采样参数经协议转换后不再丢失：Anthropic 入站 parse 读取 max_tokens/temperature/top_p/stop 填入中立模型，Gemini 入站读取 generationConfig.{maxOutputTokens/temperature/topP}；出站 serialize 反向映射（含 Gemini 出站 generationConfig 映射补全，当前缺失）。

**Blocked by:** 01 — 地基与回归网。

**Status:** ready-for-agent

- [ ] fixture：带参数的 Anthropic/Gemini 入站请求 → 转换后中立模型字段断言
- [ ] Gemini 出站 max_tokens → generationConfig.maxOutputTokens 映射生效（回归用例证明此前丢失、此后保留）
- [ ] 缺参数时不强加默认值（守卫式：有才写）
- [ ] cargo test / make lint 全绿
