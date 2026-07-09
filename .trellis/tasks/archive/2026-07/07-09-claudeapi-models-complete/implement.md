# Implement: ClaudeAPI 补全 model_list + models.default + desc + source_urls

## 载体
- 单 subtask（单文件 `protocols.claudeapi` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.claudeapi` 块
- 禁动其他协议块、顶层 `version` / `last_updated`、STATIC_MODEL_IDS

## 步骤
1. 读 `research/claudeapi-models.md`
2. 读 `prd.md`
3. 读现有 `protocols.claudeapi` 块定位
4. 改 model_list（新增 claude-fable-5 + claude-sonnet-5，保留其余 7 alias 不动日期后缀）/ models.default / desc / source_urls（endpoints 保留）
5. 验证 JSON 合法
6. 验证：
   ```bash
   python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['claudeapi'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']),p['source_urls'])"
   ```
   预期输出：`9 {'sonnet': 'claude-sonnet-5', 'opus': 'claude-opus-4-8', 'haiku': 'claude-haiku-4-5'} 1 {'docs': 'https://apito.ai/en/blog/getting-started/claude-api-model-id-list/', 'pricing': 'https://apito.ai/en/blog/pricing/claude-api-pricing-guide/'}`

## 验收（对齐 prd）
- endpoints.default = 1 端点（保留）
- model_list.default = 9 裸 id，含 claude-fable-5 + claude-sonnet-5
- model_list 中现有 7 个 aidog alias 保留原状（不增删日期后缀）
- models.default = 3 档位名 key（sonnet / opus / haiku），value 为 model id string
- desc = 8 语言改写
- source_urls 两字段均改为 apito.ai 博客
- JSON 合法

## 失败处理
- JSON 解析失败 → 检查逗号 / 引号
- python 校验抛 KeyError → 块名拼写或路径错

## 禁
- 禁动其他协议块
- 禁用 model-id 空 obj 作 models.default value（value 必须 string，对齐 `Partial<Record<ModelSlot, string>>`）
- 禁加 id 日期后缀（claude-haiku-4-5 / claude-opus-4-5 / claude-sonnet-4-5 保留现状）
- 禁动 endpoints（保留单 anthropic 端点）
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
