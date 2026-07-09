# Implement: deepseek 删弃用别名 + 补 thinking 档

## 载体

- 单 subtask（单文件 protocols.deepseek 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.deepseek` 块
- 禁动其他协议块、顶层 version/last_updated
- 禁 git commit

## 步骤

1. 读 `research/deepseek-models.md`（确认 V4 在售 line 28-37 + 别名弃用 line 42-43）
2. 读 `prd.md`（确认改动 delta）
3. 读现有 `protocols.deepseek` 块定位（`grep -n '"deepseek"' src-tauri/defaults/platform-presets.json`）
4. 改动：
   - `model_list.default`：删 `"deepseek-chat"` + `"deepseek-reasoner"`（4→2 项）
   - `models.default`：追加 `"thinking": "deepseek-v4-pro"`（1→2 档）
   - endpoints / desc / source_urls / name 不动
5. 验证 JSON 合法
6. 验证计数：
   ```bash
   python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['deepseek'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"
   ```
   预期输出：`2 {'default': 'deepseek-v4-flash', 'thinking': 'deepseek-v4-pro'} 2`

## 验收（对齐 prd）

- [ ] model_list.default 2 项（无 deepseek-chat / deepseek-reasoner）
- [ ] models.default 2 档（default + thinking），value 全 string
- [ ] endpoints.default 2 端点不动
- [ ] JSON 合法

## 失败处理

- JSON 解析失败 → 检查删项后逗号/引号（数组删项易留尾逗号）
- grep 找不到 deepseek 块 → 用 python 定位 key 行号

## 禁

- 禁动其他协议块
- 禁用 model-id 空 obj（value 必须 string）
- 禁动 endpoints（已正确）
- 禁 git commit
