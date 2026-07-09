# 补全 codex model_list+endpoints 全部官方信息

## Goal

OpenAI Codex CLI 官方端点（`codex_tui` 客户端，`openai_responses` 协议）。官方模型页推荐 4 模型（gpt-5.5 首选 + gpt-5.4 + gpt-5.4-mini + gpt-5.3-codex-spark research preview），preset 现 model_list 仅 3 项（缺 spark）；endpoints 现单端点正确，research 论证不扩。本次改动：model_list 补 1 项（3→4），models.default 补 fast 档（gpt→gpt+fast），endpoints/desc/source_urls 保留。

## Research References

- [`research/codex-models-endpoints.md`](research/codex-models-endpoints.md) — 官方推荐 4 模型清单（line 119）+ endpoints 保持单端点论证（line 131）+ spark 渠道 caveat（line 147）

## Requirements

### 1. endpoints（default 分支，1 端点，保留不动）

```json
"endpoints": {
  "default": [
    {
      "protocol": "openai_responses",
      "base_url": "https://api.openai.com/v1",
      "client_type": "codex_tui"
    }
  ]
}
```

research line 131 论证：aidog 是代理路由器，ChatGPT OAuth 流程由 codex CLI 客户端自身驱动不经代理层；数据驻留/第三方 provider 走 `platform.extra`。单端点足够。

### 2. model_list.default（4 模型，裸 id，3→4）

```json
"model_list": {
  "default": [
    "gpt-5.5",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.3-codex-spark"
  ]
}
```

- 前 3 项保留（官方推荐三件套，覆盖默认/旗舰/mini）
- **新增** `gpt-5.3-codex-spark`：官方「Recommended models」段第四项，research preview（仅 ChatGPT Pro，无 API Access 列）。补入仅影响下拉展示，不影响路由（research line 127-129）
- 不补 `gpt-5.2` / `gpt-5.3-codex`（官方明示弃用）

### 3. models.default（档位名 key → model id string）

```json
"models": { "default": { "gpt": "gpt-5.5", "fast": "gpt-5.4-mini" } }
```

- `gpt: "gpt-5.5"` 保留（官方文档示例 `model = "gpt-5.5"` 一致，slot 映射 gpt→gpt）
- **新增** `fast: "gpt-5.4-mini"`：mini → fast 档（slot 映射规则 flash/mini/nano/轻量→fast）
- 不补 `coder` 档：spark 仅 ChatGPT Pro research preview 无 API Access，作 coder 档会误导 API key 用户
- 对齐 `Partial<Record<ModelSlot,string>>`，禁 model-id 空 obj

### 4. desc（8 语言，保留不动）

官方平台 desc 准确保留。

### 5. source_urls（保留不动）

```json
"source_urls": {
  "docs": "https://platform.openai.com/docs/guides/codex",
  "pricing": "https://openai.com/api/pricing/"
}
```

research 真值源为 `developers.openai.com/codex/*`（line 6-11），preset 现 `platform.openai.com` 仍有效，保留。

## Acceptance Criteria

- [ ] model_list.default = ["gpt-5.5","gpt-5.4","gpt-5.4-mini","gpt-5.3-codex-spark"]（4 项，JSON 合法无重复）
- [ ] models.default = {"gpt":"gpt-5.5","fast":"gpt-5.4-mini"}（档位名 key→string）
- [ ] endpoints.default 保持单端点不动
- [ ] name/desc/source_urls/homepage/logo_url/client_type 不动
- [ ] platform-presets.json JSON 合法（python3 json.load 通过）

## Out of Scope

- ChatGPT OAuth/数据驻留端点（research line 131 论证不增）
- peak_hours / coding_plan 分支
- STATIC_MODEL_IDS（passthrough.rs 独立维护，本 task 不碰）
- pricing 字段补全（独立 task）
- id 日期后缀

## Technical Notes

- 真值源：`protocols.codex`（`src-tauri/defaults/platform-presets.json`）
- 数据来源：official docs `developers.openai.com/codex/models` + `/codex/auth` + `/codex/config-advanced`
- id 格式：裸 id（gpt-5.x 系列）
- `gpt-5.3-codex-spark` 渠道局限：API key 认证下可能不可调（research line 147），ChatGPT Pro 订阅可用；补 model_list 仅影响下拉展示
