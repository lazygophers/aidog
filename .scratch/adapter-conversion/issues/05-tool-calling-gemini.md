# 05 — 工具调用 Gemini 扩展

**What to build:** 工具调用扩展到 Gemini：functionDeclarations / functionCall / functionResponse 与中立格式双向。覆盖 Gemini 特有差异——functionCall 无 id 靠 name 关联（中立→Gemini 丢 id、Gemini→中立自生成 id 并按顺序配对）。

**Blocked by:** 03 — 中立工具格式敲定。

**Status:** done

- [x] fixture：中立（含 OpenAI 格式工具）→ Gemini functionDeclarations/functionCall/functionResponse 映射断言
- [x] Gemini 入站 → 中立：无 id 工具调用自生成 id，多工具按序配对
- [x] Gemini args(对象) ↔ 中立 arguments 转换正确
- [x] cargo test / make lint 全绿
