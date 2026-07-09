# 补全 sudocode model_list+endpoints 全部官方信息

## Goal
SudoCode（sudocode.us）是多供应商聚合平台（"全球一流 AI 模型聚合服务，一个 API key 多模型通用"），支持 Anthropic / OpenAI / Google / MiniMax / Moonshot / 智谱(GLM) / DeepSeek 7 家供应商，`/api/models` 免鉴权 API 返回全量 29 模型。当前 preset 仅 1 个 anthropic endpoint + 7 个 Claude alias + 空 `models.default` + desc"Claude 兼容模型"严重低估定位。

改动范围：`protocols.sudocode` 单块（endpoints 补 openai+gemini / model_list 全量扩 29 / models.default 填三档 / desc 改写）。source_urls 保留。

## Research References
- [`research/sudocode-models.md`](research/sudocode-models.md) — `/api/models` 免鉴权返回 29 模型（按 vendor_id 分组：Anthropic 9 / Google 7 / OpenAI 5 / MiniMax 2 / Moonshot 2 / GLM 2 / DeepSeek 2）；三类端点（anthropic/openai/gemini）全存活；Claude 系用 aidog 短 alias（与 aicodemirror 模式一致，haiku-4-5 而非 haiku-4-5-20251001）。

## Requirements
### 1. endpoints（default 分支，3 端点：1 保留 + 2 新增）
现有 anthropic 保留。按 research `/v1/chat/completions` + `/v1beta/models/:generateContent` 端点存活，补 openai（带 `/v1`）+ gemini（仅 host）：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://sudocode.us", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://sudocode.us/v1", "client_type": "default"},
    {"protocol": "gemini", "base_url": "https://sudocode.us", "client_type": "default"}
  ]
}
```

> openai base_url 含 `/v1`（符合全局 URL 约定 + research `/v1/chat/completions` 路径核验）；gemini 仅 host（research `/v1beta/models/{model}:generateContent` 由 provider_api_path 处理）。

### 2. model_list.default（29 模型，裸 id，Claude 系用短 alias，全量按 research 落）
按 research `/api/models` 响应全量落。Claude 9 个用 aidog 短 alias（research 明确建议，与 aicodemirror 模式一致），其他供应商用原生 id：

```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
    "claude-fable-5",
    "claude-sonnet-4-5",
    "claude-sonnet-5",
    "claude-opus-4-6",
    "claude-opus-4-7",
    "claude-opus-4-5",
    "gemini-3-pro-preview",
    "gemini-3.5-flash",
    "gemini-3.1-flash-image-preview",
    "gemini-3.1-flash-lite",
    "gemini-3-flash-preview",
    "gemini-3.1-flash-lite-preview",
    "gemini-3.1-pro-preview",
    "gpt-5.4",
    "gpt-5.5",
    "gpt-5.3-codex",
    "gpt-5.4-mini",
    "gpt-image-2",
    "MiniMax-M2.7",
    "MiniMax-M2.5",
    "kimi-k2.7-code",
    "kimi-k2.6",
    "glm-5.1",
    "glm-5.2",
    "deepseek-v4-pro",
    "deepseek-v4-flash"
  ]
}
```

> Claude 9：现有 7（opus-4-8/sonnet-4-6/haiku-4-5/opus-4-7/opus-4-6/opus-4-5/sonnet-4-5）保留短 alias，新增 `claude-fable-5` / `claude-sonnet-5`。research 表中 haiku-4-5-20251001 / opus-4-5-20251101 / sonnet-4-5-20250929 在本 preset 用短 alias（与现有 7 一致，不增删日期后缀）。
> Gemini 7：`gemini-3-pro-preview` / `gemini-3.5-flash` / `gemini-3.1-flash-image-preview` / `gemini-3.1-flash-lite` / `gemini-3-flash-preview` / `gemini-3.1-flash-lite-preview` / `gemini-3.1-pro-preview`（原生 id）。
> OpenAI 5：`gpt-5.4` / `gpt-5.5` / `gpt-5.3-codex` / `gpt-5.4-mini` / `gpt-image-2`（含 1 codex + 1 image-generation，research 表原生 id）。
> MiniMax 2 / Moonshot 2 / GLM 2 / DeepSeek 2：research 表原生 id（`MiniMax-M2.7` 等保留原大小写）。

### 3. models.default（三档，Claude 系内分档，档位名 key → model id 字符串）
`models.default` 是 `Partial<Record<ModelSlot, string>>`，key = 档位名（default/opus/haiku 等），value = model id 字符串：

```json
"models": {
  "default": {
    "default": "claude-sonnet-4-6",
    "opus": "claude-opus-4-8",
    "haiku": "claude-haiku-4-5"
  }
}
```

> default 档（主力兜底）= `claude-sonnet-4-6`（research 推荐性价比，model_ratio=1.5），opus 档（重型）= `claude-opus-4-8`（旗舰，model_ratio=2.5），haiku 档（轻量）= `claude-haiku-4-5`（入门，model_ratio=0.5，短 alias）。三档均在 Claude 系（与其他聚合平台 preset 一致）。

### 4. desc（8 语言改写）
现"Claude-compatible"改写为多供应商聚合定位（7 家供应商）：

- en-US: "SudoCode API - aggregated access to Claude, GPT, Gemini, and domestic models"
- zh-Hans: "SudoCode API - 聚合 Claude / GPT / Gemini / 国产模型，一键调用"
- ar-SA: "واجهة SudoCode - وصول مجمع إلى Claude و GPT و Gemini والنماذج المحلية"
- fr-FR: "API SudoCode - accès agrégé à Claude, GPT, Gemini et modèles nationaux"
- de-DE: "SudoCode-API - aggregierter Zugriff auf Claude, GPT, Gemini und inländische Modelle"
- ru-RU: "API SudoCode — агрегированный доступ к Claude, GPT, Gemini и отечественным моделям"
- ja-JP: "SudoCode API - Claude / GPT / Gemini / 国産モデル統合、ワンクリック呼び出し"
- es-ES: "API de SudoCode - acceso agregado a Claude, GPT, Gemini y modelos nacionales"

### 5. source_urls（保留）
research 确认 docs/pricing 两 URL 存活（200 OK）：

```json
"source_urls": {
  "docs": "https://docs.sudocode.us/",
  "pricing": "https://sudocode.us/pricing"
}
```

## Acceptance Criteria
- [ ] endpoints default = 3 端点（anthropic 保留 + openai 带 `/v1` + gemini 仅 host）
- [ ] model_list.default = 29 模型（Claude 9 短 alias + Gemini 7 + OpenAI 5 + MiniMax 2 + Moonshot 2 + GLM 2 + DeepSeek 2）
- [ ] models.default 三档 = `claude-sonnet-4-6` / `claude-opus-4-8` / `claude-haiku-4-5`，档位名 key（default/opus/haiku）→ model id 字符串
- [ ] desc 8 语言全改写为 7 家供应商聚合定位
- [ ] source_urls 保留
- [ ] JSON 合法，protocols 其他块未动

## Out of Scope
- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan / 其他协议块 / id 日期后缀
- Claude 系短 alias ↔ 完整 id 的映射机制（由平台侧处理）
- group 权限分级（dev/pro/ent/special/test，非 preset 关注）
- 改 name / homepage / logo_url / client_type

## Technical Notes
- 真值源：`src-tauri/defaults/platform-presets.json` 的 `protocols.sudocode` 块（line 2661-2712 区间）
- 数据来源：公开 API `https://sudocode.us/api/models`（免鉴权，29 模型 + vendor_id + endpoint_types，2026-07-09）+ curl 端点存活探测（400+ "无效的令牌"，非 404）
- id 格式：裸 id（无 `provider/` 前缀）；Claude 9 用 aidog 短 alias（`claude-haiku-4-5` 等，与现有 preset 一致，不增删日期后缀）；其他供应商用原生 id（`gpt-5.4` / `gemini-3-pro-preview` / `MiniMax-M2.7` 保留原大小写）
- URL 构造：`base_url + provider_api_path`，openai base_url 含 `/v1`
- Gemini 路径含动态 model 占位符（`/v1beta/models/{model}:generateContent`），由 adapter 层处理
