# Design: Claude Code 订阅平台（纯透传）

## 平台类型
- models.rs Protocol enum 加 `#[serde(rename="claude_code")] ClaudeCode,`（平台类型区）
- 前端 api.ts union 加 `| "claude_code"`；Platforms.tsx PROTOCOLS 加 `{value:"claude_code", label:"Claude Code 订阅（透传）", keywords:["claude code","订阅","透传","subscription"]}`；ENDPOINT_PROTOCOLS 不加

## 纯透传语义
路由到 `platform_type == ClaudeCode` 的平台时，**bypass 所有转换**，把客户端原始请求 1:1 relay 到 platform.base_url，原样返回响应，记 proxy_log。

## 原始请求捕获（核心改动）
proxy.rs handle_proxy 在 `req.into_parts()`（现 :201）**之前**捕获原始量（对所有请求 clone，开销小）：
```
let orig_method = req.method().clone();
let orig_uri = req.uri().clone();            // path + query
let orig_headers = req.headers().clone();    // 含真实 Authorization（未 redact）
```
（现有 `log.request_headers` 的 Authorization 被 REDACT，不可用于透传；`auth_header`/`path` 是局部提取，不够）
body bytes 复用现有 `bytes`（:202）。

## 拦截点
`select_platform`(:305) 之后、`convert_request`(:359) 之前，加：
```
if matches!(route.platform.platform_type, Protocol::ClaudeCode) {
    return handle_passthrough(&state, &mut log, &log_settings,
        orig_method, orig_uri, orig_headers, bytes,
        &route.platform.base_url, start).await;
}
```
（位置同 mock :341 区，复用已解析的 route / bytes / log）

## handle_passthrough（新函数）
1. 构造目标 URL = `base_url` + 原始 path（+ query）。注意 base_url 可能含版本前缀（db-conventions：base_url 含 /v1 等）；透传应拼客户端原始 path —— **design 决策**：CC 订阅客户端打 `/v1/messages` 等完整 anthropic path，base_url 设为 `https://api.anthropic.com`（无前缀），URL = base_url + orig_path。若 base_url 带前缀会双重，**约定 CC 平台 base_url 填 host 根**（UI 提示）
2. reqwest 用 `orig_method` + 目标 URL + body=`bytes`
3. header 原样转发 `orig_headers`，**剔除 hop-by-hop**：`Host`（reqwest 按目标 URL 设）、`Content-Length`（reqwest 按 body 设）；其余全部原样（**含客户端自带 Authorization OAuth** —— 订阅凭证）
4. 发请求，超时复用现有 proxy timeout 设置
5. 响应原样 relay：
   - 非流式：`resp.bytes()` 原样回客户端 + 原 status + 原 content-type
   - 流式（resp content-type text/event-stream 或 transfer-encoding chunked）：`resp.bytes_stream()` 原样透传（不解析不转换），响应头照搬上游
6. proxy_log 记录（正常记）：
   - `source_protocol` = `target_protocol` = "claude_code"（或客户端协议；透传不转换，标同值）
   - `upstream_request_url` / `upstream_status_code` / `response_body`(非流式) / `upstream_response_headers`
   - token：尽力从响应解析（复用 `extract_usage` 对非流式 anthropic usage；流式从 SSE usage 累计）→ 填 input/output/cache_tokens
   - `actual_model` = `log.model`（客户端请求 model，未改）
   - `platform_id` = route.platform.id
   - upsert_log 后 return

## 不做
- 不 convert_request / 不 build_upstream_headers / 不 apply_client_headers / 不注入 api_key / 不 OAuth 登录或 refresh
- router.rs 无需改（选平台不校验 api_key）
- db.rs 无需改（enum 自动序列化）

## 前端
- PROTOCOLS 加 claude_code；`platform_type==="claude_code"` 时：api_key 可空（客户端自带认证）、endpoints 隐藏、base_url 提示填 host 根（如 https://api.anthropic.com）
- getDefaultEndpoints claude_code 返空；可预填 base_url 默认 `https://api.anthropic.com`

## 验证
- cargo build + tsc 0
- 单测：handle_passthrough URL 拼接 / header 剔除 Host+Content-Length 保留 Authorization / log 字段填充；透传分支不调 convert_request
- 手测：CC CLI 指向 aidog（配 claude_code 平台）→ 请求原样到 anthropic + 响应正常 + proxy_log 记录
