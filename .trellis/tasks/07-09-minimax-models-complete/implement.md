# Implement: minimax(+minimax_en) 补 highspeed 变体 + 多档 + 改写 desc

## 载体

- 单 subtask（`protocols.minimax` + `protocols.minimax_en` 两块，对称改）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.minimax` 与 `protocols.minimax_en` 两块
- 禁动其他无关协议块、顶层 version/last_updated、endpoints/source_urls/name/homepage/logo_url/client_type

## 步骤

1. 读 `research/minimax-models.md`（确认 8 stable + 3 highspeed 清单、abab 废弃判定、M3 旗舰定位）
2. 读 `prd.md`（确认 4 档位映射 + desc 8 语言改写表）
3. 读现有 `protocols.minimax` 与 `protocols.minimax_en` 块定位
4. 对**两块分别**改：
   - `model_list.default`：5 → 8（追加 `MiniMax-M2.7-highspeed` / `MiniMax-M2.5-highspeed` / `MiniMax-M2.1-highspeed`，按 prd 顺序）
   - `models.default`：`{"default":"MiniMax-M3"}` → `{"default":"MiniMax-M3","sonnet":"MiniMax-M2.7","coder":"MiniMax-M2.5","fast":"MiniMax-M2.7-highspeed"}`
   - `desc`：8 语言全改写（去 "abab"，改 "Hailuo M 系列"）
5. 验证 JSON 合法
6. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));[print(k,len(d['protocols'][k]['model_list']['default']),d['protocols'][k]['models']['default'],len(d['protocols'][k]['endpoints']['default'])) for k in ['minimax','minimax_en']]"`
   - 期望两行均输出：`8 {'default': 'MiniMax-M3', 'sonnet': 'MiniMax-M2.7', 'coder': 'MiniMax-M2.5', 'fast': 'MiniMax-M2.7-highspeed'} 2`

## 验收（对齐 prd）

- 两块 model_list.default 均 8 模型（含 3 highspeed）
- 两块 models.default 均 4 档位（default/sonnet/coder/fast）
- 两块 desc 8 语言均无 "abab"，均为 Hailuo M 系列
- 两块 endpoints 数 = 2（未动）
- 两块 source_urls/name 不动
- cargo build/clippy/test clean

## 失败处理

- JSON 解析失败 → 检查逗号/引号（desc 多语言改写易漏逗号）
- highspeed id 大小写疑问 → research 反引号标注为 `MiniMax-M2.7-highspeed`（仅 H 大写）
- cargo test 失败 → 检查是否误改其他协议块

## 禁

- 禁动其他无关协议块
- 禁用 model-id 空 obj（`{}`）
- 禁动 endpoints（含 `coding_plan: false` flag）
- 禁动 STATIC_MODEL_IDS
- 禁保留 desc 中 "abab" 表述（research 判定已废弃）
- 禁 git commit
