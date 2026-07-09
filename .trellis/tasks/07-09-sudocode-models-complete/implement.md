# Implement: sudocode 补 openai/gemini endpoints + 全量扩 model_list 29 + 填 models.default + desc 改写

## 载体
- 单 subtask（单文件 `protocols.sudocode` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.sudocode` 块（line 2661-2712 区间）
- 禁动其他协议块、顶层 `version` / `last_updated`
- 改 `endpoints.default` / `model_list.default` / `models.default` / `desc`；`source_urls` / `name` / `homepage` / `logo_url` / `client_type` 保留

## 步骤
1. 读 `research/sudocode-models.md`
2. 读 `prd.md`
3. 读现有 `protocols.sudocode` 块定位
4. 改 `endpoints.default`：现有 anthropic 后追加 openai（`https://sudocode.us/v1`，client_type=default）+ gemini（`https://sudocode.us`，client_type=default）
5. 改 `model_list.default`：从 7 扩到 29（见 prd §2，Claude 9 短 alias + Gemini 7 + OpenAI 5 + MiniMax 2 + Moonshot 2 + GLM 2 + DeepSeek 2）
6. 改 `models.default`：从 `{}` 改为三档（档位名 key → model id 字符串：`default`→`claude-sonnet-4-6` / `opus`→`claude-opus-4-8` / `haiku`→`claude-haiku-4-5`）
7. 改 `desc` 8 语言：从 "Claude-compatible" 改为 7 家供应商聚合描述（见 prd §4）
8. 验证 JSON 合法
9. 验证：
```bash
python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['sudocode'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']),p['desc']['en-US'])"
```
期望：`29 {'default': 'claude-sonnet-4-6', 'opus': 'claude-opus-4-8', 'haiku': 'claude-haiku-4-5'} 3 SudoCode API - aggregated access to Claude, GPT, Gemini, and domestic models`

## 验收（对齐 prd）
- endpoints default = 3 端点（anthropic + openai `/v1` + gemini host）
- model_list = 29（Claude 9 短 alias + Gemini 7 + OpenAI 5 + MiniMax 2 + Moonshot 2 + GLM 2 + DeepSeek 2）
- models.default 三档 = 档位名 key（default/opus/haiku）→ model id 字符串
- desc 8 语言改写
- source_urls 保留
- JSON 合法、其他协议块未动

## 失败处理
- JSON 解析失败 → 检查逗号/引号（29 元素数组尤其注意）
- MiniMax id 大小写错 → 对照 research 表原样（`MiniMax-M2.7` / `MiniMax-M2.5`，M 大写）
- Gemini id 漏 `-preview` 后缀 → 对照 research 表（多数带 `-preview` / `-lite`）
- Claude alias 误加日期后缀 → 改回短 alias（`claude-haiku-4-5` 而非 `-20251001`）

## 禁
- 禁动其他协议块
- 禁用 model-id 作 `models.default` key（必须用档位名 default/opus/haiku）
- 禁加 id 日期后缀（Claude 系用短 alias）
- 禁动 STATIC_MODEL_IDS / peak_hours / coding_plan
- 禁 `git commit`
