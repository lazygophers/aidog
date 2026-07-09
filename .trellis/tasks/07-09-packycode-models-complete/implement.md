# Implement: packycode model_list+endpoints+desc 全量补全

## 载体

- 单 subtask（单文件 `protocols.packycode` 块）
- trellis-implement 在 task worktree 内内联执行
- 无 subtask 拆分（轻量 inline）

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.packycode` 块（仅此块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤

1. 读 `research/packycode-models.md`（53 模型清单 + 4 endpoint + caveat）
2. 读 `prd.md`（model_list 精确清单 + desc 改写决策）
3. 读现有 `protocols.packycode` 块定位
4. **endpoints 不动**（3 端点已核验正确）
5. 改 `model_list.default`：按 prd 精确清单（Claude 10 / OpenAI 7 / Google 5 / Qwen 9 / GLM 3 / Kimi 3 / MiniMax 3 / MiMo 5 / DeepSeek 2 / Hunyuan 1 = 约 49），3 个 claude id 补日期后缀
6. 改 `models.default`：三档（claude-sonnet-4-6 / gpt-5.4 / glm-5.2）
7. 改 `desc` 8 语言（失实修正，参考 prd 英文/中文文案，其余 6 语言按现有风格翻译）
8. 验证：`python3 -c "import json;json.load(open('src-tauri/defaults/platform-presets.json'))"` 通过
9. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['packycode'];print(len(p['model_list']['default']),p['models']['default'],p['desc']['en-US'][:50])"`
   预期输出含三档档位映射：`{'sonnet': 'claude-sonnet-4-6', 'gpt': 'gpt-5.4', 'default': 'glm-5.2'}`

## 验收（对齐 prd）

- [ ] endpoints 3 端点保留
- [ ] model_list 约 49（全量对话/coding，排除纯图像/审核）
- [ ] 3 个 claude id 补日期后缀
- [ ] models.default 三档
- [ ] desc 8 语言改写
- [ ] JSON 合法
- [ ] 仅改 packycode 块

## 失败处理

- JSON 解析失败 → 检查逗号/引号，修复重验
- model_list 遗漏 → 对照 prd 清单逐供应商核对
- desc 翻译卡住 → 参考现有其他平台 desc 的对应语言风格（如 cherryin/shengsuanyun）

## 禁

- 禁动其他协议块
- 禁动 endpoints（已正确）
- 禁动 STATIC_MODEL_IDS
- 禁 git commit（finish hook 处理）
- 禁照搬 53 全塞（排除图像/审核专用）
