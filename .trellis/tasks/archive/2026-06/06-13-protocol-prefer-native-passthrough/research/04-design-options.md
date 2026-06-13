# Research: 实现「优先原协议透传」的改造点 + 方案

- **Query**: 实现「优先原协议、不支持才转换」改哪些函数？方案与风险？
- **Scope**: internal (设计调研，不含实现建议的取舍裁决，仅罗列事实与改造面)
- **Date**: 2026-06-13

## 现状基线（综合 01-03）

1. 入站协议 = path 推断 `proxy.rs:578`（5 种 wire：anthropic/openai/openai_responses/openai_completions/gemini）。
2. 出站协议 = **已有**端点匹配 `proxy.rs:629-641`：`platform.endpoints` 中找 `protocol == source_protocol` 的端点；无匹配回退 `platform_type`。
3. **缺口**：端点匹配命中后仍**无条件** `convert_request` 重序列化（有损往返），无「同协议字节透传」短路。
4. 已有字节级透传基础设施：`handle_passthrough` + `orig_method/uri/headers/bytes` 原始量捕获 `proxy.rs:522,1185`，但仅 `ClaudeCode` 平台触发。

## 改造点（函数级）

### 方案 A：端点命中即字节透传（最贴用户需求）

判定位置：`proxy.rs` 在 `matched_ep` 求出后（`proxy.rs:638` 之后）插入：
- **若 `matched_ep.is_some()`（平台显式声明了入站协议端点）→ 走透传分支**：把请求 body + 必要 header 1:1 转发到 `ep.base_url + 客户端原始 path`，响应原样 relay，**不调 `convert_request` / `parse_sse`**。
- **else（无匹配端点）→ 维持现状**：`convert_request` 转成平台主协议。

需改/新增函数：
- `proxy.rs` 主转发函数：在 `proxy.rs:639` 附近加分支判定。
- 复用 `handle_passthrough` 逻辑，但需**泛化**：当前它写死 `source/target = "claude_code"` `proxy.rs:1198-1199` 且 URL 用 host 根 + 原始 path。新透传要：
  - URL：`ep.base_url`(含版本前缀) + 客户端原始 path 的协议后缀。**注意 base_url 已含 `/v1` 等前缀，原始 path 也含 `/v1/...`，直接拼接会重复**（如 `base/v1` + `/v1/messages`）。需要剥离/对齐——这是与 ClaudeCode 透传(用 host 根 base_url)的关键差异。
  - header：不能照搬客户端 OAuth（普通平台用平台自己的 `api_key`）。需注入 `apply_client_headers`(`proxy.rs:1552`) / `build_upstream_headers`(`proxy.rs:766`) 那套平台鉴权，而非 `passthrough_headers` 的原样保留。
  - log：source/target 标真实协议，非 "claude_code"。

### 方案 B：保留 convert_request 但加「同协议跳过转换」短路

判定位置：`proxy.rs:740` 前：
- `if source_protocol == target_protocol`（且无 model remap / coding_plan 特殊注入）→ 直接用客户端原始 body bytes 出站，跳过 `to_*` 重序列化；响应侧若同协议也跳过 `parse_sse→to_client_sse`，原样 relay SSE。
- else → 现状转换。

需改：
- `proxy.rs:740-742` 出站 body 构造分支。
- `proxy.rs:882-1010` 响应流式分支（加「同协议原样 relay」short-circuit）。
- 复杂度高于 A：需在已解析 `chat_req`/已 model-remap 的中途插「回退用原始 bytes」，与 `needs_model_remap`(`proxy.rs:644`) / `coding_plan`(`proxy.rs:745`) 注入冲突——model 改名时无法用原始 body。

### 方案 C：补全 platform_type 的隐式协议能力（判定增强，非透传）

- 现状回退分支 `proxy.rs:641` 无脑用 `platform_type` 出站，没判断 `platform_type` 的 wire 归类是否 == 入站协议。
- 可新增 `Protocol::wire_kind() -> Protocol`（把 48 平台类型归到 5 wire 之一，逻辑现散在 `convert_request` match 默认分支）。
- 用途：判「平台是否原生支持入站协议」时，除 endpoints 外也认 `platform_type.wire_kind() == source_protocol`。这是**精确判定的前置依赖**（见 02 caveat）。

## 风险清单

1. **URL 构造（CLAUDE.md 硬约束）**：`base_url` 含版本前缀 + 客户端原始 path 也含 `/v1` → 透传拼接易重复前缀。ClaudeCode 透传规避方式是 base_url 填 host 根；普通平台 base_url 含 `/v1`，不能照搬 `build_passthrough_url`(`proxy.rs:1367` 直接 base+path)。需新 URL 构造逻辑。
2. **不同协议端点的鉴权差异**：透传 ClaudeCode 用客户端 OAuth；普通平台端点要用平台 `api_key` + `client_type` 模拟 header（`apply_client_headers` `proxy.rs:1552-1576`，按协议分支注入 x-api-key / Authorization / Gemini key）。泛化透传必须走平台鉴权而非原样 header。
3. **入站协议↔端点协议匹配粒度**：当前精确字符串相等 `proxy.rs:631`，仅 `openai_responses→openai` 一条跨协议回退 `proxy.rs:633-637`。「Anthropic 入站平台只有 openai 端点」之类不会命中透传，会落回退转换——符合用户需求（不支持才转），但要确认 `openai_responses→openai` 这条回退在透传语义下是否仍算「支持」（它本身要转换，不能字节透传）。
4. **model remap 冲突**：透传用原始 body，但路由可能改了模型名（`actual_model != requested_model`, `proxy.rs:644`）。同协议透传时若需改模型名，无法纯字节透传，须改写 body 内 model 字段或放弃透传。
5. **coding_plan 注入**：`proxy.rs:745-748` 对 coding plan 端点注入特有字段 + 改 path。透传 bypass 这些注入，coding plan 端点不能简单透传。
6. **响应 usage/计费**：透传只能「尽力」从 SSE 累计 token（`proxy.rs:1334` 现状），est_cost 计费链路（pricing resolve）依赖结构化 usage，透传下精度可能下降。

## 涉及文件汇总

| 文件 | 角色 |
|---|---|
| `src-tauri/src/gateway/proxy.rs` | 主改造面：端点匹配 `:629`、转换调用 `:742`、透传 handler `:1185`、URL 构造 `:1367`、header `:1552` |
| `src-tauri/src/gateway/adapter/converter.rs` | wire 分支 `:10`，若加 `wire_kind` 归类可放此或 models |
| `src-tauri/src/gateway/models.rs` | Protocol 枚举 `:4`、PlatformEndpoint `:247`、（可选新增 `wire_kind`） |
| `src-tauri/src/gateway/router.rs` | 当前不涉协议，若把协议决策上移到路由层需改 `select_platform` |

## Caveats / Not Found

- 未做取舍裁决（A/B/C 哪个更优属 main agent/设计决策，非 research 职责）。
- `需要`确认：用户语义「平台本身支持该协议」是指「显式声明 endpoint」还是「platform_type 归类即可」？两者判定范围不同（见 02），直接影响是否需要方案 C 的 `wire_kind`。
