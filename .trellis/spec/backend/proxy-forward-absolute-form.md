---
updated: 2026-07-06
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# Forward Proxy Absolute-Form HTTP 转发

何时被读: 改 `src-tauri/src/gateway/proxy/mod.rs` (Router 构建 / `build_router` / `absolute_form_forward_mw` middleware) / `proxy/passthrough.rs::forward_passthrough_to_orig_host` (URL scheme 自适应) 时
谁读: main / sub-agent
不遵守的代价: forward 客户端 (`curl -x`) 的 absolute-form 请求被健康端点劫持 / scheme 错写 https 致明文 HTTP host 转发失败。`07-06-forward-http-absolute-form` 实证。

---

## absolute-form URI 路由契约 (MUST)

> 违反代价: axum 按 `Request::uri().path()` 匹配路由，absolute-form `GET http://www.baidu.com/` 的 path=`/` 命中 `.route("/")` 健康端点 → 返 `{"service":"aidog"}`，forward 模式不工作。

- **absolute-form 识别 = `uri().scheme_str().is_some() && uri().host().is_some()`** — RFC 7230 §5.3.2 absolute-form 含 scheme + authority；reverse proxy path-only URI 两者皆 None；CONNECT authority-form 无 scheme。
- **禁用 axum `.route()` 注册 absolute-form** — axum 0.8 path matcher 仅按 `uri().path()` 匹配，不识别 scheme/host；`.route("/")` 会劫持所有 absolute-form 的目标 path=`/` 请求。
- **改在 Router 顶层 middleware 识别 absolute-form → 直接转 handle_proxy** — `axum::middleware::from_fn_with_state(state, absolute_form_forward_mw)` 包 Router 外层：识别 → `handle_proxy(state, req).await`；否则 `next.run(req).await` 进正常路由。
- **early return 不破现有路由** — path-only URI (`GET /` / `GET /proxy` / `POST /v1/messages` / `POST /api/group-info` 等) 全部 `next.run` 走原显式 `.route()` 或 fallback，不受 middleware 影响。
- **复用 handle_proxy 完整请求生命周期** — middleware 内调 `handle_proxy` 而非新写 forward handler，继承 req span / RequestLogGuard / fallback 路径，与 MITM fallback 同语义。

## forward URL scheme 自适应 (MUST)

> 违反代价: `forward_passthrough_to_orig_host` 硬编码 `https://{host}{pq}` → 明文 HTTP absolute-form (`GET http://host/`) 的 stub HTTP 上游被 https:// 请求打中致 TLS handshake 失败。

- **scheme 取 `orig_uri.scheme_str().unwrap_or("https")`** — absolute-form URI 含原 scheme (http/https) 直接透传；MITM 解密灌入无 scheme (明文 Request URI 仅 path 段) 默认 https 保持原行为。
- **URL = `{scheme}://{Host header}{path}?{query}`** — Host header 含端口 (www.baidu.com:443)，缺省 path 用 `/`；scheme 与 host 分离，禁拼死 https。
- **hop-by-hop + proxy-only headers 必剥** — `passthrough_headers` 已剔 host/content-length；补 `Proxy-Authorization` / `Proxy-Connection` / `Proxy-Authenticate`，避免代理协商头被转发到上游。

## proxy_log 落虚拟桶 (MUST — 与 MITM fallback 同语义)

> 违反代价: forward 流量走独立 upsert 路径 / 单独统计 → 与 MITM 解密 fallback 语义漂移。

- **复用 `upsert_log` + 虚拟桶「未匹配」** — `forward_passthrough_to_orig_host` 内 `log.group_key = UNMATCHED_GROUP_KEY` + `log.source_protocol = "passthrough_unmatched"` + `cost=0`（`est_cost` 默认 0.0 不计费）。
- **禁走 CONNECT 的 `upsert_connect_log`** — CONNECT 是独立 TCP 隧道路径，不落 proxy_log stats_agg；absolute-form HTTP 是 HTTP 协议层 forward，落 proxy_log 与 AI 请求统计同表（虚拟桶隔离不污染）。
- **request_url / upstream_request_url 字段区分** — `request_url` = absolute-form URI（含 scheme + authority）；`upstream_request_url` = 构造的目标 URL (`{scheme}://{host}{pq}`)。

## 路由层契约 (MUST)

- **`build_router(state: Arc<ProxyState>) -> Router`** — Router 构建抽函数，供 `start_proxy` 与集成测试复用；middleware 通过 `.layer(from_fn_with_state(state.clone(), absolute_form_forward_mw))` 包外层。
- **CONNECT 不触发此 middleware 路径问题** — CONNECT 在 `handle_proxy_inner` 早期分流 (handler.rs:83)，走 `connect::handle_connect` 独立路径；middleware 仅识别 absolute-form URI 含 scheme+host (CONNECT authority-form URI 无 scheme)，不会误转。

## 验证

```bash
# absolute-form middleware 存在 + 路由顶层包装
grep -n "absolute_form_forward_mw\|from_fn_with_state" src-tauri/src/gateway/proxy/mod.rs

# scheme 自适应 (禁硬编码 https)
grep -n "scheme_str().*unwrap_or" src-tauri/src/gateway/proxy/passthrough.rs  # forward_passthrough_to_orig_host URL 构造

# 集成测试: absolute-form HTTP forward + 健康端点不回归 + HTTPS scheme 自适应
cd src-tauri && cargo test absolute_form_http_forward_returns_orig_body_not_health_endpoint -- --nocapture
cd src-tauri && cargo test path_only_uri_still_hits_health_endpoint_no_regression -- --nocapture
cd src-tauri && cargo test absolute_form_https_uri_scheme_adaptive -- --nocapture
```

## 跨层 / 关联 spec

- [Proxy CONNECT Relay](./proxy-connect-relay.md) — CONNECT 隧道独立路径对偶 (本 spec 是 HTTP absolute-form forward 契约)；CONNECT 走 authority-form URI 无 scheme/host，absolute-form 走 scheme+host 同存识别。
- [HTTP Client Forward](./http-client-forward.md) — 上游 reqwest client 构造契约；forward 客户端 (curl `-x`) 与 AirDog 自身转发上游 (build_http_client) 区分。
