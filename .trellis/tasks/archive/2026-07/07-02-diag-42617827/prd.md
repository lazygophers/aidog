# PRD — CONNECT 隧道 target 空根因修复（42617827/5c3b521b 502）

## 现象（research 实证 2026-07-02）
- request_id `42617827` / `5c3b521b` + 同批 4 个 = 6 连发 502（15s 窗口，~5s 退避）
- `source_protocol=http-connect`，全字段空，`request_url` 空，`duration_ms=0`
- 全库 http-connect 仅 6 行全 502，**CONNECT 隧道自上线（024a04c4）从未成功一条**
- 详见 `research/diag-42617827.md`

## 根因（用户已确认认同）
`connect.rs:29` `let target = req.uri().path().to_string();` —— CONNECT 是 authority-form URI（`host:port`），axum 0.8 / hyper 1 对此 path() 返空（http 标准：authority-form URI path 段为空，authority 在 `uri().authority()`）。
→ `target=""` → `TcpStream::connect("")` 必败 → 502。
DB 双向证据闭合：`connect.rs:51` target→request_url，6 行 request_url 全空 ⇒ target=""。

## 决策锁（2026-07-02 AskUserQuestion 用户裁定）
| # | 决策 | 锁定 |
|---|---|---|
| 1 | 根因 | **target 空**（path() 取 authority-form URI 必空）— 用户认同 |
| 2 | 方案 | **A — 多源取 target + 空早返 400 + tracing 取证**（合并 B） |

## 修复（单文件 `src-tauri/src/gateway/proxy/connect.rs`）
**L29 段**改为多源取 target + 空兜底：
```rust
// ponytail: target 多源兜底 — CONNECT 是 authority-form URI，path() 返空（http 标准），
// authority 在 uri().authority()；补 Host header 兜底。三源皆空 = 客户端坏请求，早返不落 connect 路径。
let target = {
    let from_path = req.uri().path().trim_start_matches('/');
    let from_auth = req.uri().authority().map(|a| a.as_str()).unwrap_or("");
    let from_host = req.headers().get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok()).unwrap_or("");
    from_path.or_if_empty(from_auth).or_if_empty(from_host).to_string()
};
if target.is_empty() {
    tracing::warn!(uri = ?req.uri(), method = ?req.method(), "connect: missing target");
    // 取证：落原始 uri 到 response_body 供下次诊断（log.rs response_body 终态一次性落）
    return (StatusCode::BAD_REQUEST, "CONNECT missing target").into_response();
}
```
（注：`or_if_empty` 是示意，Rust 无此方法，exec 用 `if s.is_empty() { next } else { s }` 或 `[[path, auth, host]].iter().find(|s| !s.is_empty())` 实现；保持 ponytail 最短）

**tracing 取证**（合并候选 B）：进函数即 `tracing::info!(uri = ?req.uri(), method = ?req.method(), "connect recv")`，502/400 路径 warn 含原始 uri。下次复现可从 stderr/日志面板看真实 URI 结构。

**host_only**（L30）仍从 target rsplit_once(':') 取，多源取后 target 形如 `host:port`，rsplit_once 仍工作。

## 验收
1. `cargo test` 全绿（现有 test_connect.rs 不回归 + 新增 3 个测试，见下）
2. `cargo clippy` 零新 warning
3. tracing 落原始 uri（下次复现可取证）

## 测试设计（用户要求：跑测试验 proxy 环境变量可行；mock 渠道 + proxy env）

**L1 单元 — authority-form URI 解析（核心回归门，test_connect.rs 加）**
- `connect_authority_form_resolves_target`：起 mock TCP 上游 listener（`127.0.0.1:0`，accept 卡住不响应）→ 构造 `Request::builder().method(CONNECT).uri(format!("127.0.0.1:{port}"))`（http crate 解为 authority-form：path 空、authority=`host:port`）→ 调 `handle_connect(AxumState(state), req).await` → 断言 `resp.status()==200`（target 解析非空 + TCP 建连成功）。
  - 修复前必 502（path() 返空 → connect("") 失败）→ 修复后 200，证明根因消除。
  - upgrade future（`hyper::upgrade::on(req)` 无 server backing）在 spawn task 内 pending，测试只取 resp status 不 await task；runtime drop 自动清理。
- `connect_triple_source_empty_returns_400`：构造极端 Request（path/authority 皆空 + 仅 Host header，或全空）→ 断言 `resp.status()==400`（非 502，不进 connect 路径）。

**L2 端到端 — 真 axum proxy + proxy 环境变量（test_connect.rs 加）**
- `connect_tunnel_via_real_proxy_env`：
  1. 起 mock 上游 TCP echo server（`127.0.0.1:0`，accept 后 tokio::io::copy 双向回显）
  2. 起 aidog proxy axum server：`Router::new().fallback(handle_proxy).with_state(make_state(db))` + `tokio::net::TcpListener::bind("127.0.0.1:0")` + `axum::serve`（复用 start_proxy 的 Router 骨架，测试内联构建，bind 0 端口）
  3. 裸 `TcpStream` 连 proxy → 写 `CONNECT 127.0.0.1:{mock_port} HTTP/1.1\r\nHost: 127.0.0.1:{mock_port}\r\n\r\n` → 读响应首行 → 断言 `HTTP/1.1 200`（隧道建立）
  4. 隧道建后续写 `ping` → 读回 `ping`（验双向透传，可选；upgrade 后字节盲转）
  - 这覆盖用户场景：客户端配 `HTTP_PROXY/HTTPS_PROXY/ALL_PROXY=http://127.0.0.1:<proxy_port>` 后发 HTTPS 流量 → 客户端发 CONNECT → aidog 解 target → 建隧道。修复前 6 连发 502，修复后 200。
  - ponytail：不引 TLS（CONNECT 握手 + 200 在 TLS 之前；字节透传用裸 TCP echo 验，不需自签 cert）。reqwest 带 proxy 测真 HTTPS 需 TLS mock server，过度；裸 TcpStream 发 CONNECT 已覆盖根因 + upgrade 链路。

## 非目标
- 不改 P1 盲转语义（不解密 HTTPS，无 apikey 无组路由/计费）
- 不改 handler.rs:84 CONNECT 早期分流
- 不评估 ALL_PROXY 路径去留（候选 C，用户未选）
- 不改 upsert_connect_log 字段语义

## 风险
- axum 0.8 对 CONNECT authority-form 的 authority() 是否真返 `host:port`——http crate 标准行为应返，但若 axum 喂入时 authority 也空（极端），三源兜底 Host header 兜（CONNECT 通常带 Host 或 authority 至少一个）
- 修后隧道建立但 P1 盲转无 apikey，组路由/计费仍不工作（设计如此，非本 task 范围）

## 阶段
1. research（已完成，research/diag-42617827.md）
2. grill 用户确认根因 + 方案（已完成）
3. exec（单 subagent：connect.rs 多源取 target + tracing + 测试）
4. check（cargo test/clippy）
5. finish（spec sediment：CONNECT 隧道 target 取法是新契约 C8 候选）
