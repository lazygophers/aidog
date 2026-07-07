# PRD: presets.html JSON.parse 嵌入实体炸（html.escape 误用）

> 已归档 task `07-07-presets-view-html` 的回归 bug。运行时报 `Uncaught SyntaxError: Expected property name or '}' in JSON at position 1`。

## 根因

`scripts/presets_view/generate.py:421`:
```python
return HTML_TEMPLATE.replace("__DATA__", html.escape(payload))
```

`html.escape` 把 JSON 的 `"` 转 `&quot;`、`<` 转 `&lt;`。但 `<script type="application/json">` 的 `textContent` **不反转 HTML 实体**（script 内容是 raw text，仅 `</script>` 终止）→ `JSON.parse(textContent)` 看到 `{&quot;protocols"...` 在 position 1（`&` after `{`）炸。

## 目标 (axis A)

- 嵌入 `<script type="application/json">` 的 JSON payload **禁 html.escape**
- 防 `</script>` 注入：`json.dumps(...).replace("<", "\\u003c")`（script-tag 安全嵌入标准模式，同 `</script>` 序列被打断）

## 非目标

- 不改 HTML 其他段（CSS / JS 逻辑 / 模板结构）
- 不改数据 JSON（platform-presets.json / models.json）
- 不改 Makefile / .gitignore

## 交付 (axis B)

| # | 交付物 | 验收 |
|---|--------|------|
| D1 | `scripts/presets_view/generate.py` render：`html.escape(payload)` → `payload.replace("<", "\\u003c")`（去 escape，仅 escape `<` 防 `</script>`） | 生成 HTML 后 `<script type="application/json" id="view-data">` 内容是合法 JSON 原文（`"` 未转实体）|
| D2 | 验证：生成 HTML + 抽取 `<script id="view-data">` 内容 + `python3 -c "import json,re,sys; t=open('.aidoc/presets.html').read(); m=re.search(r'id=\"view-data\">(.*?)</script>', t, re.S); json.loads(m.group(1))"` exit 0 | JSON 可解析 |
| D3 | 浏览器打开无 SyntaxError（main 手动验，subagent 无 GUI；D2 等价静态保证） | — |

## 调度

单 task，1 文件 1 行改。trellis-implement 内联直做。无 worktree（极小，直接主工作区）—— 但 flow 默认强制 worktree，遵默认。

## 风险

- **低**：数据含 `<` 被转 `<` 后展示精度。→ 缓解：仅展示用，JSON.parse 还原 `<`→`<`，无失真。
