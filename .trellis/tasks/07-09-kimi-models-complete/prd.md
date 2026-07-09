# 补全 kimi model_list+endpoints 全部官方信息

## Goal

Moonshot Kimi（月之暗面）。现 preset `model_list.default` 含 4 模型，其中 2 个已下线（`kimi-k2-thinking` 2026-05-25 下线、`kimi-latest` 2026-01-28 下线）。research 推荐整组替换为 10 模型清单（K2.7 Code 系列 + K2.6/K2.5 + Moonshot V1 完整系列含 vision-preview）。

- **移除**：2 个已下线模型（`kimi-k2-thinking`、`kimi-latest`）
- **新增**：8 个模型（`kimi-k2.7-code`、`kimi-k2.7-code-highspeed` + Moonshot V1 系列 6 个）
- **修正**：Anthropic 端点域名（research line 141-142 官方 Claude Code 集成指南用 `platform.moonshot.cn` 而非当前 `api.moonshot.cn`）
- **更新**：models.default.default 从 `kimi-k2.6` → `kimi-k2.7-code`（research line 85-93 官方首推）

## Research References

- [`research/kimi-models.md`](research/kimi-models.md) — research 全文（模型表 line 19-64 + 推荐清单 line 66-81 + endpoints line 95-141 + caveats line 180-195）

## Requirements

### 1. endpoints（2 端点，修正 Anthropic 域名）

research line 141-142：Anthropic 端点官方 Claude Code 集成指南用 `platform.moonshot.cn` 域名（非 `api.moonshot.cn`）。

```json
"default": [
  {"protocol": "openai", "base_url": "https://api.moonshot.cn/v1", "client_type": "claude_code"},
  {"protocol": "anthropic", "base_url": "https://platform.moonshot.cn/anthropic", "client_type": "claude_code"}
]
```

变更项：
- Anthropic `base_url`：`https://api.moonshot.cn/anthropic` → `https://platform.moonshot.cn/anthropic`（research line 113 + Claude Code 集成指南 line 119-122）
- OpenAI 端点保留不动（`https://api.moonshot.cn/v1`，research line 102 确认正确）

> **注意**：research caveat #2（line 184）标注「建议核实是否为正式域名」。implement 阶段如能验证 `platform.moonshot.cn/anthropic` 可达则用，否则保留 `api.moonshot.cn/anthropic` 并标注。默认推荐用官方文档域名。

### 2. model_list.default（10 模型，`kimi-k*` + `moonshot-v1-*` 格式）

按 research line 66-81 推荐清单整组替换：

```json
"default": [
  "kimi-k2.7-code",
  "kimi-k2.7-code-highspeed",
  "kimi-k2.6",
  "kimi-k2.5",
  "moonshot-v1-8k",
  "moonshot-v1-32k",
  "moonshot-v1-128k",
  "moonshot-v1-8k-vision-preview",
  "moonshot-v1-32k-vision-preview",
  "moonshot-v1-128k-vision-preview"
]
```

变更项：
- **移除**：`kimi-k2-thinking`（2026-05-25 下线，research line 48-55）、`kimi-latest`（2026-01-28 下线，research line 61）
- **新增**：`kimi-k2.7-code`（Stable，research line 27「Kimi 迄今最智能的 Coding 模型」）、`kimi-k2.7-code-highspeed`（Stable，高速版 180+ Tokens/s）、`moonshot-v1-8k` / `moonshot-v1-32k` / `moonshot-v1-128k`（V1 生成系列，区别仅在上下文长度）、`moonshot-v1-8k-vision-preview` / `moonshot-v1-32k-vision-preview` / `moonshot-v1-128k-vision-preview`（V1 Vision 系列）
- **保留**：`kimi-k2.6`（research line 29「Kimi 迄今最智能的模型」，agentic coding + 长上下文推理）、`kimi-k2.5`（research line 30 Agent + 代码 + 视觉通用 SoTA）

### 3. models.default（档位名 key → model id string）

```json
"models": {
  "default": {
    "default": "kimi-k2.7-code",
    "fast": "kimi-k2.7-code-highspeed"
  }
}
```

- `default`：从 `kimi-k2.6` → `kimi-k2.7-code`（research line 85-93 官方首推「Kimi 迄今最智能的 Coding 模型」+ Claude Code 集成指南默认推荐）
- `fast`：`kimi-k2.7-code-highspeed`（高速版 180+ Tokens/s → fast 档，research line 28）

对齐 `Partial<Record<ModelSlot,string>>`。

### 4. desc（保留 8 语言不动）

### 5. source_urls（保留）

```json
"source_urls": {
  "docs": "https://platform.moonshot.cn/docs",
  "pricing": "https://platform.moonshot.cn/docs/pricing/chat"
}
```

pricing 从 `https://platform.moonshot.cn/docs/pricing` → `https://platform.moonshot.cn/docs/pricing/chat`（research line 17 模型定价页更精确路径）。如 implement 阶段发现 `/docs/pricing` 重定向到 `/docs/pricing/chat` 则保留原值。

## Acceptance Criteria

- [ ] model_list.default 含 10 模型（JSON 合法 + 无重复 + 按 research 排序）
- [ ] 不含已下线模型（`kimi-k2-thinking`、`kimi-latest`）
- [ ] models.default.default = `kimi-k2.7-code`（从 `kimi-k2.6` 更新）
- [ ] models.default 含 default/fast 两档
- [ ] Anthropic 端点 base_url = `https://platform.moonshot.cn/anthropic`（或验证后保留原值并标注）
- [ ] OpenAI 端点 base_url 不动（`https://api.moonshot.cn/v1`）
- [ ] desc/name/homepage/logo_url/client_type 不动
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] `yarn build` clean（前端无改动可跳）

## Out of Scope

- 上下文窗口数值字段（research line 26-43 有标注）
- STATIC_MODEL_IDS（passthrough.rs，独立 task）
- peak_hours / coding_plan 分支
- 其他协议块（doubao/byteplus 中聚合的 `kimi-k2.6` / `kimi-k2.7-code`）
- 已下线 K2 系列（`kimi-k2-0905-preview` / `kimi-k2-0711-preview` / `kimi-k2-turbo-preview` / `kimi-k2-thinking` / `kimi-k2-thinking-turbo`，research line 47-55）
- pricing 字段（cost 估算，独立 task）
- temperature 参数限制（research caveat #3，属于运行时行为非 preset 配置）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json` 的 `protocols.kimi`
- 数据来源：research 基于 `https://platform.moonshot.cn/docs/models` + `/docs/api/chat` + `/docs/api/overview` + `/docs/anthropic` + `/docs/guide/claude-code-kimi` 官方文档
- ID 格式：
  - Kimi 系列：`kimi-k<ver>` 或 `kimi-k<ver>-<variant>`（如 `kimi-k2.7-code`、`kimi-k2.7-code-highspeed`、`kimi-k2.6`）
  - Moonshot V1 系列：`moonshot-v1-<context>` 或 `moonshot-v1-<context>-vision-preview`（如 `moonshot-v1-8k`、`moonshot-v1-128k-vision-preview`）
- Moonshot V1 系列区别仅在上下文长度（8k/32k/128k），效果无差异（research line 43）
- Anthropic 端点域名差异（research line 141-142）：OpenAI 端点用 `api.moonshot.cn`，Anthropic 端点 Claude Code 集成指南用 `platform.moonshot.cn`
- Claude Code 集成示例（research line 117-122）：`ANTHROPIC_BASE_URL=https://api.moonshot.cn/anthropic`（注意此处 research 推荐清单 line 127-138 用 `platform.moonshot.cn`，但集成示例 line 119 用 `api.moonshot.cn`，implement 阶段需验证哪个为正式域名）
