# forward proxy 扩展支持 absolute-form HTTP 转发任意 host

## Goal

aidog proxy 当前是 reverse proxy / LLM 网关，但用户合理期望 `-x` forward 模式工作。curl `-x http://127.0.0.1:9892/proxy http://www.baidu.com` 返回健康端点 JSON 而非 baidu 原始内容。根因：axum `.route("/", get(handle_root))` 按 `uri.path()` 匹配，absolute-form `GET http://www.baidu.com/` 的 path=`/` 被健康端点劫持，未进 fallback handle_proxy。

扩展支持 absolute-form HTTP forward 到任意 host（与现有 MITM CONNECT fallback 同语义：非 AI 流量直通原 host，落虚拟桶「未匹配」不计费）。

## What I already know

- 路由定义 `mod.rs:240-241`：`.route("/", get(handle_root)).route("/proxy", get(handle_root)).fallback(handle_proxy)`
- axum 按 `Request::uri().path()` 匹配路由；absolute-form URI 的 path 是目标 URL 的 path（baidu 的 `/`），不含 authority → 命中 `.route("/")`
- `handle_proxy_core` (handler.rs:102) 已有 fallback 路径：`should_fallback_passthrough(host, path, listen_addr)` (endpoint.rs:256) 查 Host header，非代理自身 host 且非 API path → `forward_passthrough_to_orig_host` (passthrough.rs:333)
- `forward_passthrough_to_orig_host` 当前硬编码 `https://{host}{pq}` (passthrough.rs:368) — MITM 解密灌入场景，明文 HTTP absolute-form 需 scheme 自适应
- 现有 MITM fallback 落 proxy_log 虚拟桶 `UNMATCHED_GROUP_KEY="未匹配"` + `cost=0` 不计费 (passthrough.rs:345-347)
- CONNECT 走独立 `upsert_connect_log` 路径，不落 proxy_log stats_agg；absolute-form HTTP 应**不**走 CONNECT 路径，复用 proxy_log 虚拟桶与 MITM fallback 对齐

## Requirements

- absolute-form HTTP 请求 (`uri.scheme_str().is_some() && uri.host().is_some()`) 绕过 `.route("/")` / `.route("/proxy")` 健康端点，进 handle_proxy 走 group 路由 + fallback
- `forward_passthrough_to_orig_host` URL 构造读 `orig_uri.scheme_str()` (http/https) 自适应，不再硬编码 https
- 落 proxy_log 虚拟桶「未匹配」+ cost=0（与 MITM fallback 对齐，不计费）
- 透传 method/headers/body（含 GET/POST/PUT/DELETE 等所有非 CONNECT 方法）
- 剥 proxy-only headers（Proxy-Authorization / Proxy-Connection 已有，passthrough.rs:381）

## Acceptance Criteria

- [ ] `curl -v -x http://127.0.0.1:9892/proxy http://www.baidu.com` 返回 baidu 原始 HTML（非 `{"ok":true,"service":"aidog"}`）
- [ ] `curl -v -x http://127.0.0.1:9892/proxy https://www.baidu.com` (HTTPS absolute-form 非 CONNECT) 同样可转发性
- [ ] 响应头含 `x-aidog-trace`（debug 模式）
- [ ] proxy_log 落虚拟桶「未匹配」+ cost=0，status/method/url 记录正确
- [ ] reverse proxy AI 协议路径不回归（`POST /v1/messages` 仍走 group 路由，返 404 健康端点探测仍返 JSON）
- [ ] `cargo clippy` 0 warning，`cargo test` 全绿

## Definition of Done

- 路由 middleware 改动 + forward_passthrough URL scheme 自适应改动
- 新增 absolute-form forward 集成测试（HTTP + HTTPS）
- 现有 reverse proxy 测试全绿不回归
- spec 沉淀（forward proxy absolute-form 契约，对偶 proxy-connect-relay.md）

## Technical Approach

### 路由层 (mod.rs)

axum middleware 在 Router::new 后、fallback 前拦截：检测 `req.uri().scheme_str().is_some() && req.uri().host().is_some()` → 直接转 `handle_proxy`，不进 `.route("/")` 健康匹配。

候选方案 A (推荐)：用 `axum::middleware::from_fn` 包 Router，middleware 内若识别 absolute-form 则 `Ok(handle_proxy(state, req).await.into_response())` 提前返回，否则 `Ok(next.run(req).await)`。

候选方案 B：把 `.route("/")` 改成自定义 matcher 检测非 absolute-form。axum 不直接支持，需 fallback + 健康端点内自检。复杂，弃。

### forward_passthrough URL (passthrough.rs:368)

```rust
let scheme = orig_uri.scheme_str().unwrap_or("https");  // MITM 解密灌入无 scheme 默认 https
let url = format!("{scheme}://{host_header}{pq}");
```

### handle_proxy_core 已就绪

CONNECT 早期分流在 handler.rs:83。absolute-form 进 handle_proxy_core 后：
- `path = req.uri().path()` = 目标 path（如 `/`）
- `should_fallback_passthrough(host_header, path, listen_addr)`：host=www.baidu.com 非代理自身 → true
- 进 `forward_passthrough_to_orig_host` 直通

## Decision (ADR-lite)

**Context**: aidog 是 LLM 网关 reverse proxy，但 `-x` forward 模式被健康端点劫持。
**Decision**: 扩展 forward proxy 支持，复用现有 MITM fallback 路径（虚拟桶 + forward_passthrough_to_orig_host），不新增独立 forward 模块。路由 middleware 识别 absolute-form。
**Consequences**: 用户可用 `-x` 转发任意 HTTP 站点；proxy_log 虚拟桶「未匹配」会包含 forward 流量（与 MITM fallback 同语义，可接受）。

## Out of Scope

- SOCKS proxy 支持
- forward proxy 鉴权（沿用现有 reverse proxy Authorization，无 token 则虚拟桶）
- forward 流量单独统计页（复用「未匹配」虚拟桶）

## Technical Notes

- 路由层：mod.rs:234-243
- forward URL：passthrough.rs:368
- fallback 判定：endpoint.rs:256
- MITM fallback 现有测试：test_integration.rs:827 `should_fallback_passthrough_decision_matrix`
