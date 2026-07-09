# 补全 packycode model_list+endpoints 全部官方信息

## Goal

PackyCode (packyapi.com) 是 **12 供应商聚合平台**（非 Claude-only），官方 `/api/pricing` 公开端点返回全量 **53 模型**（2026-07-09 实拉）。当前 preset `desc` "Claude 兼容模型" 严重失实，model_list 仅 7 个 claude + 空 `models.default`。需按官方真值全量补全，修正 3 个 claude id 缺日期后缀问题，并改写失实 desc。

## Research References

- [`research/packycode-models.md`](research/packycode-models.md) — 53 模型 + 12 供应商 + 4 endpoint 路径 + token group 机制（实拉 `/api/pricing`）

## Requirements

### 1. endpoints（default 分支，3 端点全正确，不动）

现有 3 endpoint 经核验全正确，**保留不改**：
```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://www.packyapi.com", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://www.packyapi.com/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://www.packyapi.com", "client_type": "default"}
  ]
}
```

### 2. model_list.default（全量文本对话/coding 模型，禁遗漏）

从 research 各供应商表提取**全部对话/补全/coding 模型**（排除纯图像/审核专用），裸 id 格式（无 `provider/` 前缀，与现有 7 一致）：

**Claude（10 全量）**：
- claude-fable-5 / claude-haiku-4-5-20251001 / claude-opus-4-1-20250805 / claude-opus-4-5-20251101 / claude-opus-4-6 / claude-opus-4-7 / claude-opus-4-8 / claude-sonnet-4-5-20250929 / claude-sonnet-4-6 / claude-sonnet-5

**OpenAI（7 对话/coding，排除 gpt-image-2 + omni-moderation）**：
- codex-auto-review / gpt-4.1 / gpt-5.3-codex / gpt-5.4 / gpt-5.4-mini / gpt-5.4-pro / gpt-5.5

**Google（6，排除纯 image：gemini-2.5-flash-image/gemini-3-pro-image-preview/gemini-3.1-flash-image-preview）**：
- gemini-2.5-flash / gemini-2.5-pro / gemini-3-flash-preview / gemini-3-pro-preview / gemini-3.1-pro-preview
- 注：保留主流，image 专用排除（research 列了 8 个，减 3 image + gemini-3-pro-image-preview = 留对话/多模态文本主力）

**Qwen（9 全量，vendor_id=8）**：
- qwen3-coder-next / qwen3-max / qwen3-vl-flash / qwen3.5-flash / qwen3.5-plus / qwen3.6-max-preview / qwen3.6-plus / qwen3.7-max / qwen3.7-plus

**GLM（3）**：glm-4.7 / glm-5 / glm-5.2

**Kimi（3）**：kimi-k2.5 / kimi-k2.6 / kimi-k2.7-code

**MiniMax（3）**：MiniMax-M2.7 / MiniMax-M3 / minimax-m2.5

**MiMo（5）**：mimo-v2-flash / mimo-v2-omni / mimo-v2-pro / mimo-v2.5 / mimo-v2.5-pro

**DeepSeek（2）**：deepseek-v4-flash / deepseek-v4-pro

**Hunyuan（1）**：hy3

🔴 **修正现有 3 个缺日期后缀 id**：
- `claude-haiku-4-5` → `claude-haiku-4-5-20251001`
- `claude-opus-4-5` → `claude-opus-4-5-20251101`
- `claude-sonnet-4-5` → `claude-sonnet-4-5-20250929`

合计约 **49 个模型**（53 减图像/审核专用 4 个）。

### 3. models.default（三档默认）

档位名 key → model id string（对齐 `Partial<Record<ModelSlot, string>>`，与 20 官方 protocol 同构）：

```json
"models": {
  "default": {
    "sonnet": "claude-sonnet-4-6",
    "gpt": "gpt-5.4",
    "default": "glm-5.2"
  }
}
```

三档：Claude 主力（sonnet-4-6，cc 组默认可用）/ OpenAI 主力（gpt-5.4，codex 组）/ 国产旗舰（glm-5.2，zai 组）。

### 4. desc 改写（8 语言，失实修正）

现有 desc "Claude 兼容模型" 失实（实为 12 供应商聚合）。改写：
- en-US: "PackyCode multi-vendor AI API aggregator (Claude/GPT/Gemini/Qwen/GLM/Kimi/MiniMax/DeepSeek etc.)"
- zh-Hans: "PackyCode 多供应商 AI API 聚合（Claude/GPT/Gemini/Qwen/GLM/Kimi/MiniMax/DeepSeek 等）"
- 其余 6 语言同步翻译（参考现有 desc 的对应语言风格）

## Acceptance Criteria

- [ ] endpoints 3 端点保留不改（已核验正确）
- [ ] model_list.default 约 49 模型（全量对话/coding，排除纯图像/审核）
- [ ] 3 个 claude id 补日期后缀（haiku-4-5-20251001 / opus-4-5-20251101 / sonnet-4-5-20250929）
- [ ] models.default 三档（claude-sonnet-4-6 / gpt-5.4 / glm-5.2）
- [ ] desc 8 语言改写（失实修正）
- [ ] JSON 合法
- [ ] 仅改 packycode 协议块

## Out of Scope

- 上下文窗口字段（pricing API 不返回，不附带在 models.default 内）
- token group 机制 UI 提示（preset 层不表达，属运行时用户配置）
- 其他协议块改动
- STATIC_MODEL_IDS

## Technical Notes

- 真值源：`protocols.packycode`
- research 数据来源：`https://www.packyapi.com/api/pricing`（免鉴权，53 模型 + 12 供应商）
- model id 格式：裸 id（无 `provider/` 前缀），与现有 7 一致
- token group 机制：单 key 绑单 group，model_list 表达平台全集（非单 key 可用集）
- 同源参考：cherryin（聚合平台 3 端点全量模式）
