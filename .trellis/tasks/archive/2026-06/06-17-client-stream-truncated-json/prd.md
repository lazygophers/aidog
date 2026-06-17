# 客户端 SSE 流截断 JSON Parse error (长 thinking/tool_use 流被总超时砍)

## 症状

客户端 (Claude Code) 报 `API Error: JSON Parse error: Unterminated string`，可重现。多见于长响应（扩展思考 thinking / 大 tool_use）。

## 根因（已定位，有 DB + 代码证据）

1. DB `setting: proxy|timeout = {"connect_timeout_secs":10,"request_timeout_secs":60}`。
2. `build_http_client`（`http_client.rs:54`）设 `reqwest.builder.timeout(Duration::from_secs(60))`。
3. **reqwest `.timeout()` 是总超时，覆盖「连接 → 响应头 → 响应 body 全部读完」**（reqwest 0.12 文档）。
4. 流式响应（SSE）的 body 读取计入该总超时。扩展思考/大 tool_use 流 body 读取 > 60s → **reqwest 在 60s 砍断上游流** → 无 `message_stop` → 代理 closure 捕获上游 Err → 合成 `message_stop` 发客户端 → 客户端拿到**残缺流**（中途断在 thinking_delta / input_json_delta 中间）→ 解析出 `Unterminated string` 或内容不完整。

DB 证据：近期 GLM anthropic 流中**唯一截断**的 `6ba4a2f2`（298KB / 68s / 无 message_stop，尾部断在 `thinking_delta "native"` 中间）正是最长那条；其余 ≤47s 全完整。强相关。

> 注：`stream-error-graceful-passthrough`（06-16）的「上游 Err 合成 message_stop」本身没错（避免 CC "error decoding response body"），但它暴露了**上游被超时砍**这个更上游的根因——本任务修根因（超时），非改 graceful close。

## 修复

**流式上游请求禁用总超时**（`timeout_secs=0`），保留 `connect_timeout`（连接期仍保护）。非流式维持现有 `request_timeout_secs`。

| # | 文件:行 | 改动 |
|---|---|---|
| 1 | `proxy.rs` convert 路径 client 构建（~1199-1203，`handle_proxy_inner`） | `is_stream` 为真时传 `req_timeout=0` 给 `build_http_client`（connect_timeout 不变）。`is_stream` 在该处已由请求体解析得知。 |
| 2 | `proxy.rs` passthrough 路径 client 构建（~1922-1927，`handle_passthrough`） | passthrough 是透明 relay，**默认禁总超时**（`req_timeout=0`）：透传不应施加任意 body 超时，客户端自有超时兜底；connect_timeout 保留。 |

`build_http_client` 本身不动（`timeout_secs>0` 才设 `.timeout()`，传 0 即禁用，语义已就绪）。

## 不改

- `stream-error-graceful-passthrough` 的 graceful close 逻辑（合成 message_stop）保留——它是上游真断（网络/上游自身限流）时的兜底，仍需要。本任务只消除「代理自己用总超时砍合法长流」这一人祸。
- 非流式请求超时不变（60s 对非流式 API 调用合理）。
- convert/passthrough 之外的工具请求（models / count_tokens / model_test）超时不变。

## 验收

- [ ] 流式请求（含长 thinking / 大 tool_use）body 读取 > 60s 不再被代理砍断（reqwest 不再 60s 超时）。
- [ ] 非流式请求仍受 request_timeout_secs 约束。
- [ ] connect_timeout 仍生效（连接期保护）。
- [ ] `cargo clippy` 0 warning（除已接受 block v0.1.6 future-incompat）。
- [ ] `cargo test` 现有用例不破。
- [ ] dev 验证（用户）：重现场景下长流不再截断、客户端不再 Unterminated string（留用户 dev 验）。

## 风险

- 流式禁总超时后，若上游真挂起（发完头不再发 body），连接会挂到客户端断开。可接受（客户端 Claude Code 自有超时；且 graceful close 仍处理上游 Err）。
- passthrough 非流式请求也禁了总超时——非流式 relay 挂起会挂到客户端断开。可接受（relay 语义：透传，不替客户端决策超时）。
