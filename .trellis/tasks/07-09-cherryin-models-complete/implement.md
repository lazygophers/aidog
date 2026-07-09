# Implement: cherryin model_list+endpoints 全量补全

## 载体

- 单 subtask（单文件 `src-tauri/defaults/platform-presets.json` 的 `protocols.cherryin` 块）
- trellis-implement 在 task worktree 内内联执行
- 无 subtask 拆分（单一文件单一协议块，轻量 inline）

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.cherryin` 块（仅此块）
- 禁动其他协议块、禁动顶层 version/last_updated（除非必要）

## 步骤

1. 读 `research/cherryin-models.md` 全文（全量模型清单 + endpoints 契约）
2. 读 `prd.md`（endpoints/model_list/models.default 决策）
3. 读现有 `platform-presets.json` 的 `protocols.cherryin` 块定位
4. 改 `endpoints.default`：单 anthropic → 3 端点（anthropic 裸 host + openai 含 /v1 codex_tui + gemini 裸 host），见 prd JSON 块
5. 改 `model_list.default`：按 research 各供应商表**逐条提取全部对话模型**（排除纯 embedding/rerank/image-gen 专用），含 agent/ 双 entry + free 模型，grok-4 → x-ai/grok-4
6. 改 `models.default`：三档（anthropic/claude-opus-4.8 / openai/gpt-5.5 / agent/glm-5.2）
7. 验证：`python3 -c "import json;json.load(open('src-tauri/defaults/platform-presets.json'))"` 通过
8. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));c=d['protocols']['cherryin'];print(len(c['model_list']['default']),len(c['endpoints']['default']),list(c['models']['default'].keys()))"` 输出 model_list 条数 / 3 端点 / 三档 key

## 验收（对齐 prd Acceptance Criteria）

- [ ] 3 端点（anthropic / openai 含 /v1 / gemini）
- [ ] model_list 覆盖 research 全部对话模型（预计 ~140 条，排除 ~15 条纯专用）
- [ ] grok-4 → x-ai/grok-4
- [ ] agent/ 双 entry 保留
- [ ] (free) 模型保留
- [ ] models.default 三档
- [ ] JSON 合法
- [ ] 仅改 cherryin 块

## 失败处理

- JSON 解析失败 → 检查逗号/引号，修复重验
- model_list 遗漏 → 对照 research 各表逐供应商核对
- grok-4 残留 → grep 确认已改

## 禁

- 禁动其他协议块
- 禁动 STATIC_MODEL_IDS
- 禁 git commit（finish hook 处理）
- 禁主观筛选旗舰（全量，按 research 表）
