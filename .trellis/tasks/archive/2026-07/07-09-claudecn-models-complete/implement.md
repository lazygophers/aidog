# Implement: ClaudeCN 修正 anthropic host + 补 models.default + desc 改写

## 载体
- 单 subtask（单文件 `protocols.claudecn` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围
- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.claudecn` 块
- 禁动其他协议块、顶层 `version` / `last_updated`、STATIC_MODEL_IDS

## 步骤
1. 读 `research/claudecn-models.md`
2. 读 `prd.md`
3. 读现有 `protocols.claudecn` 块定位
4. 改 endpoints（anthropic host `.top` → `.ai`）/ models.default / desc（model_list + source_urls 保留）
5. 验证 JSON 合法
6. 验证：
   ```bash
   python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['claudecn'];print(len(p['model_list']['default']),p['models']['default'],[e['base_url'] for e in p['endpoints']['default']])"
   ```
   预期输出：`7 {'sonnet': 'claude-sonnet-4-6', 'opus': 'claude-opus-4-8', 'haiku': 'claude-haiku-4-5'} ['https://claudecn.ai', 'https://claudecn.ai/v1']`

## 验收（对齐 prd）
- endpoints.default = 2 端点；anthropic host 改为 `claudecn.ai`
- model_list.default = 原 7 个 aidog alias 保留不变（数据局限，不臆造）
- models.default = 3 档位名 key（sonnet / opus / haiku），value 为 model id string
- desc = 8 语言改写
- source_urls 保留
- JSON 合法

## 失败处理
- JSON 解析失败 → 检查逗号 / 引号
- python 校验抛 KeyError → 块名拼写或路径错

## 禁
- 禁动其他协议块
- 禁用 model-id 空 obj 作 models.default value（value 必须 string，对齐 `Partial<Record<ModelSlot, string>>`）
- 禁加 id 日期后缀
- 禁臆造 GPT / Gemini 模型 id 加入 model_list（数据局限）
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
