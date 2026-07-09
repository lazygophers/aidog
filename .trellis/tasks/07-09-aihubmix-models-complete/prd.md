# 补全 aihubmix model_list+endpoints 全部官方信息

## Goal

AiHubMix 聚合平台（aihubmix.com，`default` 客户端），裸 id 格式路由 800 模型/13 provider。preset 现 14 项精选模型全部有效（research line 87-103 验证），endpoints 2 端点（anthropic + openai）缺 gemini（research line 67-76 确认 `gemini_api` 支持），models.default 空 `{}` 需补。本次改动：model_list 14 项保留不动，endpoints 补 gemini（2→3），models.default 补 6 档（`{}` → 多档）。

## Research References

- [`research/aihubmix-models.md`](research/aihubmix-models.md) — 800 模型聚合 + 4 协议 endpoints + 14 preset 模型全部有效（line 87）+ gemini_api 支持确认（line 67）

## Requirements

### 1. endpoints（default 分支，2→3 端点，补 gemini）

```json
"endpoints": {
  "default": [
    {
      "protocol": "anthropic",
      "base_url": "https://aihubmix.com",
      "client_type": "claude_code"
    },
    {
      "protocol": "openai",
      "base_url": "https://aihubmix.com/v1",
      "client_type": "codex_tui"
    },
    {
      "protocol": "gemini",
      "base_url": "https://aihubmix.com",
      "client_type": "default"
    }
  ]
}
```

- 前 2 端点保留（research line 124-126 验证正确）
- **新增** gemini 端点：research line 67-76 确认 AiHubMix 支持 `gemini_api`（支持模型含 gemini-3.5-flash / gemini-3.1-pro-preview / claude-sonnet-5 等）。base_url 用根域 `https://aihubmix.com`（对齐 packycode/apinebula/compshare 等同模式聚合平台 gemini 端点惯例，research line 80 推测根域或 /google，取根域更保守且符合 aidog 聚合平台 gemini endpoint 惯例）
- client_type 用 `default`（对齐全部其他平台 gemini 端点）

### 2. model_list.default（14 模型，裸 id，保留不动）

```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "claude-sonnet-4-5",
    "gpt-5.5",
    "gpt-5.5-pro",
    "gpt-5.3-codex",
    "gemini-3.5-flash",
    "gemini-3.1-pro-preview",
    "deepseek-v4-pro",
    "deepseek-v4-flash",
    "qwen3.7-max",
    "glm-5.2",
    "kimi-k2.7-code",
    "grok-4.3"
  ]
}
```

research line 87-103 全部 14 项验证有效，保留。全量 800 模型由前端动态从 `/api/v1/models` 拉取（research line 148-149 建议），preset 只维护精选。

### 3. models.default（档位名 key → model id string，空→6 档）

```json
"models": {
  "default": {
    "default": "claude-sonnet-4-6",
    "opus": "claude-opus-4-8",
    "sonnet": "claude-sonnet-4-6",
    "gpt": "gpt-5.5",
    "coder": "kimi-k2.7-code",
    "fast": "gemini-3.5-flash"
  }
}
```

- `default: "claude-sonnet-4-6"`：主力兜底，Anthropic Sonnet 能力全面（research line 135 推荐 claude-sonnet-5 或 gpt-5.5，preset model_list 未含 sonnet-5，取现有 sonnet-4-6）
- `opus: "claude-opus-4-8"`：slot 映射 opus→opus
- `sonnet: "claude-sonnet-4-6"`：slot 映射 sonnet→sonnet
- `gpt: "gpt-5.5"`：slot 映射 gpt(非mini)→gpt
- `coder: "kimi-k2.7-code"`：slot 映射 coder/codex→coder（preset 含 kimi-k2.7-code + gpt-5.3-codex，取 kimi-code 更专注编程）
- `fast: "gemini-3.5-flash"`：slot 映射 flash/轻量→fast（gemini-3.5-flash 或 deepseek-v4-flash，取 gemini 覆盖更广）
- 对齐 `Partial<Record<ModelSlot,string>>`，禁 model-id 空 obj

### 4. desc（8 语言，保留不动）

官方平台 desc 准确保留。

### 5. source_urls（保留不动）

```json
"source_urls": {
  "docs": "https://docs.aihubmix.com/",
  "pricing": "https://aihubmix.com/pricing"
}
```

## Acceptance Criteria

- [ ] model_list.default 保持 14 项不变
- [ ] models.default = 6 档（default/opus/sonnet/gpt/coder/fast），value 全 string
- [ ] endpoints.default = 3 端点（anthropic + openai + gemini），gemini base_url=`https://aihubmix.com` client_type=default
- [ ] name/desc/source_urls/homepage/logo_url/client_type 不动
- [ ] platform-presets.json JSON 合法

## Out of Scope

- 全量 800 模型写入（research 建议前端动态拉取，preset 维护精选）
- claude-sonnet-5 / 其他新模型补入 model_list（research 未列新增推荐）
- peak_hours / coding_plan 分支
- pricing 字段补全
- id 日期后缀

## Technical Notes

- 真值源：`protocols.aihubmix`
- 数据来源：`https://aihubmix.com/api/v1/models` 端点（OpenAI 兼容，2026-07-09 拉取）
- id 格式：裸 id（无 provider 前缀，research line 41-52 验证）
- gemini 端点 base_url 取根域：对齐 packycode/apinebula/compsame 等聚合平台惯例（research line 80 待确认，取保守根域）
- 数据局限：gemini base_url 路径未经官方文档明示（research line 168），取根域 + client_type=default 符合 aidog 聚合平台惯例
