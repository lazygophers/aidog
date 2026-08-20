---
title: host-check-before-path
name: host-check-before-path
description: should_fallback_passthrough 的 host 判定必须前置于 path/is_api_endpoint 判定，否则 MITM 解密灌入流量被误判为代理自身 API 请求而全 404
layer: core
keywords: [should_fallback_passthrough, is_api_endpoint, MITM, 直通, host, 判定顺序]
created: 1785516000
inclusion: auto
---

## host 判定必须前置于 path 判定

### 铁律

- **`should_fallback_passthrough`（`gateway/proxy/endpoint.rs:262`）内，host 判定 MUST 排在
  `is_api_endpoint(path)` 之前。**
- host 非 self（不是 loopback 名，也不是 listen ip）→ 直接 `true` 直通原始 host，**不看 path**。
  这是 MITM 解密灌入与 forward proxy absolute-form 的流量。
- host 是 self + `is_api_endpoint(path)` → `false`，走 `resolve_group`，无 token 落 404
  （客户端直连代理自身 API 的语义要保留）。
- **禁把 `is_api_endpoint(path)` 的 early return 提到 host 判定之前** —— 这是历史顺序 bug
  （2026-07-06，task `07-06-mitm-decrypt-fallback-404`），代码里 `endpoint.rs:254-255` 的注释
  已把这个修法背书为「Bug B 修法」，改动前先读那段注释。

### 为什么只有 host 能区分

MITM 解密灌入与 forward proxy 的 path **与代理自身 API path 同形** ——
都是 `/v1/messages`、`/api/anthropic/v1/messages` 这类上游真实 API path。
单看 path 无法分辨「客户端在调 AiDog 的 API」和「AiDog 正在替客户端转发上游」，
只有 host 段能分（self vs 非 self）。path 早返会把所有 `/api/...` 的上游真实 path 一律拦死。

### 反例表

| 禁 | 改为 |
|---|---|
| `if is_api_endpoint(path) { return false }` 置顶 | host 非 self 先返 `true`，再判 path |
| 只按 path 前缀判「是不是自己的 API」 | host 判定为准，path 仅在 host=self 时参与 |

### 验收基准（复用既有断言）

- MITM 灌入：host=`open.bigmodel.cn` + path=`/api/anthropic/v1/messages` + listen_addr=Some
  → `should_fallback_passthrough=true` → 透明转发 200
  （测试 `mitm_decrypted_api_path_falls_through_to_orig_host`）
- 自身直连：host=`127.0.0.1:port` + path=`/api/...` + 无效 token + listen_addr=Some(port)
  → `false` → 404（测试 `api_path_wrong_token_still_404_no_bypass`）

## 关联

[[connect-tunnel-contract]] · [[http-client-no-env-proxy]]
