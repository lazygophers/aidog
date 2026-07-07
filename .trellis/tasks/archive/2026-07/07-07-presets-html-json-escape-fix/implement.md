# implement.md: presets.html JSON 嵌入修复

> 配合 PRD。1 文件 1 行改。

## 执行层

- 载体: trellis-implement 内联直做
- worktree: 默认（flow 强制）
- 并行: 禁
- 门禁: `python3 scripts/presets_view/generate.py` + JSON 抽取解析

## 改动清单

### 步骤 1 — generate.py render 修（D1）

`scripts/presets_view/generate.py:421`:

```python
# 改前
return HTML_TEMPLATE.replace("__DATA__", html.escape(payload))

# 改后
# script type="application/json" 的 textContent 不反转 HTML 实体，
# html.escape 会把 " 转 &quot; 致 JSON.parse 炸。
# 仅 escape "<" 防 </script> 注入（< 不影响 JSON.parse 还原）。
return HTML_TEMPLATE.replace("__DATA__", payload.replace("<", "\\u003c"))
```

### 步骤 2 — 门禁（D2）

```bash
python3 scripts/presets_view/generate.py
python3 -c "
import json, re
t = open('.aidoc/presets.html').read()
m = re.search(r'id=\"view-data\">(.*?)</script>', t, re.S)
json.loads(m.group(1))
print('JSON OK')
"
```

exit 0 + 打印 `JSON OK`。

## 自检

`✅ lint=N/A type=N/A test=JSON-parse-OK TODO=0 验收物=generate.py 去 html.escape + 防 </script> + 嵌入 JSON 可解析`

## 失败处理

- JSON 仍炸：检查 payload.replace 是否漏（仅 `<`，不是全 escape）
- 浏览器仍报错：检查是否有 `</script>` 序列漏防（数据含该串）→ `</script>` 已打断
