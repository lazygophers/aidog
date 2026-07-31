---
title: connect-tunnel-contract
name: connect-tunnel-contract
description: CONNECT authority-form 与 absolute-form 这两种非标准 URI 形态禁用 axum .route() 注册，必须在 handler 头部或 Router 外层 middleware 早期分流
layer: core
keywords: [CONNECT, absolute-form, authority-form, axum, path-matcher, 隧道]
created: 1785516000
inclusion: auto
---

## 非标准 URI 形态禁走 axum path matcher

axum 的路由匹配**只看 `req.uri().path()`**。HTTP 的两种非 origin-form 请求经过它都会被错误分派，
必须在进入路由表之前分流。

### 铁律

- **CONNECT（authority-form，`CONNECT host:port HTTP/1.1`）禁用 `.route()` 注册** ——
  authority-form 的 path 段为空，`uri().path()` 返 `""` 而非 `host:port`，path matcher 不可靠。
  正解：`handle_proxy_inner` 头部按 method 早期分流（`proxy/handler.rs:83`
  `if req.method() == axum::http::Method::CONNECT`）转 `proxy/connect.rs:50 handle_connect`。
- **absolute-form（`GET http://host/path`）禁用 `.route()` 注册** ——
  它的 path 段是目标 path，`GET http://www.baidu.com/` 的 path=`/` 会命中健康端点 `.route("/")`，
  返 `{"service":"aidog"}`，forward 模式静默失效。
  正解：Router 顶层 middleware 识别后直转 `handle_proxy`（`proxy/mod.rs:311 build_router` →
  `:328` 挂 `absolute_form_forward_mw` → `:340` 函数体，`:346` 判据
  `uri.scheme_str().is_some() && uri.host().is_some()`）。
- **两种分流都必须是 early return / `next.run` 透传**，不得影响 origin-form 路由 ——
  `GET /`、`GET /proxy`、`GET /models`、`POST /api/*`、AI path 全部照走原 `.route()` 或 fallback。
- **CONNECT target 禁单源取 `uri().path()`** —— 三源兜底
  `path → uri().authority() → Host header`，三源皆空返 400，禁走 `TcpStream::connect("")`
  （必败 502 且误导诊断）。
- **forward URL 的 scheme 禁硬编码 `https`** —— 取 `orig_uri.scheme_str().unwrap_or("https")`
  （`proxy/passthrough.rs:441`）。明文 HTTP 上游被 https 打中会 TLS handshake 失败。

### 反例表

| 禁 | 改为 |
|---|---|
| `.route("/*path", connect_handler)` | `handler.rs` 头部 `method() == CONNECT` 早期分流 |
| `.route("/", health)` 拦下 absolute-form | Router 外层 `absolute_form_forward_mw` 先识别 |
| `req.uri().path()` 单源取 CONNECT target | path → authority → Host header 三源兜底 |
| `format!("https://{host}{pq}")` | `scheme_str().unwrap_or("https")` 拼 scheme |

### 落库语义（两条路径不同，禁混用）

- CONNECT 走 `upsert_connect_log`（直接 `insert_proxy_log_columns`），**禁调 `upsert_log`** ——
  隧道流量非 AI 请求，进 `stats_agg` 会让统计页虚高。
- absolute-form forward 走 `upsert_log` + 虚拟桶 `UNMATCHED_GROUP_KEY`，与 MITM 解密 fallback 同语义。

## 关联

[[http-client-no-env-proxy]] · [[host-check-before-path]]
