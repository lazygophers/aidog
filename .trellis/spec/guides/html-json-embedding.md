---
updated: 2026-07-07
rewrite-version: 1
supersedes: []
authored-by: trellisx-spec
mode: sediment
---

# HTML JSON Embedding

何时被读: server-side / build-time 模板（Python / JS / Rust）把 JSON payload 嵌入 `<script type="application/json">` 时
谁读: trellis-implement sub-agent / main
不遵守的代价: `JSON.parse(textContent)` 运行时炸 `SyntaxError`，整页数据失效

---

## MUST

`<script type="application/json">` 的 textContent 是 **raw text**（仅 `</script>` 序列终止，**不反转 HTML 实体**）。嵌入 JSON payload 时：

- **禁** `html.escape(payload)` / 等价全量实体转义 —— 会把 `"` 转 `&quot;`，浏览器 `JSON.parse(textContent)` 看到 `{&quot;...` 在 position 1 炸（textContent 不还原实体）
- **必须** 仅防 `</script>` 注入：`json.dumps(...).replace("<", "\\u003c")`（script-tag 安全嵌入标准模式）
  - `<` → `<` 不影响 `JSON.parse`（JS 解码回 `<`，无失真）
  - 打断 `</script>` 序列即可，无需转 `>` 或 `"`

## MUST NOT

- 禁对嵌入 script 的 JSON payload 用任何 HTML 实体转义（`html.escape` / `escapeHtml` / 模板引擎 autoescape）
- 禁仅转 `</script>` 字面量而不处理 `<`（数据含 `<script>` 仍会匹配终止符）—— 用 `<` 打断所有 `<` 才彻底

## Verification

```bash
# 抽取嵌入 JSON + 校验可解析 + 无实体
python3 -c "
import json, re
t = open('<generated>.html').read()
m = re.search(r'<script[^>]*type=\"application/json\"[^>]*>(.*?)</script>', t, re.S)
s = m.group(1)
assert s.startswith('{') or s.startswith('['), 'textContent 被实体化'
json.loads(s)  # exit 0 = 可解析
assert '&quot;' not in s, '残留 html.escape 实体'
print('OK')
"
```

## 踩坑来源

task `07-07-presets-html-json-escape-fix`：`scripts/presets_view/generate.py:421` 用 `html.escape(payload)` 嵌入 `<script type="application/json" id="view-data">`，浏览器报 `Uncaught SyntaxError: Expected property name or '}' in JSON at position 1`。根因：误以为 script 内容会被浏览器反转实体，实际 textContent 是 raw text。
