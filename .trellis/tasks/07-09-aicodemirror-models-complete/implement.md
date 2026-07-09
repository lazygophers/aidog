# Implement: aicodemirror models.default 补三档

## 载体

- 单 subtask（单文件 `protocols.aicodemirror` 块，仅改 models.default）
- trellis-implement 在 task worktree 内内联执行
- 轻量改动（1 字段补 3 key）

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.aicodemirror` 块（仅 `models.default` 字段）
- 禁动 endpoints / model_list / desc / 其他协议块 / 顶层 version/last_updated

## 步骤

1. 读 `research/aicodemirror-models.md`（纯 Claude 代理确认 + 7 alias 无需改）
2. 读 `prd.md`（仅补 models.default 三档）
3. 读现有 `protocols.aicodemirror` 块定位
4. endpoints 3 保留（已 401 核验）
5. model_list 7 保留（不增删）
6. desc 保留（"Claude 兼容" 准确）
7. 改 `models.default`：补三档（claude-sonnet-4-6 / claude-opus-4-8 / claude-haiku-4-5）
8. 验证 JSON 合法
9. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));a=d['protocols']['aicodemirror'];print(list(a['models']['default'].keys()),len(a['model_list']['default']),len(a['endpoints']['default']))"`

## 验收（对齐 prd）

- [ ] endpoints 3 保留
- [ ] model_list 7 保留
- [ ] models.default 三档（sonnet-4-6 / opus-4-8 / haiku-4-5）
- [ ] desc 保留
- [ ] JSON 合法
- [ ] 仅改 aicodemirror 块的 models.default

## 失败处理

- JSON 解析失败 → 检查逗号/引号
- 字段定位错 → grep `protocols.aicodemirror` 核行号

## 禁

- 禁动 endpoints / model_list / desc
- 禁补 claude-sonnet-5（平台未确认）
- 禁补 id 日期后缀（alias 约定）
- 禁动其他协议块
- 禁动 STATIC_MODEL_IDS
- 禁 git commit
