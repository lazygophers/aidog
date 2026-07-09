# Implement: rightcode model_list+endpoints+desc 全量补全

## 载体

- 单 subtask（单文件 `protocols.rightcode` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.rightcode` 块（仅此块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤

1. 读 `research/rightcode-models.md`（`/models/public` 全量 + 7 渠道 + 现有核对）
2. 读 `prd.md`（endpoints 补全 + 27 模型清单 + 3 id 修正 + 三档）
3. 读现有 `protocols.rightcode` 块定位
4. 改 `endpoints.default`：保留 anthropic `/claude` + openai `/codex/v1`，**新增** gemini `/gemini` + openai `/deepseek`
5. 改 `model_list.default`（27 模型，裸 id）：
   - Claude 9：claude-fable-5 / claude-haiku-4-5-20251001 / claude-opus-4-5-20251101 / claude-opus-4-6 / claude-opus-4-7 / claude-opus-4-8 / claude-sonnet-4-5-20250929 / claude-sonnet-4-6 / claude-sonnet-5
   - Codex 8：codex-auto-review / gpt-5.4 / gpt-5.4-high / gpt-5.4-medium / gpt-5.4-mini / gpt-5.4-xhigh / gpt-5.5 / gpt-5.5-openai-compact
   - Gemini 8：gemini-2.5-flash / gemini-2.5-pro / gemini-3-flash-preview / gemini-3-pro-preview / gemini-3.1-pro / gemini-3.1-pro-preview / gemini-3.1-pro-preview-customtools / gemini-3.5-flash
   - DeepSeek 2：deepseek-v4-flash / deepseek-v4-pro
6. 改 `models.default`：三档（claude-sonnet-5 / gpt-5.5 / deepseek-v4-pro）
7. 改 `desc` 8 语言
8. 改 `source_urls.pricing`：`https://right.codes/pricing` → `https://right.codes/models/public`
9. 验证 JSON 合法
10. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));r=d['protocols']['rightcode'];print(len(r['endpoints']['default']),len(r['model_list']['default']),r['models']['default'],r['source_urls']['pricing'])"`
   预期输出含三档档位映射：`{'sonnet': 'claude-sonnet-5', 'gpt': 'gpt-5.5', 'default': 'deepseek-v4-pro'}`

## 验收（对齐 prd）

- [ ] endpoints 4
- [ ] 3 claude id 补后缀（haiku-4-5-20251001 / opus-4-5-20251101 / sonnet-4-5-20250929）
- [ ] model_list 27
- [ ] models.default 三档
- [ ] desc 8 语言
- [ ] source_urls.pricing 改 `/models/public`
- [ ] JSON 合法
- [ ] 仅改 rightcode 块

## 失败处理

- JSON 解析失败 → 检查逗号/引号
- desc 翻译卡住 → 参考现有平台对应语言风格
- endpoint base_url 拼写疑虑 → 对照 research「API Endpoints」段

## 禁

- 禁动其他协议块
- 禁收阿里系测试渠道模型
- 禁收 claude-aws / 画图模型
- 禁用裸 id 以外格式（必须裸 id）
- 禁保留缺后缀的 claude-haiku-4-5 / claude-opus-4-5 / claude-sonnet-4-5
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
