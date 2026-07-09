# 补全 AtlasCloud model_list+endpoints 全部官方信息

## Goal
AtlasCloud 是聚合路由平台，官方 `/v1/models` 端点公开返回全量 **114 个模型**（15 家 provider），当前 preset 仅 11 项精选且准确率 36%（4/11 匹配），存在大小写错误（moonshotai/MiniMaxAI）和已下架模型。desc 已准确（「聚合路由, 接入多家模型供应商」）保留。本次改动：补全 model_list 至全部 114 款、新增 openai 端点（双协议）、补 models.default 三档、source_urls 保留。

## Research References
- [`research/atlascloud-models.md`](research/atlascloud-models.md) — `curl -s https://api.atlascloud.ai/v1/models` 返回全量 114 模型 ID（provider/model-name 格式）；/v1/messages + /v1/chat/completions 均 401 确认存在；/v1/models 公开 200 OK；现有 11 项仅 4 匹配。

## Requirements

### 1. endpoints（default 分支，2 端点，保留 1 + 新增 1）

保留现有 anthropic 端点，新增 openai 端点（research 方案 B，/v1/chat/completions 401 确认存在）：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.atlascloud.ai", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.atlascloud.ai/v1", "client_type": "codex_tui"}
  ]
}
```

- 保留 anthropic endpoint（base_url 仅 host，框架追加 /v1/messages）
- 新增 openai endpoint（base_url 带 /v1，追加 /chat/completions）
- Gemini 原生端点未检测到，不加

### 2. model_list.default（114 模型，provider/ 前缀格式）

按 provider 分组（直接使用 /v1/models 返回的 id 字段，不做转换）：

**anthropic（20）:**
`anthropic/claude-haiku-4.5-20251001`, `anthropic/claude-haiku-4.5-20251001-coding`, `anthropic/claude-opus-4-20250514`, `anthropic/claude-opus-4-20250514-coding`, `anthropic/claude-opus-4.1-20250805`, `anthropic/claude-opus-4.1-20250805-coding`, `anthropic/claude-opus-4.5-20251101`, `anthropic/claude-opus-4.5-20251101-coding`, `anthropic/claude-opus-4.6`, `anthropic/claude-opus-4.6-coding`, `anthropic/claude-opus-4.7`, `anthropic/claude-opus-4.7-coding`, `anthropic/claude-opus-4.8`, `anthropic/claude-opus-4.8-coding`, `anthropic/claude-sonnet-4-20250514`, `anthropic/claude-sonnet-4-20250514-coding`, `anthropic/claude-sonnet-4.5-20250929`, `anthropic/claude-sonnet-4.5-20250929-coding`, `anthropic/claude-sonnet-4.6`, `anthropic/claude-sonnet-4.6-coding`

**bytedance（10）:**
`bytedance/doubao-seed-1.6-251015`, `bytedance/doubao-seed-1.6-flash-250828`, `bytedance/doubao-seed-1.8-251228`, `bytedance/doubao-seed-2.0-code-preview-260215`, `bytedance/doubao-seed-2.0-lite-260428`, `bytedance/doubao-seed-2.0-mini-260428`, `bytedance/doubao-seed-2.0-pro-260215`, `bytedance/doubao-seed-2.1-pro-260628`, `bytedance/doubao-seed-2.1-turbo-260628`, `bytedance/doubao-seed-evolving`

**deepseek-ai（7）:**
`deepseek-ai/DeepSeek-V3.1`, `deepseek-ai/DeepSeek-V3.1-Terminus`, `deepseek-ai/DeepSeek-V3.2-Exp`, `deepseek-ai/deepseek-ocr`, `deepseek-ai/deepseek-v3.2`, `deepseek-ai/deepseek-v4-flash`, `deepseek-ai/deepseek-v4-pro`

**google（13）:**
`google/gemini-2.0-flash`, `google/gemini-2.0-flash-lite`, `google/gemini-2.5-flash`, `google/gemini-2.5-flash-image`, `google/gemini-2.5-flash-lite`, `google/gemini-2.5-pro`, `google/gemini-3-flash-preview`, `google/gemini-3-pro-image-preview`, `google/gemini-3.1-flash-image`, `google/gemini-3.1-flash-image-preview`, `google/gemini-3.1-flash-lite`, `google/gemini-3.1-pro-preview`, `google/gemini-3.5-flash`

**kwaipilot（3）:**
`kwaipilot/kat-coder-air-v2.5`, `kwaipilot/kat-coder-pro-v2`, `kwaipilot/kat-coder-pro-v2.5`

**meituan-longcat（1）:**
`meituan-longcat/longcat-2.0`

**minimaxai（3）:**
`minimaxai/minimax-m2.5`, `minimaxai/minimax-m2.7`, `minimaxai/minimax-m3`

**moonshotai（3）:**
`moonshotai/kimi-k2.5`, `moonshotai/kimi-k2.6`, `moonshotai/kimi-k2.7-code`

**openai（30）:**
`openai/gpt-4.1`, `openai/gpt-4.1-mini`, `openai/gpt-4.1-nano`, `openai/gpt-4o`, `openai/gpt-4o-mini`, `openai/gpt-5`, `openai/gpt-5-chat`, `openai/gpt-5-codex`, `openai/gpt-5-mini`, `openai/gpt-5-nano`, `openai/gpt-5-pro`, `openai/gpt-5.1`, `openai/gpt-5.1-chat`, `openai/gpt-5.1-codex`, `openai/gpt-5.1-codex-max`, `openai/gpt-5.1-codex-mini`, `openai/gpt-5.2`, `openai/gpt-5.2-chat`, `openai/gpt-5.2-codex`, `openai/gpt-5.3-codex`, `openai/gpt-5.4`, `openai/gpt-5.4-mini`, `openai/gpt-5.4-nano`, `openai/gpt-5.5`, `openai/gpt-image-2`, `openai/o1`, `openai/o3`, `openai/o3-mini`, `openai/o3-pro`, `openai/o4-mini`

**Qwen（1，Q 大写例外）:**
`Qwen/Qwen3-235B-A22B-Instruct-2507`

**qwen（11，q 小写）:**
`qwen/qwen3-vl-235b-a22b-thinking`, `qwen/qwen3.5-122b-a10b`, `qwen/qwen3.5-27b`, `qwen/qwen3.5-35b-a3b`, `qwen/qwen3.5-397b-a17b`, `qwen/qwen3.5-flash`, `qwen/qwen3.5-plus`, `qwen/qwen3.6-35b-a3b`, `qwen/qwen3.6-plus`, `qwen/qwen3.7-max`, `qwen/qwen3.7-plus`

**tencent（1）:**
`tencent/hy3`

**xai（2）:**
`xai/grok-4.3`, `xai/grok-build-0.1`

**xiaomi（2）:**
`xiaomi/mimo-v2.5`, `xiaomi/mimo-v2.5-pro`

**zai-org（7）:**
`zai-org/GLM-4.6`, `zai-org/glm-4.7`, `zai-org/glm-5`, `zai-org/glm-5-turbo`, `zai-org/glm-5.1`, `zai-org/glm-5.2`, `zai-org/glm-5v-turbo`

最终 model_list.default（114 款，完整 JSON 见 research 第 35-179 行）。执行时按 research 全量列表逐条填入。

### 3. models.default（三档，档位名 key → model id string，对齐 `Partial<Record<ModelSlot, string>>` + 20 官方 protocol）

```json
"models": {
  "default": {
    "default": "openai/gpt-5.5",
    "opus": "anthropic/claude-opus-4.8",
    "haiku": "anthropic/claude-haiku-4.5-20251001"
  }
}
```

三档选型理由：
- 主力 `openai/gpt-5.5`：OpenAI 最新旗舰，通用能力强（research 推荐 default） → slot `default`
- 重型 `anthropic/claude-opus-4.8`：Claude 旗舰，复杂推理 → slot `opus`
- 轻量 `anthropic/claude-haiku-4.5-20251001`：Haiku 系列快速响应（research 推荐 fast） → slot `haiku`

### 4. desc（保留）

现有 desc 准确（8 语言已描述「聚合路由, 接入多家模型供应商」），无需改写。

### 5. source_urls（保留）

现有 source_urls 经 research 验证有效（docs / pricing 均可访问）：
```json
"source_urls": {
  "docs": "https://docs.atlascloud.ai/",
  "pricing": "https://atlascloud.ai/pricing"
}
```

## Acceptance Criteria
- [ ] endpoints 2（anthropic 保留 + openai 新增）
- [ ] model_list 114（15 provider 全量，provider/ 前缀格式，大小写与 /v1/models 一致）
- [ ] models.default 三档 档位名 key → string（default=openai/gpt-5.5 / opus=anthropic/claude-opus-4.8 / haiku=anthropic/claude-haiku-4.5-20251001）
- [ ] desc 保留不变
- [ ] source_urls 保留不变
- [ ] JSON 合法
- [ ] 仅改 protocols.atlascloud 块

## Out of Scope
- 上下文窗口字段
- STATIC_MODEL_IDS
- peak_hours / coding_plan 分支
- 其他协议块
- Gemini 原生端点（/v1beta/，未检测到）
- provider 大小写规范化（Qwen/Q 是官方例外，保持原样）

## Technical Notes
- 真值源：protocols.atlascloud
- 数据来源：`curl -s https://api.atlascloud.ai/v1/models`（公开 200 OK，实时全量 114 模型 ID）+ /v1/messages + /v1/chat/completions HEAD 探测（401 确认存在）
- 数据强度：**强**（官方 /v1/models 公开端点返回完整 JSON，无需鉴权）
- id 格式：`provider/model-name` 前缀格式（与 modelscope / novita / openrouter 一致），provider 全小写唯一例外 `Qwen/`（Q 大写是官方规范）
- 现有 11 项仅 4 匹配：deepseek-ai/DeepSeek-V3.2-Exp, deepseek-ai/DeepSeek-V3.1-Terminus, zai-org/GLM-4.6, Qwen/Qwen3-235B-A22B-Instruct-2507；其余 7 项大小写错误或不存在，全量替换
