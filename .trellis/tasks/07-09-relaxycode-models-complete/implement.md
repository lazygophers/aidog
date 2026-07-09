# Implement: relaxycode 填 models.default 三档 + desc 改写

## 载体
- 单 subtask（单文件 `protocols.relaxycode` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.relaxycode` 块
- 禁动其他协议块、顶层 `version` / `last_updated`
- 仅改 `models.default` / `desc`；`endpoints` / `model_list` / `source_urls` / `name` / `homepage` / `logo_url` / `client_type` 保留

## 步骤
1. 读 `research/relaxycode-models.md`
2. 读 `prd.md`
3. 读现有 `protocols.relaxycode` 块定位（line 2874-2935 区间）
4. 改 `models.default`：从 `{}` 改为三档（档位名 key → model id 字符串：`default`→`claude-sonnet-4-6` / `opus`→`claude-opus-4-8` / `haiku`→`claude-haiku-4-5`）
5. 改 `desc` 8 语言：从 "Claude-compatible" 改为多供应商聚合描述（见 prd §4）
6. 验证 JSON 合法
7. 验证：
```bash
python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['relaxycode'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']),p['desc']['en-US'])"
```
期望：`7 {'default': 'claude-sonnet-4-6', 'opus': 'claude-opus-4-8', 'haiku': 'claude-haiku-4-5'} 3 RelaxyCode API - aggregated access to Claude, GPT, Gemini`

## 验收（对齐 prd）
- endpoints 3 端点保留（openai 带 `/v1`）
- model_list 7 个 Claude alias 保留不动
- models.default 三档 = 档位名 key（default/opus/haiku）→ model id 字符串（claude-sonnet-4-6 / claude-opus-4-8 / claude-haiku-4-5）
- desc 8 语言改写完成
- source_urls 保留
- JSON 合法、其他协议块未动

## 失败处理
- JSON 解析失败 → 检查逗号/引号
- models.default key 误写 model-id（应为档位名 default/opus/haiku）→ 改回档位名
- desc 改写遗漏某语言 → 对照 prd §4 八语言逐条核对

## 禁
- 禁动其他协议块
- 禁用 model-id 作 `models.default` key（必须用档位名 default/opus/haiku）
- 禁加 id 日期后缀
- 禁臆造 GPT/Gemini 具体 model id（数据弱，待 HTTP 451 解锁）
- 禁动 STATIC_MODEL_IDS / peak_hours / coding_plan
- 禁 `git commit`
