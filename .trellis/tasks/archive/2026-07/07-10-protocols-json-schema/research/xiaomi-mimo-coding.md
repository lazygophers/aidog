# Research: 小米 MiMo Token Plan (`xiaomi_mimo_coding`)

- **Query**: 为新增 `xiaomi_mimo_coding` 独立协议调研 MiMo Token Plan 真实 API 字段
- **Scope**: external（官方文档）+ internal（现有 preset 结构对照）
- **Date**: 2026-07-10

## Findings

### base_url

Token Plan（套餐）使用**独立 host** `token-plan-{region}.xiaomimimo.com`，与普通版 `api.xiaomimimo.com` 不同。三集群：

| 集群 | OpenAI 协议 base_url | Anthropic 协议 base_url |
|---|---|---|
| 中国大陆 (cn) | `https://token-plan-cn.xiaomimimo.com/v1` | `https://token-plan-cn.xiaomimimo.com/anthropic` |
| 新加坡 (sgp) | `https://token-plan-sgp.xiaomimimo.com/v1` | `https://token-plan-sgp.xiaomimimo.com/anthropic` |
| 欧洲 (ams) | `https://token-plan-ams.xiaomimimo.com/v1` | `https://token-plan-ams.xiaomimimo.com/anthropic` |

> 推荐 preset 默认填 **cn 集群**（与现有普通版默认 China 一致，社区分享帖典型形态也是 `token-plan-cn`，见 `src/utils/platformPaste.test.ts:278`）。

对照普通版（`src-tauri/defaults/platform-presets.json:348-365`）：
- OpenAI: `https://api.xiaomimimo.com/v1`
- Anthropic: `https://api.xiaomimimo.com/anthropic`

### provider_api_path

- OpenAI 协议: `/chat/completions`（curl 示例 `BASE_URL/chat/completions`，[quick-access.md](https://platform.xiaomimimo.com/static/docs/price/tokenplan/quick-access.md) line ~353）
- Anthropic 协议: `/v1/messages`（curl 示例 `BASE_URL/v1/messages`，其中 BASE_URL 已含 `/anthropic`，最终 URL = `/anthropic/v1/messages`）

> 与普通版 path 一致，仅 host 不同。Rust 侧 `provider_api_path()` 不需改动。

### cp 独占模型

**Token Plan 无独占模型**——套餐覆盖的模型与普通版相同，均为 MiMo 旗舰模型：

- `mimo-v2.5-pro`（默认）
- `mimo-v2.5`
- `mimo-v2.5-asr`（语音识别）
- `mimo-v2.5-tts` / `mimo-v2.5-tts-voiceclone` / `mimo-v2.5-tts-voicedesign`（语音合成，限时免费）
- 兼容旧版 `mimo-v2-pro` / `mimo-v2-omni`（自动转发到 V2.5；2026-06-30 停服）

> 长上下文：支持 1M 上下文的模型可在 model id 后缀 `[1m]` 启用，如 `mimo-v2.5-pro[1m]`（[claudecode.md](https://mimo.mi.com/static/docs/tokenplan/integration/claudecode.md)）。

> 套餐的本质区别在**计费方式**（Credits 配额制）而非模型集合：所有套餐共享同一池子 Credits，套餐内多模型并行消耗（不同 token 类型按比率扣 Credits，参见下方 Credit 表）。非独占 coding 模型——这一点与 `glm_coding`（GLM Coding Plan 独立模型 `glm-4.6-coding`）不同。

### client_type

与普通版一致：
- Anthropic 协议端点 → `claude_code`
- OpenAI 协议端点 → `codex_tui`（Codex CLI / Responses API；MiMo Token Plan 已支持 Responses API，[codex-configuration.md](https://mimo.mi.com/static/docs/tokenplan/integration/codex-configuration.md)）

### 鉴权（tp- 前缀含义）

**核心区别**：

| 项 | 普通版 (Pay-as-you-go) | Token Plan（套餐） |
|---|---|---|
| API Key 前缀 | `sk-xxxxx` | `tp-xxxxx`（**t**oken **p**lan） |
| 计费 | 按 token 单价 | 月/年套餐固定费 + Credits 配额 |
| Key 生命周期 | 充值即用 | **仅订阅有效期内可用**，过期失效 |
| 两套 key 关系 | 互相独立，**不可混用**（[quick-access.md](https://platform.xiaomimimo.com/static/docs/price/tokenplan/quick-access.md) L318-322） | 同 |

**鉴权头规范**（[quick-access.md](https://platform.xiaomimimo.com/static/docs/price/tokenplan/quick-access.md) L352-393）：MiMo 自定义要求 **`api-key: <key>`** 头，**OpenAI 与 Anthropic 协议均使用此头**——并非 OpenAI 标准 `Authorization: Bearer`，也不是 Anthropic 标准 `x-api-key`。官方 curl 示例两种协议都只发 `--header "api-key: $MIMO_API_KEY"`，不发 Authorization / x-api-key。

> 本仓适配现状（**关键约束**）：
> - `src-tauri/src/gateway/proxy/headers.rs:99` 已声明 `api-key` 为敏感头（须 redact）
> - `src-tauri/src/gateway/proxy/headers.rs:205/234/265` 对 OpenAI 协议同时发送 `Authorization: Bearer` + `api-key`（双发）——MiMo 端接受额外未知头，OK
> - 但 **Anthropic 协议路径目前发的是 `x-api-key`，而非 `api-key`**（headers.rs:200-203）——这意味着按现状路由 Anthropic 端点可能被 MiMo 拒（除非 MiMo 兼容 x-api-key）。`需要: 验证 MiMo Anthropic 端点是否同时接受 x-api-key / api-key；若仅接受 api-key，新协议路由层需补一条 anthropic→api-key 的特例。`
>
> Token Plan 路由命中后建议补发 `api-key` 头（与 OpenAI 协议同模式），相关落点：`gateway/proxy/headers.rs::apply_default_headers` / `apply_claude_code_family_headers` / `apply_codex_family_headers` 三处 match arm。

### 与普通 xiaomi_mimo 区别

| 维度 | 普通版 `xiaomi_mimo` | Token Plan `xiaomi_mimo_coding` |
|---|---|---|
| host | `api.xiaomimimo.com` | `token-plan-{cn,sgp,ams}.xiaomimimo.com` |
| key 前缀 | `sk-` | `tp-` |
| 鉴权头 | OpenAI 标准 / MiMo api-key | **MiMo api-key**（两协议同头） |
| 计费 | token 单价 | Credits 配额（Lite 4.1B / Standard 11B / Pro 38B / Max 82B 月配额） |
| 模型集合 | 全量 | 同（无独占） |
| 有效期 | 充值永久 | 订阅期内 |
| 低峰折扣 | 无 | **0.8x**（北京时间 0:00-8:00 = UTC 16:00-24:00） |
| 使用范围限制 | 无 | **仅限 AI 编程工具**（OpenCode / Claude Code / Codex / Cline 等），禁用于自动化脚本 / 自定义后端；违规则封 key（[subscription.md](https://platform.xiaomimimo.com/static/docs/price/tokenplan/subscription.md) L233-235） |
| 区域集群 | 单一 | cn / sgp / ams 三集群（用户订阅时选定） |

> 低峰 0.8x 折扣已符合本仓 `peak_hours` 反向机制（multiplier < 1.0 表示折扣；现有 peak_hours 概念是高峰倍率，约定 multiplier > 1 加价）。但时间窗口是 UTC+8 北京时间 0-8 的**低峰**（非高峰），与现有 `peak_hours` 语义方向相反，**不适合直接用 peak_hours 表达**——降费率不是平台路由维度，建议留为文档说明，不入 preset。

## 结论（推荐字段值）

### `platform-presets.json` 新增 key `xiaomi_mimo_coding`

```json
"xiaomi_mimo_coding": {
  "client_type": "default",
  "endpoints": {
    "default": [
      {"protocol": "anthropic", "base_url": "https://token-plan-cn.xiaomimimo.com/anthropic", "client_type": "claude_code"},
      {"protocol": "openai", "base_url": "https://token-plan-cn.xiaomimimo.com/v1", "client_type": "codex_tui"}
    ]
  },
  "models": {"default": {"default": "mimo-v2.5-pro"}},
  "model_list": {"default": ["mimo-v2.5-pro", "mimo-v2.5"]},
  "name": {
    "en-US": "Xiaomi MiMo Token Plan",
    "zh-Hans": "小米 MiMo 套餐",
    ...
  },
  "source_urls": {
    "docs": "https://platform.xiaomimimo.com/static/docs/price/tokenplan/subscription.md",
    "pricing": "https://platform.xiaomimimo.com/#/token-plan"
  },
  "homepage": "https://mimo.mi.com",
  "logo_url": "xiaomi"
}
```

### 前端 PROTOCOLS（`src/domains/platforms/constants.ts`）

```ts
{ value: "xiaomi_mimo_coding", label: "小米 MiMo 套餐",
  keywords: ["xiaomi coding", "小米编程", "mimo token plan", "token plan", "tp-"],
  codingKeyPrefixes: ["tp-"] }
```

> 原 `xiaomi_mimo` coding 变体（`constants.ts:32`，同 value 带 `codingPlan: true`）应删除（避免双显，与 `glm_coding` 同模式）。

### Rust Protocol 枚举

`src-tauri/src/gateway/models.rs` 新增 `GlmCoding` 同模式：
```rust
#[serde(rename = "xiaomi_mimo_coding")]
XiaomiMimoCoding,
```

### 鉴权特例（**待验证**）

`gateway/proxy/headers.rs` 新协议命中时，**Anthropic 协议也需补发 `api-key` 头**（不仅是 OpenAI）。三处 `apply_*_family_headers` 的 `Protocol::Anthropic` arm 需特判 `xiaomi_mimo_coding`：
```rust
Protocol::Anthropic if is_mimo_token_plan => {
    rb = rb.header("api-key", api_key);  // MiMo 自定义，非 x-api-key
}
```

## Caveats / Not Found

- **MiMo Anthropic 端点是否兼容 `x-api-key`**：官方文档仅示范 `api-key` 头，未提 `x-api-key`。`需要: 真实 tp- key 跑一次 `https://token-plan-cn.xiaomimimo.com/anthropic/v1/messages` 对照两种头发回包，确认是否需补特例。`
- **`peak_hours` 字段不建议入 preset**：0.8x 是低峰折扣（反方向），与现有 peak_hours 语义冲突；留给 est_cost 估算阶段单独处理（或在 `subscription.md` 文档化为注意事项）。
- **区域集群选择**：preset 默认填 cn 集群。sgp / ams 集群作为可选 endpoint 由用户在 PlatformCard「Code」手动加。本仓 `injectProtocolHosts` 单一真值源机制下，preset 只列 cn，其他集群靠用户手工添加。
- **tp- key 使用范围限制（仅编程工具）**：若 aidog 路由被判定为「自动化脚本 / 自定义后端」，理论上 MiMo 有权封 key。此为平台政策风险，不在代码层处理。

## Files Found (internal)

| File Path | Description |
|---|---|
| `src-tauri/defaults/platform-presets.json:348-365` | 现有 `xiaomi_mimo` 普通 preset（参考结构） |
| `src/domains/platforms/constants.ts:31-32` | 现 PROTOCOLS 双显 `xiaomi_mimo` 普通 + coding 变体（codingKeyPrefixes: ["tp-"]），需拆为独立 value |
| `src/utils/platformPaste.test.ts:39,90,238-278` | 已有 `token-plan-cn.xiaomimimo.com` host + `tp-` 前缀 key 的测试 fixture（社区分享帖形态） |
| `src/utils/platformPaste.ts:32-34,327,546` | codingKeyPrefixes 数据驱动升级机制文档 |
| `src/domains/platforms/defaults.ts:222` | host 区分注释（token-plan-cn vs api） |
| `src-tauri/src/gateway/proxy/headers.rs:99,205,234,265` | `api-key` 头脱敏 + OpenAI 协议双发 Authorization/api-key 实现 |
| `src-tauri/src/gateway/proxy/passthrough.rs:613` | passthrough 路径同上 |

## 来源（URL）

- 官方 llms.txt: https://platform.xiaomimimo.com/llms.txt
- Token Plan 订阅说明: https://platform.xiaomimimo.com/static/docs/price/tokenplan/subscription.md
- Token Plan 快速接入: https://platform.xiaomimimo.com/static/docs/price/tokenplan/quick-access.md
- Claude Code 配置（含 1m 上下文）: https://mimo.mi.com/static/docs/tokenplan/integration/claudecode.md
- Codex 配置（Responses API）: https://mimo.mi.com/static/docs/tokenplan/integration/codex-configuration.md
- Token Plan FAQ: https://mimo.mi.com/static/docs/quick-start/faq/token-plan.md
