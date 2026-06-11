---
updated: 2026-06-11
rewrite-version: 1
authored-by: trellis-implement
mode: optimize
---

# Claude Code Passthrough Platform Type

何时被读: 改 Claude Code 订阅透传逻辑（proxy.rs handle_passthrough / 原始请求捕获点 / models.rs Protocol::ClaudeCode）/ 新增纯透传平台类型 / 调透传 header 剔除或 log 记录语义时
谁读: trellis-implement sub-agent / main
不遵守的代价: 透传链路误转换 body/header → 订阅认证失效 / 双重路径前缀 404 / 剔错 hop-by-hop 头 / proxy_log 漏记

---

## What & When (MUST)

- `Protocol::ClaudeCode`（`models.rs`，serde rename `"claude_code"`）是**平台主类型**，不是 endpoint 协议；禁加入 `ENDPOINT_PROTOCOLS`
- 语义 = **纯透传 relay**：路由到 `platform_type == ClaudeCode` 的平台时，**bypass 所有转换**，把客户端原始请求 1:1 转发到 `platform.base_url`，原样返回响应，记 `proxy_log`
- 适用场景：用户用已登录 claude.ai 订阅的 Claude Code CLI 把 CLI 指向 aidog 代理 —— 订阅 OAuth 凭证由**客户端自带**在请求 header 里，aidog 不管理 token

## Original Request Capture (MUST)

- `proxy.rs` handle_proxy 在 `req.into_parts()` **之前**捕获原始量（对所有请求 clone，开销小）：
  - `orig_method = req.method().clone()`
  - `orig_uri = req.uri().clone()`（含 path + query）
  - `orig_headers = req.headers().clone()`（含**真实未 redact 的 Authorization**）
- 禁用 `log.request_headers`：其 Authorization 已被 REDACT，不可用于透传
- body bytes 复用现有已读 `bytes`（与 model 提取 / log 共用同一份）

## Intercept Point (MUST)

- 拦截点：`select_platform` 之后、`convert_request` 之前（与 mock 拦截点同区），判 `matches!(route.platform.platform_type, Protocol::ClaudeCode)` → 走 `handle_passthrough` 后 return
- router.rs 无需改（选平台不校验 api_key）；db.rs 无需改（enum 自动序列化）

## handle_passthrough Semantics (MUST)

1. **目标 URL** = `base_url` + 客户端原始 path（+ query）。**约定 CC 平台 base_url 填 host 根**（如 `https://api.anthropic.com`，无版本前缀），URL = base_url + orig_path；base_url 带前缀会导致双重路径，前端 UI 必须提示填 host 根
2. reqwest 用 `orig_method` + 目标 URL + body = 原始 `bytes`
3. **header 原样转发** `orig_headers`，仅剔除 hop-by-hop：
   - `Host`（reqwest 按目标 URL 自动设）
   - `Content-Length`（reqwest 按 body 自动设）
   - 其余**全部原样**，**含客户端自带 Authorization OAuth**（订阅凭证）
4. 超时复用现有 proxy timeout 设置
5. **响应原样 relay**（不回转协议）：
   - 非流式：`resp.bytes()` 原样回客户端 + 原 status + 原 content-type
   - 流式（content-type `text/event-stream` 或 chunked）：`resp.bytes_stream()` 原样透传，不解析不转换，响应头照搬上游

## No Transform / No Inject (MUST)

- 禁 `convert_request` / 禁 `build_upstream_headers` / 禁 `apply_client_headers` / 禁 ClientType 模拟
- 禁注入 `api_key` / token（客户端自带订阅 OAuth header）
- 禁 OAuth 登录或 refresh（aidog 不管理 token）

## proxy_log (MUST)

- 透传分支**正常记** `proxy_log`：
  - `source_protocol` = `target_protocol` = `"claude_code"`（透传不转换，标同值）
  - `upstream_request_url` / `upstream_status_code` / `response_body`（非流式）/ `upstream_response_headers`
  - token：尽力解析 —— 非流式复用 `extract_usage`（anthropic usage），流式从 SSE usage 累计 → 填 `input_tokens` / `output_tokens` / `cache_tokens`
  - `actual_model` = `log.model`（客户端请求 model，未改）
  - `platform_id` = `route.platform.id`
- upsert_log 后 return

## Frontend (MUST)

- `api.ts` Protocol union 含 `| "claude_code"`
- `Platforms.tsx`：`PROTOCOLS` 含 `claude_code`（label「Claude Code 订阅（透传）」）；`ENDPOINT_PROTOCOLS` **不含**
- `getDefaultEndpoints` claude_code 预填 base_url 默认 `https://api.anthropic.com`
- `platform_type === "claude_code"`（`isPassthrough`）时：api_key 可空（去必填校验）、隐藏 endpoints/models 编辑、base_url 字段提示「填 host 根，透传拼客户端原始 path/query，勿带版本前缀」
- 参照现有 mock（`isMock`）条件渲染模式实现 `isPassthrough` 分支

## Verification

```bash
cd src-tauri && cargo test passthrough   # URL 拼接 / header 剔除 Host+Content-Length 保留 Authorization / log 字段填充 / 不调 convert_request

# claude_code 不入 endpoint 协议
grep -nE "claude_code" src/pages/Platforms.tsx   # PROTOCOLS 有，ENDPOINT_PROTOCOLS 无

yarn tsc --noEmit   # 退出码 0
```

- 手测：CC CLI 指向 aidog（配 claude_code 平台）→ 请求原样到 anthropic + 响应正常（流式 + 非流式）+ proxy_log 记录
