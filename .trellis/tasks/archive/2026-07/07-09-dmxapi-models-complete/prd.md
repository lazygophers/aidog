# 补全 dmxapi model_list+endpoints 主流模型

## Goal

DMXAPI 聚合平台（dmxapi.cn，`default` 客户端），裸 id 格式路由多 provider。preset 现 11 项精选模型含 1 历史版本（claude-opus-4-5-20251101），endpoints 2 端点（anthropic + openai）缺 gemini（research line 100-114 推测支持），models.default 空 `{}` 需补。本次改动：model_list 删历史版本（11→10），models.default 补 6 档，endpoints/desc/source_urls 保留。

**数据局限**：research 无法通过 `/v1/models` 认证（research line 11），全量模型清单为推测；gemini 端点支持基于文档导航链接推测（research line 100-114），base_url 路径未验证。本 prd 取保守策略——不扩 model_list、不加 gemini 端点（需 implement 阶段或独立验证 task 确认后补）。

## Research References

- [`research/dmxapi-models.md`](research/dmxapi-models.md) — preset 11 项核实（line 77-89）+ endpoints 核实（line 92-99）+ gemini 协议推测（line 100-114）+ models.default 建议（line 127-138）+ caveats（line 162-167）

## Requirements

### 1. endpoints（default 分支，2 端点，保留不扩 gemini）

```json
"endpoints": {
  "default": [
    {
      "protocol": "anthropic",
      "base_url": "https://www.dmxapi.cn",
      "client_type": "claude_code"
    },
    {
      "protocol": "openai",
      "base_url": "https://www.dmxapi.cn/v1",
      "client_type": "codex_tui"
    }
  ]
}
```

- 2 端点保留（research line 92-99 验证正确）
- **不补 gemini 端点**：research line 100-114 + 162-167 明确 gemini 支持仅基于文档导航推测，base_url 路径（根域 vs /v1）未验证，且 `/v1/models` 端点需认证无法确认。保守策略——待独立验证 task 获取有效 API Key 后补
- 若 implement 阶段可验证 gemini 端点可达（实测请求 `https://www.dmxapi.cn` + gemini 协议路径返回非 404），可追加：
  ```json
  {"protocol":"gemini","base_url":"https://www.dmxapi.cn","client_type":"default"}
  ```

### 2. model_list.default（10 模型，裸 id，11→10 删历史版本）

```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "deepseek-v4-pro",
    "deepseek-v4-flash",
    "gpt-5.5",
    "gpt-5.3-codex",
    "gemini-3.5-flash",
    "gemini-3.1-pro-preview",
    "glm-5.2",
    "kimi-k2.7-code"
  ]
}
```

- 保留 10 项有效模型（research line 77-89 验证）
- **删除** `claude-opus-4-5-20251101`：research line 81 标注「⚠ 历史版本，2025-11-01 版本，可能已过期」，已有最新 `claude-opus-4-8` 覆盖，历史版本误导用户
- **不扩**：research line 36-50 推测全量约 20 项但 caveats line 162-167 明确「无法通过 /v1/models 获取，以上为推测」，保守不补

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

- `default: "claude-sonnet-4-6"`：主力兜底，能力全面
- `opus: "claude-opus-4-8"`：slot 映射 opus→opus
- `sonnet: "claude-sonnet-4-6"`：slot 映射 sonnet→sonnet
- `gpt: "gpt-5.5"`：slot 映射 gpt(非mini)→gpt
- `coder: "kimi-k2.7-code"`：slot 映射 coder/codex→coder（kimi-k2.7-code 专注编程）
- `fast: "gemini-3.5-flash"`：slot 映射 flash/轻量→fast
- 对齐 `Partial<Record<ModelSlot,string>>`，禁 model-id 空 obj
- **禁** 使用 research line 128-138 推荐的 `gemini`/`deepseek`/`glm`/`kimi` 等 key（非合法 ModelSlot）

### 4. desc（8 语言，保留不动）

### 5. source_urls（保留不动）

```json
"source_urls": {
  "docs": "https://docs.dmxapi.cn/",
  "pricing": "https://www.dmxapi.cn/pricing"
}
```

## Acceptance Criteria

- [ ] model_list.default = 10 项（无 claude-opus-4-5-20251101）
- [ ] models.default = 6 档（default/opus/sonnet/gpt/coder/fast），value 全 string
- [ ] endpoints.default = 2 端点保留（除非 implement 阶段实测 gemini 可达再补）
- [ ] name/desc/source_urls/homepage/logo_url/client_type 不动
- [ ] platform-presets.json JSON 合法

## Out of Scope

- gemini 端点补全（需独立验证 task 获取 API Key 实测）
- 全量模型清单扩（research 数据局限，需认证后拉取）
- peak_hours / coding_plan 分支
- pricing 字段补全
- id 日期后缀

## Technical Notes

- 真值源：`protocols.dmxapi`
- 数据来源：preset 现状 + aihubmix 对比推测 + 文档导航结构（research line 11 明确 `/v1/models` 需认证无法获取）
- id 格式：裸 id（preset 现状 + aihubmix 对比验证，research line 66-73）
- **数据强度：弱**（research line 162-167 全量清单无法验证、gemini 支持仅推测、认证方式未明示），本 prd 取保守策略（不扩不补 gemini），仅删确认的历史版本 + 补 models.default 档位映射
- 独立验证建议：注册 DMXAPI 账号获取 API Key → 测试 `/v1/models` + gemini 端点路径
