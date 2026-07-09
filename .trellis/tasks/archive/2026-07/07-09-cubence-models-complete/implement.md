# Implement: cubence model_list+endpoints+desc 补全

## 载体

- 单 subtask（单文件 `protocols.cubence` 块）
- trellis-implement 在 task worktree 内内联执行
- 无 subtask 拆分（轻量 inline）

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.cubence` 块（仅此块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤

1. 读 `research/cubence-models.md`（endpoints 核验 + 模型清单 + 数据局限）
2. 读 `prd.md`（model_list 精确清单 + desc 改写决策）
3. 读现有 `protocols.cubence` 块定位
4. **endpoints 不动**（3 端点已核验正确）
5. 改 `model_list.default`：7 claude（保留）+ gpt-5.5 + gpt-5 + gemini-3-pro-preview = 10 模型
6. 改 `models.default`：三档（claude-sonnet-4-6 / gpt-5.5 / gemini-3-pro-preview）
7. 改 `desc` 8 语言（范围修正，参考 prd 英文/中文文案，其余按现有风格翻译）
8. 验证：`python3 -c "import json;json.load(open('src-tauri/defaults/platform-presets.json'))"` 通过
9. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));c=d['protocols']['cubence'];print(len(c['model_list']['default']),c['models']['default'],c['desc']['en-US'][:50])"`
   预期输出含三档档位映射：`{'sonnet': 'claude-sonnet-4-6', 'gpt': 'gpt-5.5', 'default': 'gemini-3-pro-preview'}`

## 验收（对齐 prd）

- [ ] endpoints 3 端点保留
- [ ] model_list 10 模型（7 claude + gpt-5.5 + gpt-5 + gemini-3-pro-preview）
- [ ] models.default 三档
- [ ] desc 8 语言改写
- [ ] JSON 合法
- [ ] 仅改 cubence 块

## 失败处理

- JSON 解析失败 → 检查逗号/引号，修复重验
- desc 翻译卡住 → 参考现有其他平台 desc 对应语言风格

## 禁

- 禁动其他协议块
- 禁动 endpoints（已正确）
- 禁加 gpt-image-2（图像专用）
- 禁动 STATIC_MODEL_IDS
- 禁 git commit（finish hook 处理）
