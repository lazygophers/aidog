# Implement: sensenova model_list 重排 + desc 改写 + models.default 补档

## 载体

- 单 subtask（单文件 `protocols.sensenova` 块，无 sensenova_en 镜像）
- trellis-implement 在 task worktree 内内联执行
- 无 subtask 拆分（轻量 inline）

## 工作目录与范围

- 改 `src-tauri/defaults/platform-presets.json` 的 `protocols.sensenova` 块（仅此块）
- 禁动其他协议块、顶层 version/last_updated

## 步骤

1. 读 `research/sensenova-models.md`（官方自研 2 款 + deepseek-v4-flash 转发推测 + anthropic 端点待确认 + Nova 历史命名 caveat）
2. 读 `prd.md`（model_list 重排 + desc 改写 8 语言 + models.default 补 default 档）
3. 读现有 `protocols.sensenova` 块定位（research line 9-45 已贴现状）
4. **endpoints 不动**（2 端点，openai ✅ + anthropic ⚠️保守保留）
5. 改 `model_list.default`：重排为自研前置 + 转发后置 → `["sensenova-6.7-flash-lite", "sensenova-u1-fast", "deepseek-v4-flash"]`
6. 改 `models.default`：`{"default": "sensenova-6.7-flash-lite"}`
7. 改 `desc` 8 语言（Nova→SenseNova，参考 prd 英文/中文文案，其余 6 语言按现有风格翻译）
8. 验证：`python3 -c "import json;json.load(open('src-tauri/defaults/platform-presets.json'))"` 通过
9. 验证：`python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['sensenova'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']),p['desc']['en-US'][:60])"`

## 验收（对齐 prd）

- [ ] endpoints 2 端点保留
- [ ] model_list 3 模型，自研前置 + 转发后置
- [ ] models.default = {"default": "sensenova-6.7-flash-lite"}
- [ ] desc 8 语言改写（Nova→SenseNova）
- [ ] JSON 合法
- [ ] 仅改 sensenova 块

## 失败处理

- JSON 解析失败 → 检查逗号/引号，修复重验
- desc 翻译卡住 → 参考现有其他平台 desc 的对应语言风格（如 kimi/glm 的 ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES 措辞）
- anthropic 端点疑虑 → 不改，保守保留（research 未明确否定，可能是非公开配置）

## 禁

- 禁动其他协议块
- 禁动 endpoints（保守保留）
- 禁动 STATIC_MODEL_IDS
- 禁动 homepage（research 建议改 sensetime.com，但属 Out of Scope）
- 禁动 source_urls（保留现状）
- 禁 git commit（finish hook 处理）
- 禁用 model-id 空 obj（必须档位名 key → string）
- 禁臆造 anthropic 端点路径（如 /v1/messages）
- 禁删 deepseek-v4-flash（虽是转发推测，但已在线，保留避免破坏）
