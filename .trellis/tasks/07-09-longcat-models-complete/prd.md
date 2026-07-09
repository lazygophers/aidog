# 补全 longcat model_list+endpoints 全部官方信息

## Goal

美团 LongCat 自研平台，官方当前仅 `LongCat-2.0` 一个模型（1.6T MoE, 1M 上下文）。preset 现 `model_list.default=[]` + `models.default={}`。补全单模型 + 验证 endpoints（research 疑多 `/v1`，交叉验证后判定现有配置符合项目 URL 构造约束，保留不动）。改动范围仅 `protocols.longcat` 块的 `model_list` + `models`，不动 endpoints/desc/source_urls。

## Research References

- [`research/longcat-models.md`](research/longcat-models.md) — 单一自研模型平台（非聚合），官方仅 `LongCat-2.0`；OpenAI base_url `https://api.longcat.chat/openai` + `/v1/chat/completions`，Anthropic base_url `https://api.longcat.chat/anthropic` + `/v1/messages`

## Requirements

### 1. endpoints.default（2 端点，保留不动）

研究文档曾质疑 OpenAI base_url `https://api.longcat.chat/openai/v1` 多 `/v1`，但按项目 URL 构造约束（CLAUDE.md：`base_url` 含版本前缀，`provider_api_path()` 只返回 `/chat/completions`，最终 URL = `base_url + provider_api_path`），现有配置拼接结果 `https://api.longcat.chat/openai/v1/chat/completions` 与官方文档 cURL 示例一致。**结论：保留不动。**

```json
"endpoints": {
  "default": [
    { "protocol": "anthropic", "base_url": "https://api.longcat.chat/anthropic", "client_type": "claude_code" },
    { "protocol": "openai", "base_url": "https://api.longcat.chat/openai/v1", "client_type": "codex_tui" }
  ]
}
```

### 2. model_list.default（1 模型，字符串数组）

```json
"model_list": { "default": ["LongCat-2.0"] }
```

### 3. models.default（档位名 key → model id string）

```json
"models": { "default": { "default": "LongCat-2.0" } }
```

### 4. desc（保留）

8 语言现状准确（"Claude 兼容模型"），不改写。

### 5. source_urls（保留）

- docs: https://longcat.chat/
- pricing: https://longcat.chat/pricing

## Acceptance Criteria

- [ ] `model_list.default = ["LongCat-2.0"]`（JSON 合法、无重复）
- [ ] `models.default = {"default":"LongCat-2.0"}`（档位名 key → string，非空 obj）
- [ ] endpoints/default/desc/source_urls/name/homepage/logo_url 全部不动
- [ ] `python3 -c "import json;d=json.load(open('src-tauri/defaults/platform-presets.json'));p=d['protocols']['longcat'];print(len(p['model_list']['default']),p['models']['default'],len(p['endpoints']['default']))"` 输出 `1 {'default': 'LongCat-2.0'} 2`
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] 不动其他协议块、不动顶层 version/last_updated

## Out of Scope

- 上下文窗口字段（无 pricing/context 字段 schema）
- STATIC_MODEL_IDS（passthrough.rs，独立维护）
- peak_hours / coding_plan 分支（本 task 仅 default 分支）
- 其他协议块
- 模型 id 加日期后缀

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json` 的 `protocols.longcat`
- 数据来源：research/longcat-models.md（官方文档 https://longcat.chat/platform/docs/）
- id 格式：`LongCat-2.0`（PascalCase + 点分版本，官方唯一)
- 数据强度：**强**（单一模型、官方文档明确、cURL 示例验证）
