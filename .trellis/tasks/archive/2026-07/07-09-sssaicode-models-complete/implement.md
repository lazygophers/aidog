# Implement: sssaicode 补 openai endpoint + 扩 model_list + 填 models.default + desc 改写 + source_urls 修正

## 载体
- 单 subtask（单文件 `protocols.sssaicode` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.sssaicode` 块
- 禁动其他协议块、顶层 `version` / `last_updated`
- 改 `endpoints.default` / `model_list.default` / `models.default` / `desc` / `source_urls`；`name` / `homepage` / `logo_url` / `client_type` 保留

## 步骤
1. 读 `research/sssaicode-models.md`
2. 读 `prd.md`
3. 读现有 `protocols.sssaicode` 块定位
4. 改 `endpoints.default`：现有 anthropic 后追加 openai（`https://node-hk.sssaicodeapi.com/api/v1`，client_type=default）
5. 改 `model_list.default`：保留原 7 个 alias，追加 `claude-sonnet-5` / `claude-fable-5` / `gpt-4o` / `gpt-4o-mini` / `gpt-4-turbo` / `gpt-3.5-turbo` / `o1-preview` / `deepseek-chat` / `deepseek-coder`
6. 改 `models.default`：从 `{}` 改为三档（档位名 key → model id 字符串：`default`→`claude-sonnet-5` / `opus`→`claude-opus-4-8` / `haiku`→`claude-haiku-4-5`）
7. 改 `desc` 8 语言：从 "Claude-compatible" 改为 Claude/GPT/DeepSeek 聚合描述（见 prd §4）
8. 改 `source_urls`：docs 从 `https://node-hk.sssaicodeapi.com/` 改为 `https://sssaicode.com/docs`；pricing 从 `https://node-hk.sssaicodeapi.com/` 改为 `https://sssaicode.com/pricing`
9. 验证 JSON 合法
10. 验证：
```bash
python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['sssaicode'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']),p['source_urls'])"
```
期望：`16 {'default': 'claude-sonnet-5', 'opus': 'claude-opus-4-8', 'haiku': 'claude-haiku-4-5'} 2 {'docs': 'https://sssaicode.com/docs', 'pricing': 'https://sssaicode.com/pricing'}`

## 验收（对齐 prd）
- endpoints default = 2（anthropic `/api` + openai `/api/v1`）
- model_list = 16（原 7 + claude-sonnet-5 + claude-fable-5 + OpenAI 5 + DeepSeek 2）
- models.default 三档 = 档位名 key（default/opus/haiku）→ model id 字符串
- desc 8 语言改写
- source_urls 修正为 `sssaicode.com` 主站
- JSON 合法、其他协议块未动

## 失败处理
- JSON 解析失败 → 检查逗号/引号
- openai base_url 漏 `/v1` 或多 `/api` → 校对 `https://node-hk.sssaicodeapi.com/api/v1`（完整路径 = base + `/chat/completions` = `/api/v1/chat/completions`）
- source_urls 漏改 → 对照 prd §5

## 禁
- 禁动其他协议块
- 禁用 model-id 作 `models.default` key（必须用档位名 default/opus/haiku）
- 禁加 id 日期后缀
- 禁加 HK2/HK3/CF 备用节点 endpoint
- 禁动 STATIC_MODEL_IDS / peak_hours / coding_plan
- 禁 `git commit`
