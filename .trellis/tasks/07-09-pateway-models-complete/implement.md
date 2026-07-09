# Implement: pateway model_list+endpoints+desc 补全

## 载体

- 单 subtask（单文件 `protocols.pateway` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.pateway` 块（仅此块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤

1. 读 `research/pateway-models.md`（5 家 11 模型 + 2 endpoint + 2 下架 + source_urls）
2. 读 `prd.md`（13 模型清单 + 三档 + source_urls 修正）
3. 读现有 `protocols.pateway` 块定位
4. endpoints 不动（2 端点正确）
5. 改 `model_list.default`（13，裸 id）：
   - Claude 5：claude-opus-4-8 / claude-opus-4-7 / claude-opus-4-6 / claude-sonnet-4-6 / claude-haiku-4-5
   - **删** claude-opus-4-5 / claude-sonnet-4-5（下架）
   - Codex 2：gpt-5.5 / gpt-5.3-codex
   - DeepSeek 2：deepseek-v4-pro / deepseek-v4-flash
   - Qwen 2：qwen3.7-max / qwen3.6-plus
   - GLM 2：glm-5.1 / glm-5
6. 改 `models.default`：三档（claude-sonnet-4-6 / gpt-5.5 / deepseek-v4-pro）
7. 改 `desc` 8 语言
8. 改 `source_urls`：docs → `https://pateway.ai/docs/`，pricing → `https://pateway.ai/docs/pricing.html`
9. 验证 JSON 合法
10. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['pateway'];print(len(p['model_list']['default']),list(p['models']['default'].keys()),p['source_urls'])"`

## 验收（对齐 prd）

- [ ] endpoints 2 保留
- [ ] claude-opus-4-5 + claude-sonnet-4-5 已删
- [ ] model_list 13
- [ ] models.default 三档
- [ ] desc 8 语言
- [ ] source_urls 修正为 /docs/ + /docs/pricing.html
- [ ] JSON 合法
- [ ] 仅改 pateway 块

## 失败处理

- JSON 解析失败 → 检查逗号/引号
- desc 翻译卡住 → 参考现有 packycode/cherryin 对应语言风格

## 禁

- 禁动其他协议块
- 禁动 endpoints
- 禁保留 claude-opus-4-5 / claude-sonnet-4-5（下架）
- 禁加 gemini 端点（不支持）
- 禁用 `provider/model` 前缀（必须裸 id）
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
