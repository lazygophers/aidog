# 补全 sensenova model_list+endpoints 全部官方信息

## Goal

商汤日日新 SenseNova。preset 现 model_list 3 项（sensenova-6.7-flash-lite / sensenova-u1-fast / deepseek-v4-flash）+ 2 端点（openai /v1 ✅ + anthropic 根域 ⚠️待确认）。research 核实官方自研在售仅 2 款（sensenova-6.7-flash-lite + sensenova-u1-fast），deepseek-v4-flash 推测为第三方转发。desc「Nova 系列模型」属历史遗留命名（现 Nova-Max/Plus 已退役），需修正为「SenseNova 系列」。改动：model_list 重排（自研前置 / 转发后置）+ desc 改写 + models.default 补 default 档。endpoints 保持现状（anthropic 端点 research 未明确否定，保守保留）。

## Research References

- [`research/sensenova-models.md`](research/sensenova-models.md) — 官方自研 2 款（6.7-flash-lite + u1-fast）+ deepseek-v4-flash 第三方转发推测 + anthropic 端点待确认 + Nova 历史命名 caveat + Token Plan 公测信息

## Requirements

### 1. endpoints（2 端点，保持不动）

openai 端点 research 明确确认正确；anthropic 端点官方文档未提及但保守保留（preset 可能来自实际测试或非公开文档）。

```json
"endpoints": {
  "default": [
    {"protocol": "openai", "base_url": "https://token.sensenova.cn/v1", "client_type": "codex_tui"},
    {"protocol": "anthropic", "base_url": "https://token.sensenova.cn", "client_type": "claude_code"}
  ]
}
```

### 2. model_list.default（3 模型，重排：自研前置 / 转发后置）

官方自研在售 2 款前置（research 明确），第三方转发 deepseek-v4-flash 后置（research 标推测，保留因已在线）。

```json
"model_list": {
  "default": [
    "sensenova-6.7-flash-lite",
    "sensenova-u1-fast",
    "deepseek-v4-flash"
  ]
}
```

### 3. models.default（档位名 key → model id string）

```json
"models": {
  "default": {
    "default": "sensenova-6.7-flash-lite"
  }
}
```

`default` 档 = sensenova-6.7-flash-lite（官方推荐默认，轻量多模态智能体，面向真实工作流，Token Plan 核心模型）。其余 slot（sonnet/opus/haiku/gpt/fable/coder/fast/thinking）均不适用（非 Claude/GPT/带明确 slot 语义的命名）。

### 4. desc 改写（8 语言，修正「Nova 系列」历史命名）

现有「Nova 系列模型」属历史遗留（Nova-Max/Plus 已退役），当前模型为 sensenova-6.7-flash-lite + sensenova-u1-fast。

- zh-Hans: 商汤日日新 API，SenseNova 系列模型
- en-US: SenseTime SenseNova API for SenseNova models
- ar-SA: واجهة برمجة تطبيقات SenseNova من SenseTime لنماذج SenseNova
- fr-FR: API SenseNova de SenseTime pour les modèles SenseNova
- de-DE: SenseTime SenseNova API für SenseNova-Modelle
- ru-RU: API SenseNova от SenseTime для моделей SenseNova
- ja-JP: SenseTime SenseNova API、SenseNova シリーズモデル
- es-ES: API SenseNova de SenseTime para modelos SenseNova

### 5. source_urls（保留，caveat 见 Technical Notes）

保留现状：
```json
"source_urls": {
  "docs": "https://platform.sensenova.cn/document",
  "pricing": "https://platform.sensenova.cn/pricing"
}
```

research 标 docs URL 可能返回 404（地理限制或临时不可达不确定），但无确认替代 URL，保守保留。homepage `https://www.sense_time.com`（含下划线）research 建议改 sensetime.com，但属本 task Out of Scope（homepage 非改动字段）。

## Acceptance Criteria

- [ ] endpoints 2 端点保留不改（research 未明确否定）
- [ ] model_list.default 3 模型，自研前置 + 转发后置（JSON 合法 + 无重复）
- [ ] models.default = {"default": "sensenova-6.7-flash-lite"}
- [ ] desc 8 语言改写（Nova→SenseNova）
- [ ] source_urls 保留
- [ ] JSON 合法
- [ ] 仅改 sensenova 协议块

## Out of Scope

- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan 分支
- homepage 字段（research 建议改 sensetime.com，但非本 task 范围）
- pricing 字段补全（独立 task）
- anthropic 端点实测验证（需 API key，属运行时验证非 preset 编辑）
- 其他协议块改动

## Technical Notes

- 真值源：`protocols.sensenova`（单协议块，无 sensenova_en 镜像）
- 数据来源：research/sensenova-models.md（GitHub OpenSenseNova/SenseNova6.7 API 文档 + Token Plan 页面 + 官方控制台）
- id 格式：裸 id（无 provider/ 前缀），如 `sensenova-6.7-flash-lite`
- anthropic 端点 `https://token.sensenova.cn`（根域无路径）research 标待确认，但与其他协议（如 Kimi 的 /anthropic 前缀）不同，可能是非公开配置。保守保留，禁臆造路径。
- deepseek-v4-flash 来源不明（research 标推测为第三方转发），但已在 preset 中，保留不动避免破坏现网可用性
- Nova 历史命名：Nova-Max/Nova-Plus 已退役，现模型用 sensenova- 前缀，desc 需同步修正
