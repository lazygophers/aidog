# 补全 openrouter model_list+endpoints 主流旗舰模型

## Goal

OpenRouter 聚合 344 模型 / 57 provider。preset 现 15 项精选旗舰 + `models.default={}` + 三端点（含 gemini）。改动三处：① research 实证 OpenRouter **不支持 gemini 原生协议** → 删 gemini endpoint（3→2）；② `models.default` 从空 obj 扩到 9 档位完整映射（聚合平台覆盖全部 slot）；③ model_list 15→18，补 3 模型（claude-haiku-4.5 / claude-fable-5 / deepseek-r1）以覆盖 haiku/fable/thinking 三个新档位。research 明确反对硬编码全量 344（高频腐化、月级过时），维持"精选旗舰 15-20"策略。

## Research References

- [`research/openrouter-models.md`](research/openrouter-models.md) — 344 模型/57 provider 全量清单（按 provider 分组）；现有 15 preset 模型全部 ✅ 有效；gemini 端点 ❌ 不支持；推荐 default = anthropic/claude-sonnet-4.6；明确建议 preset 维持精选 15-20、运行时拉取全量

## Requirements

### 1. endpoints.default（3 → 2 端点，删 gemini）

research 实证 OpenRouter 仅提供 OpenAI 与 Anthropic 兼容层，Gemini 模型只能通过 OpenAI/Anthropic 协议访问：

```json
"endpoints": {
  "default": [
    { "protocol": "anthropic", "base_url": "https://openrouter.ai/api", "client_type": "claude_code" },
    { "protocol": "openai", "base_url": "https://openrouter.ai/api/v1", "client_type": "codex_tui" }
  ]
}
```

（删除原 `{ "protocol": "gemini", "base_url": "https://openrouter.ai/api", "client_type": "default" }`）

### 2. model_list.default（18 模型，字符串数组，provider/id 前缀格式）

现有 15 + 补 3（覆盖 haiku/fable/thinking 档位），均经 research 确认在 `/api/v1/models` 返回中 ✅：

```json
"model_list": {
  "default": [
    "anthropic/claude-opus-4.8",
    "anthropic/claude-sonnet-4.6",
    "anthropic/claude-haiku-4.5",
    "anthropic/claude-fable-5",
    "openai/gpt-5.5",
    "openai/gpt-5.5-pro",
    "openai/gpt-5.3-codex",
    "google/gemini-3.5-flash",
    "google/gemini-3.1-pro-preview",
    "deepseek/deepseek-v4-pro",
    "deepseek/deepseek-v4-flash",
    "deepseek/deepseek-r1",
    "qwen/qwen3.7-max",
    "z-ai/glm-5.2",
    "moonshotai/kimi-k2.7-code",
    "x-ai/grok-4.3",
    "minimax/minimax-m3"
  ]
}
```

（新增：`anthropic/claude-haiku-4.5`、`anthropic/claude-fable-5`、`deepseek/deepseek-r1`）

排除：全量 344 模型（research 明确反对硬编码，月级腐化）；`:free` 变体；~ 前缀别名（`~anthropic/claude-opus-latest` 等）；非文本主线（图像/音频/嵌入）。

### 3. models.default（档位名 key → model id string，9 档位完整）

聚合平台覆盖全部 9 slot，每档选该 slot 在 OpenRouter 上的旗舰：

```json
"models": {
  "default": {
    "default": "anthropic/claude-sonnet-4.6",
    "opus": "anthropic/claude-opus-4.8",
    "sonnet": "anthropic/claude-sonnet-4.6",
    "haiku": "anthropic/claude-haiku-4.5",
    "fable": "anthropic/claude-fable-5",
    "gpt": "openai/gpt-5.5",
    "coder": "openai/gpt-5.3-codex",
    "fast": "google/gemini-3.5-flash",
    "thinking": "deepseek/deepseek-r1"
  }
}
```

档位依据（research line 73-87 anthropic 清单 / line 279-342 openai 清单 / line 133-163 google 清单 / line 120-132 deepseek 清单）：
- `default`：claude-sonnet-4.6（research 推荐，平衡性能与成本）
- `opus`：claude-opus-4.8（anthropic 最高档 opus 系列最新）
- `sonnet`：claude-sonnet-4.6（同 default，sonnet 系列最新）
- `haiku`：claude-haiku-4.5（haiku 系列最新）
- `fable`：claude-fable-5（fable 系列唯一且最新）
- `gpt`：gpt-5.5（openai 非-mini 主力最新）
- `coder`：gpt-5.3-codex（codex 系列，coder/codex → coder）
- `fast`：gemini-3.5-flash（flash → fast）
- `thinking`：deepseek-r1（r1 推理模型，thinking 档位）

### 4. desc（保留）

8 语言现状准确（"聚合路由, 接入多家模型供应商"），不改写。

### 5. source_urls（保留）

- docs: https://openrouter.ai/docs
- pricing: https://openrouter.ai/docs/models

## Acceptance Criteria

- [ ] `endpoints.default` 数 = 2（仅 anthropic + openai，gemini 已删）
- [ ] `model_list.default` 数 = 18（含新增 claude-haiku-4.5 / claude-fable-5 / deepseek-r1，JSON 合法无重复）
- [ ] `models.default` 为 9 档位完整映射（default/opus/sonnet/haiku/fable/gpt/coder/fast/thinking），每档值为 `provider/id` 字符串
- [ ] desc/source_urls/name/homepage/logo_url/client_type 不动
- [ ] `python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['openrouter'];print(len(p['model_list']['default']),len(p['models']['default']),len(p['endpoints']['default']))"` 输出 `18 9 2`
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] 不动其他协议块、不动顶层 version/last_updated

## Out of Scope

- 全量 344 模型硬编码（research 明确反对，月级腐化，建议运行时拉取 `/api/v1/models`）
- `:free` 变体 / `~` 前缀别名（`~anthropic/claude-opus-latest` 等）
- STATIC_MODEL_IDS（passthrough.rs，独立维护）
- peak_hours / coding_plan 分支
- 其他协议块

## Technical Notes

- 真值源：`protocols.openrouter`
- 数据来源：research/openrouter-models.md（`GET https://openrouter.ai/api/v1/models` 全量返回 + 端点协议支持验证）
- id 格式：`<provider>/<model-id>`（provider 小写，model-id 保留原厂命名）
- 数据强度：**强**（API 实测 344 模型全量返回、端点协议支持实证判定、现有 15 模型全部验证有效）
- 腐化策略：精选 18 旗舰（research 推荐 15-20 区间），覆盖 9 slot + 6 主力 provider；月级腐化需手工核对
