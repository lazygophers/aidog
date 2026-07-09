# 补全 deepseek model_list+endpoints 全部官方信息

## Goal

DeepSeek 官方 API 平台（deepseek.com，`default` 客户端），仅 V4 系列（flash + pro）在售。preset 现 model_list 4 项含待弃用别名（deepseek-chat/deepseek-reasoner，2026/07/24 23:59 UTC+8 弃用，距今 15 天）；endpoints 2 端点正确（openai /v1 + anthropic /anthropic）；models.default 单档正确。本次改动：model_list 删 2 别名（4→2），models.default 补 thinking 档（1→2 档），endpoints/desc/source_urls 保留。

## Research References

- [`research/deepseek-models.md`](research/deepseek-models.md) — V4 系列在售清单（line 24-37）+ 别名弃用时间表（line 39-45）+ endpoints 核实（line 64-69）+ models.default 推荐（line 49-59）

## Requirements

### 1. endpoints（default 分支，2 端点，保留不动）

```json
"endpoints": {
  "default": [
    {
      "protocol": "openai",
      "base_url": "https://api.deepseek.com/v1",
      "client_type": "codex_tui"
    },
    {
      "protocol": "anthropic",
      "base_url": "https://api.deepseek.com/anthropic",
      "client_type": "claude_code"
    }
  ]
}
```

research line 64-69 验证两端点正确：`/v1` 实测有效（返认证错误非 404），`/anthropic` 为官方 Anthropic 兼容路径。

### 2. model_list.default（2 模型，裸 id，4→2 删别名）

```json
"model_list": {
  "default": [
    "deepseek-v4-flash",
    "deepseek-v4-pro"
  ]
}
```

- 保留 V4 系列 2 模型（research line 28-37 在售 Stable）
- **删除** `deepseek-chat`：2026/07/24 23:59 UTC+8 弃用，别名映射 → deepseek-v4-flash non-thinking 模式（research line 42）
- **删除** `deepseek-reasoner`：同弃用时间，别名映射 → deepseek-v4-flash thinking 模式（research line 43）
- 不补 V3/R1/Coder/Math（research line 82-90 论证：开源 GitHub 仓库非 API 销售）

### 3. models.default（档位名 key → model id string，1→2 档）

```json
"models": {
  "default": {
    "default": "deepseek-v4-flash",
    "thinking": "deepseek-v4-pro"
  }
}
```

- `default: "deepseek-v4-flash"` 保留（官方首页主推，高并发 2500 vs pro 500，低价，research line 49-58 推荐）
- **新增** `thinking: "deepseek-v4-pro"`：pro 档（更高质量、适合复杂推理任务，pro 价格 3x flash 但质量更高，research line 59 替代选择）。slot 映射 pro/重推理→thinking
- 不补 `fast` 档：default 与 fast 同指 flash 冗余
- 对齐 `Partial<Record<ModelSlot,string>>`

### 4. desc（8 语言，保留不动）

官方平台 desc 准确保留。

### 5. source_urls（保留不动）

```json
"source_urls": {
  "docs": "https://api-docs.deepseek.com/",
  "pricing": "https://api-docs.deepseek.com/quick_start/pricing"
}
```

## Acceptance Criteria

- [ ] model_list.default = ["deepseek-v4-flash","deepseek-v4-pro"]（2 项，删别名）
- [ ] models.default = {"default":"deepseek-v4-flash","thinking":"deepseek-v4-pro"}（2 档 value 全 string）
- [ ] endpoints.default 保持 2 端点不动
- [ ] name/desc/source_urls/homepage/logo_url/client_type 不动
- [ ] platform-presets.json JSON 合法

## Out of Scope

- V3/R1/Coder/Math（开源仓库非 API 销售，research line 82-90）
- deepseek-chat/deepseek-reasoner 别名（2026/07/24 弃用，距今 15 天，主动清理）
- peak_hours / coding_plan 分支
- pricing 字段补全
- id 日期后缀

## Technical Notes

- 真值源：`protocols.deepseek`
- 数据来源：official docs `api-docs.deepseek.com` + pricing 页（2026-07-09 拉取）
- id 格式：裸 id（deepseek-v4-flash / deepseek-v4-pro）
- 别名弃用倒计时：2026/07/24 23:59 UTC+8（research line 42-43），preset 主动清理避免用户踩坑
- 数据强度：强（官方 pricing 页 + thinking_mode 指南 + anthropic_api 指南交叉验证）
