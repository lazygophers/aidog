# implement.md: presets-view 可视化 HTML

## 执行层

- 载体: main 同步（单文件生成器 + Makefile + gitignore，逻辑连贯不拆 subtask）
- worktree: 无
- 并行: 串行

## 改动清单

### 步骤 1 — `scripts/presets_view/generate.py`（新增）

Python stdlib（`json` + `html` + `string` + `os`/`pathlib`），结构：

```python
#!/usr/bin/env python3
"""读 platform-presets.json + models.json → 生成单 HTML（内嵌数据 + vanilla JS）。"""
import json, html, os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PRESETS = ROOT / "src-tauri/defaults/platform-presets.json"
MODELS = ROOT / "src-tauri/defaults/models.json"
OUT = ROOT / ".aidoc/presets.html"

def load(): ...           # 读两 JSON
def build_view_data(): ... # 关联: 每协议 model_list → models 查价（override>top-level>无价）
def render(view_data): ... # string template → 单 HTML（内嵌 JSON + CSS + JS）
def main():
    data = build_view_data()
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(render(data), encoding="utf-8")
    print(f"✓ {OUT} ({OUT.stat().st_size} bytes)")

if __name__ == "__main__":
    main()
```

关键逻辑：
- **关联**：遍历 `presets["protocols"]`，每协议取 `model_list.default`（list），每个 model id 查 `models["models"][id]`：
  - 有 + `pricing[<protocol_key>]` 存在 → input/output/cache 取 override 值，标 `override`
  - 有 + 无 override → 取 top-level `input_cost_per_token` 等，标 `top-level`
  - 无 model 条目 → 标 `no_price`
- **价格换算**：per-token × 1_000_000 → $/M tokens（HTML 展示用，原始 per-token 内嵌 JSON 保留精度）
- **HTML**：`string.Template`，CSS 内联（liquid-glass 灵感配色，简洁），vanilla JS 读内嵌 `<script type="application/json" id="view-data">` 初始渲染平台卡片，点击展开模型行，搜索/排序实时 filter。

### 步骤 2 — `.gitignore`

末尾加：

```
# presets-view generated HTML (make presets-view)
.aidoc/
```

### 步骤 3 — `Makefile`

`##@ Maintenance` 段（或新增 `##@ Docs`）加：

```makefile
.PHONY: presets-view
presets-view: ## Generate interactive HTML from platform-presets.json + models.json and open it
	@printf "$(GREEN)▶ Generating presets HTML...$(RESET)\n"
	@python3 scripts/presets_view/generate.py
	@printf "$(GREEN)▶ Opening...$(RESET)\n"
	@case "$$(uname -s)" in \
		Darwin) open "$(PWD)/.aidoc/presets.html" ;; \
		Linux)  (xdg-open "$(PWD)/.aidoc/presets.html" || echo "open manually: $(PWD)/.aidoc/presets.html") ;; \
		MINGW*|MSYS*|CYGWIN*) start "" "$(PWD)/.aidoc/presets.html" ;; \
		*) echo "unsupported OS, open manually: $(PWD)/.aidoc/presets.html" ;; \
	esac
```

### 步骤 4 — 门禁

```bash
python3 scripts/presets_view/generate.py
test -f .aidoc/presets.html
python3 -c "import os; s=os.path.getsize('.aidoc/presets.html'); assert s>10000, s"
python3 -c "import json; json.load(open('src-tauri/defaults/platform-presets.json'))"  # 未改数据，回归
make help | grep presets-view
```

### 步骤 5 — 手动交互验证

`make presets-view` → 浏览器打开 → 验：
- 搜索框输入模型名（如 "claude"）→ 平台卡片实时过滤
- 排序切换 → 顺序变
- 点平台卡片 → 展开模型行，价格 $/M tokens 显示
- override 行有标记 vs top-level
- 无价模型标 `no_price`

## 自检

`✅ lint=? type=? test=? TODO=? 验收物=generate.py + Makefile presets-view + .gitignore .aidoc/ + HTML 生成+交互验证`

## 失败处理

- 生成器 JSON 解析失败：检查两 JSON 路径 + 合法性（本 task 不改 JSON，若报错是预存）。
- HTML 体积过大（>5MB）：检查是否预渲染所有模型行 DOM → 改为 JS 运行时渲染。
- `open` 跨平台失败：打印路径让用户手开（不 fail target）。
- models.json 模型漂移（model_list 有 id 但 models.json 无）：渲染 `no_price`，不报错。
