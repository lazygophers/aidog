---
updated: 2026-07-06
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# Proxy Fallback Host Routing

何时被读: 改 `src-tauri/src/gateway/proxy/endpoint.rs::should_fallback_passthrough` 或 handler fallback 路由判定时
谁读: main / sub-agent
不遵守代价: MITM 解密灌入流量 (host=上游域名, path=上游真实 API path) 被误判为代理自身 API 请求 → 不直通 → 404 → 代理破坏上游通信

## 核心契约 (MUST)

- **`should_fallback_passthrough` host 判定 MUST 前置于 path/is_api_endpoint 判定** —
  MITM 解密灌入与 forward proxy 的 path 含 `/v1/messages` 等上游真实 API path, 与代理自身 API path 同形。
  仅靠 host (self vs 非 self) 能区分:
  - host 非 self (loopback 名 / listen ip 之外) → 直接 `true` 直通原始 host, **不看 path** (MITM 灌入 / forward proxy absolute-form)
  - host self + `is_api_endpoint(path)` → `false` (客户端直连代理自身 API, 走 resolve_group → 无 token 404 语义保留)

- **禁恢复 `is_api_endpoint(path)` early return 前置于 host 判定** — 历史顺序 bug (2026-07-06, task `07-06-mitm-decrypt-fallback-404`): path 早返拦死所有 `/api/...` 上游真实 API path, MITM 解密链路全 404。

## host self 判定分支 (复用, 不变)

loopback 名 (`localhost`/`127.0.0.1`/`0.0.0.0`) + listen ip 字面量比对 + port 比对, 见 `endpoint.rs::should_fallback_passthrough` 既有 4 分支。

## 验收基准 (复用断言)

- MITM 灌入: host=`open.bigmodel.cn` + path=`/api/anthropic/v1/messages` + Authorization=上游真实 key + listen_addr=Some → `should_fallback_passthrough=true` → 透明转发 → 200 (测试: `mitm_decrypted_api_path_falls_through_to_orig_host`)
- 自身直连: host=`127.0.0.1:port` + path=`/api/...` + 无效 token + listen_addr=Some(port) → `should_fallback_passthrough=false` → 404 (测试: `api_path_wrong_token_still_404_no_bypass`)

## 关联

- CONNECT 隧道 / relay 层: [proxy-connect-relay.md](proxy-connect-relay.md)
- forward proxy absolute-form: [proxy-forward-absolute-form.md](proxy-forward-absolute-form.md)
- request_url 含 host 重构 (Bug A): `passthrough.rs::build_url_from_host` helper, origin-form 缺 scheme 默认 https, absolute-form 取 `uri.scheme_str()`
