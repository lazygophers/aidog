# 补全 APIKEY.FUN model_list+endpoints 全部官方信息

## Goal
APIKEY.FUN 是多供应商聚合平台（Universal AI Gateway，首页描述「low-latency access to Claude, ChatGPT, Gemini, and more」），非纯 Claude 代理。现有 preset 仅含 7 个 Claude alias，desc 描述「Claude 兼容模型」失实。**数据局限**：pricing 页仅公开 Claude 模型定价，GPT/Gemini 定价页需登录（302），docs 子页需登录，全量模型清单不可得。本次保守处理：model_list 保留现有 7 alias 不增删（GPT/Gemini 无法穷尽），仅补 models.default 三档 + 修正 desc 8 语言 + 验证 endpoints/source_urls 保留。

## Research References
- [`research/apikeyfun-models.md`](research/apikeyfun-models.md) — pricing 页确认 6 个 Claude 模型（含 claude-sonnet-5 新增）；GPT/Gemini 定价页 302 需登录，模型列表不可得；3 协议 endpoint 全部正确（401 探测确认存在）；docs/faq 子页均需登录。

## Requirements

### 1. endpoints（default 分支，3 端点，全部保留）
现有 3 端点经 research 401 探测全部确认存在且正确：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.apikey.fun", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.apikey.fun/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://api.apikey.fun", "client_type": "default"}
  ]
}
```

### 2. model_list.default（7 模型，裸 id 无 provider/ 前缀，全部保留不变）

**数据局限说明**：pricing 页仅展示 Claude 模型定价（6 款），GPT/Gemini 模型列表需登录查看，本次研究未获取。按保守策略（task 约定：数据弱 → 保留现有 alias 不增删 model_list），维持现有 7 alias：

```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-sonnet-4-5"
  ]
}
```

> 注：research 发现 pricing 页有 `claude-sonnet-5`（preset 缺），且 `claude-opus-4-5` / `claude-sonnet-4-5` 未在 pricing 页展示。但因 GPT/Gemini 全量清单不可得（需登录），整体数据弱，按 task 约定保守不增删 alias。待后续有登录态数据时一并补全 GPT/Gemini + claude-sonnet-5 + 清理旧 alias。

### 3. models.default（三档，档位名 key → model id string，对齐 `Partial<Record<ModelSlot, string>>` + 20 官方 protocol）

```json
"models": {
  "default": {
    "default": "claude-sonnet-4-6",
    "opus": "claude-opus-4-8",
    "haiku": "claude-haiku-4-5"
  }
}
```

三档选型理由：
- 主力 `claude-sonnet-4-6`：平衡型，pricing 页确认存在 → slot `default`
- 重型 `claude-opus-4-8`：pricing 页首位，最强能力 → slot `opus`
- 轻量 `claude-haiku-4-5`：pricing 页最经济（￥0.70/1M 输入） → slot `haiku`

### 4. desc（改写 8 语言）

平台定位从「Claude 兼容模型」改为「多供应商聚合网关（Claude/GPT/Gemini）」：

```json
"desc": {
  "zh-Hans": "APIKEY.FUN 多供应商 AI 网关，支持 Claude/GPT/Gemini 协议",
  "en-US": "APIKEY.FUN universal AI gateway (Claude/GPT/Gemini protocols)",
  "ar-SA": "بوابة APIKEY.FUN الذكاء الاصطناعي الشاملة (بروتوكولات Claude/GPT/Gemini)",
  "fr-FR": "Passerelle AI universelle APIKEY.FUN (protocoles Claude/GPT/Gemini)",
  "de-DE": "APIKEY.FUN universelles AI-Gateway (Claude/GPT/Gemini-Protokolle)",
  "ru-RU": "Универсальный AI-шлюз APIKEY.FUN (протоколы Claude/GPT/Gemini)",
  "ja-JP": "APIKEY.FUN ユニバーサル AI ゲートウェイ（Claude/GPT/Gemini プロトコル）",
  "es-ES": "Puerta de enlace AI universal APIKEY.FUN (protocolos Claude/GPT/Gemini)"
}
```

### 5. source_urls（保留）

现有 source_urls 经 research 验证有效（首页 200 OK / pricing 200 OK）：
```json
"source_urls": {
  "docs": "https://apikey.fun/",
  "pricing": "https://apikey.fun/pricing"
}
```

## Acceptance Criteria
- [ ] endpoints 3（不变）
- [ ] model_list 7（保留现有 alias，不增删）
- [ ] models.default 三档 档位名 key → string（default=claude-sonnet-4-6 / opus=claude-opus-4-8 / haiku=claude-haiku-4-5）
- [ ] desc 8 语言改写为多供应商聚合网关
- [ ] source_urls 保留不变
- [ ] JSON 合法
- [ ] 仅改 protocols.apikeyfun 块

## Out of Scope
- GPT/Gemini 模型补全（需登录态，留后续 task）
- claude-sonnet-5 新增 / claude-opus-4-5 清理（待全量数据后一并处理）
- 上下文窗口字段
- STATIC_MODEL_IDS
- peak_hours / coding_plan 分支
- 其他协议块
- id 日期后缀（alias 约定）

## Technical Notes
- 真值源：protocols.apikeyfun
- 数据来源：首页元描述 + pricing 页 Claude 段（公开）+ 3 协议 endpoint 401 探测；GPT/Gemini 段需登录未获取
- 数据强度：**弱**（GPT/Gemini 全量清单不可得，docs/pricing 子页 302 需登录；仅 Claude 6 款 + 3 endpoint 确认）
- id 格式：裸 id（无 provider/ 前缀），与 aidog 18+ Claude 代理 alias 约定一致
- 保守策略依据：task 约定「research 数据弱 → 保留现有 alias 不增删 model_list，仅补 models.default 三档 + 修 desc/source_urls」
