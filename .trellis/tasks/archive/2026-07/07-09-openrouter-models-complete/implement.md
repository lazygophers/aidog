# Implement: openrouter 删 gemini endpoint + 扩 model_list 至 18 + 补 9 档

## 载体

- 单 subtask（`protocols.openrouter` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.openrouter` 块
- 禁动其他无关协议块、顶层 version/last_updated、desc/source_urls/name/homepage/logo_url/client_type

## 步骤

1. 读 `research/openrouter-models.md`（确认 gemini 不支持、18 模型清单、9 档位映射依据）
2. 读 `prd.md`（确认 endpoints 3→2、model_list 15→18、models.default 9 档）
3. 读现有 `protocols.openrouter` 块定位
4. 改：
   - `endpoints.default`：删除 gemini 端点对象（数组 filter 掉 `protocol=="gemini"`），保留 anthropic + openai 两项
   - `model_list.default`：15 → 18，在现有基础插入 `anthropic/claude-haiku-4.5`、`anthropic/claude-fable-5`（紧随 anthropic 区块）、`deepseek/deepseek-r1`（紧随 deepseek 区块）
   - `models.default`：`{}` → 9 档位完整映射（default/opus/sonnet/haiku/fable/gpt/coder/fast/thinking）
5. 验证 JSON 合法
6. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['openrouter'];print(len(p['model_list']['default']),len(p['models']['default']),len(p['endpoints']['default']));print([e['protocol'] for e in p['endpoints']['default']])"`
   - 期望输出：`18 9 2` 换行 `['anthropic', 'openai']`

## 验收（对齐 prd）

- endpoints.default 数 = 2（anthropic + openai，无 gemini）
- model_list.default 数 = 18（含 3 新增）
- models.default 数 = 9 档位，每档值为 `provider/id` 字符串
- desc/source_urls/name 不动
- cargo build/clippy/test clean

## 失败处理

- JSON 解析失败 → 检查 model_list 数组逗号、endpoints 数组末尾逗号
- endpoints 删错项 → 确认仅删 `protocol=="gemini"`，保留另两项
- cargo test 失败 → 检查是否误改其他协议块

## 禁

- 禁动其他无关协议块
- 禁用 model-id 空 obj（`{}`）
- 禁硬编码全量 344 模型（research 明确反对，月级腐化）
- 禁动 STATIC_MODEL_IDS
- 禁动 desc / source_urls / name / homepage / logo_url
- 禁 git commit
