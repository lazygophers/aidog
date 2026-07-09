# Research: kimi（Moonshot / Kimi）

- **Query**: 核对 kimi 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | openai: `https://api.moonshot.cn/v1`（client_type: claude_code ⚠️ 异常）<br>anthropic: `https://platform.moonshot.cn/anthropic` |
| models.default | default: kimi-k2.7-code, **fast: kimi-k2.7-code-highspeed**（非标） |
| model_list | kimi-k2.7-code, kimi-k2.7-code-highspeed, kimi-k2.6, kimi-k2.5, moonshot-v1-8k/32k/128k, moonshot-v1-8k/32k/128k-vision-preview |

## 官方文档列出值

### Source
- 文档首页：https://platform.moonshot.cn/docs
- 定价：https://platform.moonshot.cn/docs/pricing/chat

### 官方模型（docs 提取）
- **kimi-k2.7-code**（主推 coding 模型，有 quickstart 页）
- **kimi-k2.7-code-highspeed**（高速变体，pricing 页明示）
- **kimi-k2.6**（前代，有 quickstart 页）
- kimi-k2-thinking（思维链模型，docs 有专页）
- moonshot-v1-* 系列（legacy 通用 chat，8k/32k/128k + vision preview）

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| endpoints openai client_type | `claude_code` | OpenAI 兼容路径正常应配 `codex_tui` 或 `default` | **疑似填错**（openai 协议配 claude_code client），ST7 改 `codex_tui` 或核对路由 |
| base_url `api.moonshot.cn` vs `platform.moonshot.cn` | openai 用 api.，anthropic 用 platform. | 官方文档需核实 | `需要: Moonshot 官方 base_url 表（api. vs platform.）` |
| models.default.fast slot | 非标 | D3 删 | **删 fast**，default 已是 k2.7-code |
| model_list 缺 kimi-k2-thinking | 无 | docs 有专页 | 可选补（思维链模型） |
| model_list moonshot-v1 系列 | 有 | ✅ legacy 仍在 | 维持 |
| model_list 缺 kimi-k2.5 的官方确认 | 有 k2.5 | docs 仅见 k2.7-code / k2.6 quickstart，k2.5 未在抓取页面 | `需要: kimi-k2.5 是否仍可调` |

## 补齐建议

1. **D3 删 fast slot**。
2. **endpoints openai client_type 改 `codex_tui`**（与 openai 协议一致；当前 `claude_code` 疑似笔误，会让该 endpoint 走 Anthropic 协议路径，与 protocol=openai 矛盾）。
3. 可选补 `kimi-k2-thinking`（若官方开放 API）。

## Caveats

- Moonshot docs 是 SPA，curl 抓取仅得部分页面。完整模型 + 定价表需登录或翻子页。
- `kimi-k2.5` 在 docs landing 未出现，但 JSON 有 —— 可能是更早版本。`需要: kimi-k2.5 模型卡片 URL`。
