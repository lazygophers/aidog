# 补全 rightcode model_list+endpoints 全部官方信息

## Goal

RightCode (right.codes) 是 **多供应商按渠道分发平台**（非统一网关，每 CLI 工具/号池独立 prefix 端点），权威源 `/models/public` 公开 API（2026-07-09 实拉）。覆盖 Claude / GPT-5.x Codex / Gemini / DeepSeek V4 / 国产阿里代理（测试中）/ 图像。现有 preset 仅 2 endpoint + 7 claude（**3 个 id 缺日期后缀错误**），缺 gemini + deepseek 端点。需补全 endpoints + 全量稳定模型 + 修正 3 id + 三档默认。**排除官方标注「测试中勿用」的阿里系渠道**。

## Research References

- [`research/rightcode-models.md`](research/rightcode-models.md) — `/models/public` 全量 + 7 渠道 endpoint 路径 + 7 现有核对（3 错）

## Requirements

### 1. endpoints（default 分支，补 gemini + deepseek）

现有 2 endpoint 正确。**新增 gemini + deepseek（openai 协议）**：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://www.right.codes/claude", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://right.codes/codex/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://right.codes/gemini", "client_type": "default"},
    {"protocol": "openai", "base_url": "https://right.codes/deepseek", "client_type": "default"}
  ]
}
```

> 每渠道 base_url 含独立 prefix（`/claude` `/codex/v1` `/gemini` `/deepseek`），非统一网关。deepseek 走 openai 协议（也有 anthropic 变体 `/deepseek/anthropic`，preset 收 openai 即可）。
> claude-aws 逆向渠道不稳定，不收。阿里系渠道测试中，不收 endpoint。

### 2. model_list.default（全量稳定渠道，裸 id 格式，约 27）

**Claude 官渠（9，⚠️ 修正 3 日期后缀 + 补 2 新）**：
- claude-fable-5（新）
- claude-haiku-4-5-20251001（**修正**，现有 `claude-haiku-4-5` 缺后缀）
- claude-opus-4-5-20251101（**修正**，现有 `claude-opus-4-5` 缺后缀）
- claude-opus-4-6 / claude-opus-4-7 / claude-opus-4-8（保留）
- claude-sonnet-4-5-20250929（**修正**，现有 `claude-sonnet-4-5` 缺后缀）
- claude-sonnet-4-6（保留）
- claude-sonnet-5（新）

**Codex 渠道（8，openai 协议）**：
- codex-auto-review / gpt-5.4 / gpt-5.4-high / gpt-5.4-medium / gpt-5.4-mini / gpt-5.4-xhigh / gpt-5.5 / gpt-5.5-openai-compact

**Gemini 渠道（8，gemini 协议，不稳定但官方在售）**：
- gemini-2.5-flash / gemini-2.5-pro / gemini-3-flash-preview / gemini-3-pro-preview / gemini-3.1-pro / gemini-3.1-pro-preview / gemini-3.1-pro-preview-customtools / gemini-3.5-flash

**DeepSeek V4（2，openai 协议）**：
- deepseek-v4-flash / deepseek-v4-pro

合计 **27 模型**。

🔴 **排除**：
- 阿里特供 `/ali-sale`（12 模型，官方 remark「测试中勿用」）
- GLM阿里 `/glm-ali`（4，「测试中勿用」）
- Kimi阿里 `/kimi-ali`（3，「测试中勿用」）
- 画图 `/draw`（6，按次计费非 chat）
- claude-aws 逆向（不稳定）

### 3. models.default（三档默认）

档位名 key → model id string（对齐 `Partial<Record<ModelSlot, string>>`，与 20 官方 protocol 同构）：

```json
"models": {
  "default": {
    "sonnet": "claude-sonnet-5",
    "gpt": "gpt-5.5",
    "default": "deepseek-v4-pro"
  }
}
```

三档：经济 Claude（sonnet-5，$2/$10）/ 旗舰 Codex（gpt-5.5，$5/$30）/ 国产（deepseek-v4-pro，$3/$6 官方同价）。

### 4. desc 改写（8 语言）

现有 desc 若仅写 Claude 兼容则失实。RightCode 多渠道分发：
- en-US: "RightCode multi-vendor API by channel (Claude/GPT-Codex/Gemini/DeepSeek, per-CLI endpoints)"
- zh-Hans: "RightCode 按渠道多供应商分发（Claude/GPT-Codex/Gemini/DeepSeek，每 CLI 独立端点）"
- 其余 6 语言同步翻译

### 5. source_urls.pricing 修正

现有 `https://right.codes/pricing` 返 404。改为 `https://right.codes/models/public`（或 docs 页 `https://docs.right.codes/docs/rc_quick_start/models.html`）。

## Acceptance Criteria

- [ ] endpoints 4（anthropic + openai/codex + gemini + openai/deepseek）
- [ ] 3 claude id 补日期后缀
- [ ] model_list 27（9 claude + 8 codex + 8 gemini + 2 deepseek）
- [ ] models.default 三档
- [ ] desc 8 语言
- [ ] source_urls.pricing 改为 `/models/public`
- [ ] JSON 合法
- [ ] 仅改 rightcode 块

## Out of Scope

- 阿里系测试渠道（即便 id 已知，官方标勿用）
- claude-aws 逆向（不稳定）
- 画图模型（非 chat）
- 上下文窗口字段（不附带在 models.default 内）
- STATIC_MODEL_IDS
- 其他协议块

## Technical Notes

- 真值源：`protocols.rightcode`
- 数据来源：`https://right.codes/models/public`（免鉴权，结构化全量 + 价格）
- id 格式：裸 id（无 `provider/` 前缀），与现有 7 一致
- 每渠道 base_url 含独立 prefix（非统一网关）
- host `www.` vs 裸域等价（CNAME 同后端）
