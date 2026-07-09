# Implement: codex 补 gpt-5.3-codex-spark + fast 档

## 载体

- 单 subtask（单文件 protocols.codex 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.codex` 块
- 禁动其他协议块、顶层 version/last_updated
- 禁动 STATIC_MODEL_IDS（passthrough.rs）
- 禁 git commit

## 步骤

1. 读 `research/codex-models-endpoints.md`（确认推荐清单 line 119 + endpoints 论证 line 131）
2. 读 `prd.md`（确认改动 delta）
3. 读现有 `protocols.codex` 块定位（`grep -n '"codex"' src-tauri/defaults/platform-presets.json`）
4. 改动：
   - `model_list.default`：尾追加 `"gpt-5.3-codex-spark"`（3→4 项）
   - `models.default`：追加 `"fast": "gpt-5.4-mini"`（1→2 档）
   - endpoints/desc/source_urls/name 不动
5. 验证 JSON 合法
6. 验证计数：
   ```bash
   python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['codex'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"
   ```
   预期输出：`4 {'gpt': 'gpt-5.5', 'fast': 'gpt-5.4-mini'} 1`

## 验收（对齐 prd）

- [ ] model_list.default 4 项含 gpt-5.3-codex-spark
- [ ] models.default 2 档（gpt + fast），value 全 string
- [ ] endpoints 单端点不动
- [ ] JSON 合法

## 失败处理

- JSON 解析失败 → 检查逗号/引号（追加项易漏逗号）
- grep 找不到 codex 块 → 用 python 定位 key 行号

## 禁

- 禁动其他协议块
- 禁用 model-id 空 obj（value 必须 string，档位名 key 才正确）
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
