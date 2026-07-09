# Implement: runapi 补 openai/gemini endpoints + 扩 model_list + 填 models.default + desc 改写

## 载体
- 单 subtask（单文件 `protocols.runapi` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.runapi` 块
- 禁动其他协议块、顶层 `version` / `last_updated`
- 改 `endpoints.default` / `model_list.default` / `models.default` / `desc`；`source_urls` / `name` / `homepage` / `logo_url` / `client_type` 保留

## 步骤
1. 读 `research/runapi-models.md`
2. 读 `prd.md`
3. 读现有 `protocols.runapi` 块定位
4. 改 `endpoints.default`：现有 anthropic 后追加 openai（`https://runapi.co/v1`，client_type=default）+ gemini（`https://runapi.co`，client_type=default）
5. 改 `model_list.default`：保留原 7 个 alias，末尾追加 `claude-sonnet-5` / `claude-fable-5` / `claude-sonnet-4-6-thinking`
6. 改 `models.default`：从 `{}` 改为三档（档位名 key → model id 字符串：`default`→`claude-sonnet-5` / `opus`→`claude-opus-4-8` / `haiku`→`claude-haiku-4-5`）
7. 改 `desc` 8 语言：从 "Claude-compatible" 改为 150+ 模型聚合描述（见 prd §4）
8. 验证 JSON 合法
9. 验证：
```bash
python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['runapi'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']),p['desc']['en-US'])"
```
期望：`10 {'default': 'claude-sonnet-5', 'opus': 'claude-opus-4-8', 'haiku': 'claude-haiku-4-5'} 3 RunAPI proxy - aggregated access to 150+ models (...)`

## 验收（对齐 prd）
- endpoints default = 3 端点（anthropic + openai `/v1` + gemini host）
- model_list = 10（原 7 + 新增 3）
- models.default 三档 = 档位名 key（default/opus/haiku）→ model id 字符串
- desc 8 语言改写
- source_urls 保留
- JSON 合法、其他协议块未动

## 失败处理
- JSON 解析失败 → 检查逗号/引号（endpoints 数组追加尤其注意）
- 新增 model id 拼错 → 对照 research 表（`claude-sonnet-5` / `claude-fable-5` / `claude-sonnet-4-6-thinking`）
- openai base_url 漏 `/v1` → 校对 `https://runapi.co/v1`

## 禁
- 禁动其他协议块
- 禁用 model-id 作 `models.default` key（必须用档位名 default/opus/haiku）
- 禁加 id 日期后缀（保留现有短 alias 格式）
- 禁动 STATIC_MODEL_IDS / peak_hours / coding_plan
- 禁 `git commit`
