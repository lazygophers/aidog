# 补全 cubence model_list+endpoints 全部官方信息

## Goal

Cubence (cubence.com) 是 **Claude + GPT + Gemini 三大 AI 一站式代理**（非 Claude-only，无国产模型）。当前 preset `desc` "Claude 兼容模型" 低估范围，model_list 仅 7 个 claude + 空 `models.default`。官方文档无独立 Models 页（FAQ JS 渲染抓不到），按 setup 页 + OpenClaw 配置示例明文提及的模型补全，标注数据局限。

## Research References

- [`research/cubence-models.md`](research/cubence-models.md) — 三协议 endpoints（4 等价线路）+ 模型清单（文档明文抽取）+ 数据局限 caveat

## Requirements

### 1. endpoints（default 分支，3 端点全正确，不动）

现有 3 endpoint 经核验全正确（OpenClaw 文档佐证 "Codex baseUrl must end with /v1; Claude does not"），**保留不改**：
```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.cubence.com", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.cubence.com/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://api.cubence.com", "client_type": "default"}
  ]
}
```

### 2. model_list.default（文档明文 + 现有保留，裸 id 格式）

官方文档无 Models 页，按明文提及 + 现有保留补全（数据局限已在 research 标注）：

**Claude 系（7，保留现有；仅 claude-opus-4-7 文档明文 ✅，其余 6 个 ⚠️ 推测可用）**：
- claude-opus-4-8 / claude-sonnet-4-6 / claude-haiku-4-5 / claude-opus-4-7 / claude-opus-4-6 / claude-opus-4-5 / claude-sonnet-4-5

**OpenAI 系（2，文档明文 ✅）**：
- gpt-5.5（Codex config.toml 官方默认）/ gpt-5（Codex setup 描述明文）

**Google 系（1，文档明文 ✅）**：
- gemini-3-pro-preview（Gemini CLI .env 官方默认）

合计 **10 模型**。

🔴 **排除** `gpt-image-2`（图像生成专用，非聊天代理路由范围，且需单独 share group）。

### 3. models.default（三档默认，档位名 key → model id string）

档位名 key → model id string（对齐 `Partial<Record<ModelSlot, string>>`，与 20 官方 protocol 同构）：

```json
"models": {
  "default": {
    "sonnet": "claude-sonnet-4-6",
    "gpt": "gpt-5.5",
    "default": "gemini-3-pro-preview"
  }
}
```

三档：Claude sonnet 档（官方未指明具体 id，按现有 sonnet-4-6）/ OpenAI（gpt-5.5 官方 config.toml 默认 ✅）/ Gemini（gemini-3-pro-preview 官方 .env 默认 ✅）。

### 4. desc 改写（8 语言，范围修正）

现有 desc "Claude 兼容模型" 低估（实为 Claude/GPT/Gemini 三协议代理）。改写：
- en-US: "Cubence one-stop proxy for Claude, GPT, and Gemini models"
- zh-Hans: "Cubence 一站式代理 Claude、GPT、Gemini 三大模型"
- 其余 6 语言同步翻译

## Acceptance Criteria

- [ ] endpoints 3 端点保留不改（已核验正确）
- [ ] model_list.default 10 模型（7 claude + gpt-5.5 + gpt-5 + gemini-3-pro-preview）
- [ ] models.default 三档（claude-sonnet-4-6 / gpt-5.5 / gemini-3-pro-preview）
- [ ] desc 8 语言改写（范围修正）
- [ ] JSON 合法
- [ ] 仅改 cubence 协议块

## Out of Scope

- 上下文窗口字段（文档仅给 claude-opus-4-7=200000，其余未知，不附带在 models.default 内）
- gpt-image-2（图像专用，排除）
- 备用线路 endpoint（api-dmit/api-bwg/api-cf，非必要）
- 官方文档未罗列的模型（数据局限，需用户控制台核实，本研究已穷尽文档明文）
- 其他协议块改动

## Technical Notes

- 真值源：`protocols.cubence`
- 数据局限：官方无 Models 页，FAQ JS 渲染抓不到；model id 来自 setup 默认值 + OpenClaw 示例 + 现有 preset，**可能不完整**（research Caveats 已标注）
- id 格式：裸 id（无 `provider/` 前缀）
- share group 机制：API Key 需为每服务类型指派 share group，Max 组为高档
- 同源参考：packycode（聚合平台 desc 改写模式）
