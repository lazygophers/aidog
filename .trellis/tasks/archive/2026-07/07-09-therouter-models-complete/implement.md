# Implement: therouter model_list+endpoints+desc 全量补全

## 载体

- 单 subtask（单文件 `protocols.therouter` 块）
- trellis-implement 在 task worktree 内内联执行

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.therouter` 块（仅此块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤

1. 读 `research/therouter-models.md`（291 模型 + 28 provider 逐项 + endpoint 实测），**注意文件 531 行，需读全**（分页 Read offset=0/289）
2. 读 `prd.md`（endpoints 补全 + model_list 筛选规则 + 三档默认）
3. 读现有 `protocols.therouter` 块定位
4. 改 `endpoints.default`：补 openai `/v1` + gemini 两端点（保留现有 anthropic）
5. 改 `model_list.default`：按 prd 筛选规则（保留对话/coding/推理/多模态文本，排除纯 embedding/image/audio/video/safety/reward/ocr/search）逐 provider 从 research 提取，`provider/model-id` 单斜杠格式
6. 改 `models.default`：三档（anthropic/claude-sonnet-4.5 / openai/gpt-5.2-codex / deepseek/deepseek-v3.2）
7. 改 `desc` 8 语言（参考 prd 英文/中文，其余 6 语言按现有平台风格翻译）
8. 验证：`python3 -c "import json;json.load(open('src-tauri/defaults/platform-presets.json'))"` 通过
9. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));t=d['protocols']['therouter'];print(len(t['endpoints']['default']),len(t['model_list']['default']),t['models']['default'])"`
   预期输出含三档档位映射：`{'sonnet': 'anthropic/claude-sonnet-4.5', 'coder': 'openai/gpt-5.2-codex', 'default': 'deepseek/deepseek-v3.2'}`

## 验收（对齐 prd）

- [ ] endpoints 3 协议
- [ ] model_list 约 180-220（全量对话/coding/推理）
- [ ] model id `provider/model-id` 单斜杠
- [ ] models.default 三档
- [ ] desc 8 语言
- [ ] JSON 合法
- [ ] 仅改 therouter 块

## 失败处理

- JSON 解析失败 → 检查逗号/引号，修复重验
- model_list 筛选卡壳 → 对照 research 每 provider 表，按 prd 排除规则逐项判
- desc 翻译卡住 → 参考现有 cherryin/shengsuanyun 对应语言风格

## 禁

- 禁动其他协议块
- 禁用裸 id（必须 `provider/model-id`）
- 禁收录纯 embedding/image/audio/video/safety/reward/ocr/search 模型
- 禁动 STATIC_MODEL_IDS
- 禁 git commit（finish hook 处理）
