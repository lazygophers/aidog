# 补全 xiaomi_mimo model_list+endpoints 全部官方信息

## Goal

小米 MiMo。preset 现 model_list 5 项（mimo-v2.5-pro / mimo-v2-pro / mimo-v2.5 / mimo-v2-omni / mimo-v2-flash），research 核实官方 2026-06-30 已正式弃用 v2 系列 3 款（v2-pro / v2-omni / v2-flash），在售仅 mimo-v2.5-pro + mimo-v2.5 两款。endpoints 2 端点（anthropic /anthropic ✅ + openai /v1 ✅，research 明确确认）配置正确保留。models.default mimo-v2.5-pro 正确保留。desc 准确保留。source_urls 现指向 mimo.xiaomi.com（产品主页非文档），research 给出实际文档/定价 URL，修正。改动：model_list 移除 3 已弃用 + source_urls 修正 + models.default 确认。

## Research References

- [`research/xiaomi-mimo-models.md`](research/xiaomi-mimo-models.md) — 官方弃用公告（2026-06-30 v2 系列下线）+ 双协议端点 Pay-as-you-go 路径核实 + 认证 api-key 请求头（非 Authorization）+ ASR/TTS 非对话模型排除项 + Token Plan 订阅域名变体

## Requirements

### 1. endpoints（2 端点，保持不动）

research line 44-70 明确确认 Pay-as-you-go 模式端点配置正确。anthropic 用 `/anthropic`（非 /v1），openai 用 `/v1`。

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.xiaomimimo.com/anthropic", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.xiaomimimo.com/v1", "client_type": "codex_tui"}
  ]
}
```

### 2. model_list.default（2 在售模型，移除 3 已弃用）

research line 30-36 官方弃用公告：mimo-v2-pro / mimo-v2-omni / mimo-v2-flash 已于 2026-06-30 正式弃用，需移除。仅保留官方在售文本生成模型。

```json
"model_list": {
  "default": [
    "mimo-v2.5-pro",
    "mimo-v2.5"
  ]
}
```

排除项（research line 100-110）：ASR/TTS/voiceclone/voicedesign（语音专用，非对话）+ mimo-v2.5-pro-ultraspeed（内测未公开）。

### 3. models.default（1 档：default）

```json
"models": {
  "default": {
    "default": "mimo-v2.5-pro"
  }
}
```

`default` 档 = mimo-v2.5-pro（官方推荐「复杂推理、深度分析、长文档处理」场景，性能旗舰）。其余 slot（sonnet/opus/haiku/gpt/fable/coder/fast/thinking）均不适用（非 Claude/GPT/带明确 slot 语义的命名）。

### 4. desc（保留，准确）

现有 desc 准确描述平台定位（「小米 MiMo API, MiMo 系列模型」），保留不改。8 语言全保留。

### 5. source_urls（修正）

现 source_urls 指向 `https://mimo.xiaomi.com/`（产品主页），非实际文档/定价页。research line 9-13 给出官方文档/开放平台实际 URL，修正：

```json
"source_urls": {
  "docs": "https://platform.xiaomimimo.com/docs",
  "pricing": "https://platform.xiaomimimo.com/docs/en-US/quick-start/summary/model"
}
```

pricing 暂用模型列表页（含能力 + 计费说明，research line 11），无独立 pricing 页。

## Acceptance Criteria

- [ ] endpoints 2 端点保留（research 确认正确）
- [ ] model_list.default 2 模型（移除 mimo-v2-pro / mimo-v2-omni / mimo-v2-flash 3 已弃用）
- [ ] models.default = {"default": "mimo-v2.5-pro"}
- [ ] desc 保留
- [ ] source_urls 修正（mimo.xiaomi.com → platform.xiaomimimo.com/docs）
- [ ] JSON 合法
- [ ] 仅改 xiaomi_mimo 协议块

## Out of Scope

- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan 分支
- Token Plan 订阅域名（token-plan-cn.xiaomimimo.com，preset 仅表达 Pay-as-you-go 默认模式）
- ASR/TTS/voiceclone/voicedesign 语音模型（非对话）
- mimo-v2.5-pro-ultraspeed（内测未公开）
- 国际版域名（research 标无国际版域名，仅国内）
- pricing 字段补全（独立 task）
- 其他协议块改动

## Technical Notes

- 真值源：`protocols.xiaomi_mimo`（单协议块，无 xiaomi_mimo_en 镜像，research line 143 确认无国际版域名）
- 数据来源：research/xiaomi-mimo-models.md（platform.xiaomimimo.com/docs 官方文档 + 弃用公告 + First API Call 指南 + GitHub rong6/mimo-2api 社区验证）
- id 格式：裸 id（无 provider/ 前缀），如 `mimo-v2.5-pro`
- 认证：api-key 请求头（非 Authorization），Key 格式 `sk-xxxxx`（Pay-as-you-go）或 `tp-xxxxx`（Token Plan）
- 弃用公告权威性：官方文档明确日期 2026-06-30，research line 34-36 直接引用
- 无国际版：仅 `api.xiaomimimo.com`（Pay-as-you-go）+ `token-plan-cn.xiaomimimo.com`（Token Plan），preset 用前者
