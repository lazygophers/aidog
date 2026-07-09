# 补全 sssaicode model_list+endpoints 全部官方信息

## Goal
SSSAiCode（sssaicode.com）是多供应商聚合平台（Anthropic / OpenAI / DeepSeek 三类协议，官网 `/models` 页面 17 模型），支持多节点布局（HK / HK2 / HK3 / CF US / CF2）。当前 preset 仅 1 个 anthropic endpoint（`node-hk.sssaicodeapi.com/api`）+ 7 个 Claude alias + 空 `models.default` + desc"Claude 兼容模型"低估定位 + source_urls 误指 API 网关域。

改动范围：`protocols.sssaicode` 单块（endpoints 补 openai / model_list 扩 / models.default 填三档 / desc 改写 / source_urls 修正回主站）。

## Research References
- [`research/sssaicode-models.md`](research/sssaicode-models.md) — 三类端点存活 401（"缺少 client token"，非 404）；官网 `/models` 页面 17 模型（Anthropic 7 + OpenAI 5 + DeepSeek 2 + 其他 3）；多节点布局；source_urls 应指 `sssaicode.com` 主站而非 API 网关。

## Requirements
### 1. endpoints（default 分支，2 端点：1 保留 + 1 新增）
现有 anthropic（`node-hk.sssaicodeapi.com/api`）保留。补 openai（复用同一网关，路径 `/api/v1/chat/completions` 已验证存活）。DeepSeek 复用 openai 协议（同一 `/api/v1/chat/completions` 端点），不单列：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://node-hk.sssaicodeapi.com/api", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://node-hk.sssaicodeapi.com/api/v1", "client_type": "default"}
  ]
}
```

> sssaicode 的 URL 构造特殊：网关 base 为 `/api`，anthropic 路径 = `/api + /v1/messages`（provider_api_path 含 /v1），openai 路径 = `/api/v1 + /chat/completions`。故 openai base_url = `https://node-hk.sssaicodeapi.com/api/v1`（含 /v1，符合全局 URL 约定）。HK2/HK3/CF 等备用节点为内部通道，不写入 preset（避免失效）。

### 2. model_list.default（16 模型，裸 id，保留现有 7 + 新增 9）
现有 7 个 Claude alias 保留不删（与 aidog 其他平台 preset 一致，旧版本不强制清理）。新增 research 确认的 2 个最新 Claude + 5 个 OpenAI + 2 个 DeepSeek：

```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-sonnet-4-5",
    "claude-sonnet-5",
    "claude-fable-5",
    "gpt-4o",
    "gpt-4o-mini",
    "gpt-4-turbo",
    "gpt-3.5-turbo",
    "o1-preview",
    "deepseek-chat",
    "deepseek-coder"
  ]
}
```

> Claude 新增：`claude-sonnet-5`（2026-07-01 上线）、`claude-fable-5`（2026-07-02 上线）。
> OpenAI 5 个：research 探测 `/api/v1/chat/completions` 时 model=`gpt-4o` 有效，OpenAI 系基于官网 `/models` 页面 + 通用命名（research 标注为推测）。
> DeepSeek 2 个：探测端点 model=`deepseek-chat` 有效。
> **数据局限**：官网 Cloudflare 保护，OpenAI/DeepSeek 具体 id 基于端点探测 + 通用命名推测，未经 `/v1/models` 全量验证（需 client token）。

### 3. models.default（三档，Claude 系内分档，档位名 key → model id 字符串）
`models.default` 是 `Partial<Record<ModelSlot, string>>`，key = 档位名（default/opus/haiku 等），value = model id 字符串：

```json
"models": {
  "default": {
    "default": "claude-sonnet-5",
    "opus": "claude-opus-4-8",
    "haiku": "claude-haiku-4-5"
  }
}
```

> default 档（主力兜底）= `claude-sonnet-5`（2026-07-01 上线最新主力），opus 档（重型）= `claude-opus-4-8`（旗舰），haiku 档（轻量）= `claude-haiku-4-5`（短 alias 与现有 preset 一致）。

### 4. desc（8 语言改写）
现"Claude-compatible"改写为多供应商聚合定位：

- en-US: "SSSAiCode API - aggregated access to Claude / GPT / DeepSeek"
- zh-Hans: "SSSAiCode API - 聚合 Claude / GPT / DeepSeek 多模型"
- ar-SA: "واجهة SSSAiCode - وصول مجمع إلى Claude و GPT و DeepSeek"
- fr-FR: "API SSSAiCode - accès agrégé à Claude / GPT / DeepSeek"
- de-DE: "SSSAiCode-API - aggregierter Zugriff auf Claude / GPT / DeepSeek"
- ru-RU: "API SSSAiCode — агрегированный доступ к Claude / GPT / DeepSeek"
- ja-JP: "SSSAiCode API - Claude / GPT / DeepSeek 統合アクセス"
- es-ES: "API de SSSAiCode - acceso agregado a Claude / GPT / DeepSeek"

### 5. source_urls（修正）
现 docs/pricing 误指 API 网关域（`node-hk.sssaicodeapi.com/`），修正回官网主站：

```json
"source_urls": {
  "docs": "https://sssaicode.com/docs",
  "pricing": "https://sssaicode.com/pricing"
}
```

> research 明确：docs/pricing 应指 `sssaicode.com` 主站（SPA），API 网关域仅供 API 调用。

## Acceptance Criteria
- [ ] endpoints default = 2 端点（anthropic 保留 `/api` + openai 新增 `/api/v1`）
- [ ] model_list.default = 16 模型（原 7 + claude-sonnet-5 + claude-fable-5 + OpenAI 5 + DeepSeek 2）
- [ ] models.default 三档 = `claude-sonnet-5` / `claude-opus-4-8` / `claude-haiku-4-5`，档位名 key（default/opus/haiku）→ model id 字符串
- [ ] desc 8 语言全改写为 Claude/GPT/DeepSeek 聚合
- [ ] source_urls 修正为 `sssaicode.com` 主站
- [ ] JSON 合法，protocols 其他块未动

## Out of Scope
- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan / 其他协议块 / id 日期后缀
- HK2/HK3/CF 等备用节点 endpoint（内部通道，不写入 preset）
- 删除旧 Claude 版本（4-5/4-6，保留与 aidog 其他平台一致）
- 改 name / homepage / logo_url / client_type

## Technical Notes
- 真值源：`src-tauri/defaults/platform-presets.json` 的 `protocols.sssaicode` 块
- 数据来源：官网 `/models` + `/pricing` + `/install` 页面（Cloudflare 保护，部分 SPA 内容）+ curl 端点存活探测（401 "缺少 client token"），2026-07-09
- 数据局限：OpenAI/DeepSeek 具体 model id 基于端点探测 + 通用命名推测，未经 `/v1/models` 全量验证（需 client token）
- id 格式：裸 id（`claude-opus-4-8` / `gpt-4o` / `deepseek-chat`，无 `provider/` 前缀）
- URL 构造：网关 base 含 `/api` 前缀（平台特殊路径），openai base_url 须含 `/api/v1`
