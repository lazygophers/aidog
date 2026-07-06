# mitm 解密流量 fallback 404 + proxy_log url 不完整

## 背景

会话延续 (上一 task `07-06-mitm-chain-full-diagnosis` 已 archive, 修了 suffix 前导点双点永不命中)。用户重启 dev 验证, .cn 后缀匹配已生效 (MITM 解密链路打通), 但解密后请求返回 404, 且 proxy_log 的 url 字段只记 path 段。

证据 request_id: a33769831667441181b1996390ab0321
- host=open.bigmodel.cn, Authorization=智谱上游真实 key, path=/api/anthropic/v1/messages
- response: 404 "no matching group for token 'b6cf...'.OLsQE1zkWbqyamBK' or path '/api/anthropic/v1/messages'"
- url 字段: `/api/anthropic/v1/messages?beta=true` (仅 path, 缺 host)

用户原话: "如果不加代理, 这个请求是正常的, 加了代理又报错 404" + "url 应该是完整的而不是只有 path"

## 根因 (已诊断)

### Bug B (核心, 404)
`src-tauri/src/gateway/proxy/endpoint.rs:256-258` `should_fallback_passthrough` 第一行:
```rust
if is_api_endpoint(path) {
    return false;
}
```
此判定**前置**于 host 判定。MITM 解密灌入请求 path 是上游真实 API path (`/api/anthropic/v1/messages` 含 `/v1/messages` → is_api_endpoint=true) → 直接 return false → 不直通 → resolve_group 返 None → 404。

设计冲突:
- 客户端直连代理自身 (host=127.0.0.1:port) + /api/... → AirDog 应用 API → 无 group 404 ✓ (期望保留)
- MITM 解密灌入 (host=open.bigmodel.cn) + /api/... → 应透明转发原始 host (智谱 anthropic 兼容端点) → 200 ✓ (期望修复)

`forward_passthrough_to_orig_host` (`passthrough.rs:333+`) 已就绪: 取 Host header + path_and_query 重构 url 转发上游。仅 should_fallback_passthrough 入口拦死。

### Bug A (url 不完整)
`src-tauri/src/gateway/proxy/handler.rs:186`:
```rust
log.request_url = req.uri().to_string();
```
HTTP origin-form URI 仅含 path 段 (无 host)。MITM 解密灌入与 forward proxy absolute-form 都进同一 handler。完整 url 须从 Host header 重构: `format!("https://{}{}", host_header, path_and_query)`。

参考 `passthrough.rs:354-364` 已实现 host 重构 (`log.upstream_request_url`), 可抽公共 helper 或 inline。

## 修法 (推荐)

### Bug B 修法
`should_fallback_passthrough` 调整判定顺序 — host 判定**前置**:
1. 先跑 host 判定 (loopback 名 / listen ip 比对, 现有逻辑不变)
2. host 非自身 → 直接 return true (MITM 解密灌入, 不看 path)
3. host 自身 → 走 is_api_endpoint → false (保留 404 语义)

即把 `is_api_endpoint` early return 移到 host 自身判定之后。最小 diff, 不破坏现有 4 类 host 判定分支。

### Bug A 修法
`handler.rs:186` 重构 url 含 host:
- 从 req.headers().get(HOST) 取 host_header
- scheme 推导 (orig_uri.scheme_str 或默认 https; 与 passthrough.rs:363 同款)
- `log.request_url = format!("{scheme}://{host_header}{path_and_query}")`
- 缺 Host header 时 fallback 当前 path-only 行为 (不破坏)

可抽 `passthrough.rs` 现有 host 重构为 helper 复用 (避免双份)。

## Acceptance Criteria

- [ ] MITM 解密灌入请求 (host=上游域名, path=/api/anthropic/v1/messages, Authorization=上游真实 key) → 透明转发原始 host → 不再 404
- [ ] 客户端直连代理自身 (host=127.0.0.1:port) + /api/... + 无效 token → 仍 404 (保留语义)
- [ ] proxy_log.request_url 字段含完整 url (scheme://host/path?query) 不只 path
- [ ] cargo test (含 test_endpoint.rs 既有 24 用例 + 新增覆盖) 全绿
- [ ] cargo clippy 无新 warning

## Out of Scope

- passthrough 流量计费 (UNMATCHED_GROUP_KEY, cost=0, 维持现状)
- 新增 forward proxy absolute-form url 测试 (现有逻辑已工作, 仅 MITM origin-form 缺 host)

## Technical Notes

- 文件: endpoint.rs (Bug B 核心修), handler.rs:186 (Bug A), 可能 passthrough.rs (抽 helper)
- 现有 test_endpoint.rs 24 用例覆盖 should_fallback_passthrough 各分支, 改顺序后需补 "MITM host + api endpoint path → true" 用例
- 上游真实 key 不外传 (脱敏处理), 测试用 mock
