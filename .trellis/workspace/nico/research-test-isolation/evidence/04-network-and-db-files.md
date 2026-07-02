# Evidence: 网络 / 真实 DB 文件 / 真实 HTTP 请求

---

## 1. 网络 — `gateway/proxy/test_integration.rs:677-678` 连接未监听端口（中严重度）

**文件**: `src-tauri/src/gateway/proxy/test_integration.rs:670-685`

```rust
#[tokio::test]
async fn responses_endpoint_dead_upstream_returns_5xx() {
    let state = make_state(test_db().await).await;
    setup_responses_group(&state, "gkrdead", "http://127.0.0.1:1").await;  // ← 死端口

    let req = responses_get("gkrdead", "/v1/responses/resp_x");
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert!(resp.status().is_server_error());
}
```

**问题**: 真实发起 TCP 连接到 `127.0.0.1:1`（几乎必然 connection refused），测网络错误路径。**是真实出站连接尝试**，但目标是 localhost 未用端口，不触外网、不触用户数据。

**判定**: 中严重度（用户清单第6项「测试发真实 HTTP 请求」字面违规，但无副作用）。

**修复建议**: 改用 `spawn_stub_upstream(599, "")` 启本地 stub server 模拟错误，或在断言中接受 `is_server_error()` 即可不强求真死连接。

**注**: 同文件 `http://unused.invalid`（line 242）**不违规**——该测试是 `GET /v1/models`，走静态模型端点（注释「不 relay 上游」），从不实际连 unused.invalid。

---

## 2. 真实 DB 文件（tempdir，**非违规**）

以下测试用 `Db::new(<tempdir>/xxx.db)` 而非 `:memory:`，落在 `std::env::temp_dir()`（系统临时目录，非用户 `~/.aidog`）：

| file:line | 说明 |
|---|---|
| `gateway/db/test_rw_pool.rs:55-64` | 文件库读池能见 WAL 提交（需文件库特有行为） |
| `gateway/db/test_rw_pool.rs:91-99` | 并发读写（同上） |
| `gateway/proxy/test_log.rs:46-54` | flush 时序测试（需文件持久化行为） |
| `gateway/proxy/test_stream.rs:186-194` | 流式 flush（同上） |
| `gateway/import_export/ccswitch/read/test_read.rs:233-248` | 读外部 cc-switch.db（tempdir 内自建） |
| `gateway/import_export/ccswitch/read/test_read.rs:258-275` | 读 config.json（tempdir 内自建） |

**判定**: **不违规**。均在 `std::env::temp_dir()`（macOS `/var/folders/.../T/`），非用户 `~/.aidog`，且测试结束清理（`std::fs::remove_file`）。语义需要文件库行为（WAL / 跨连接可见性），`:memory:` 无法替代。

---

## 3. reqwest::Client::new()（**非违规**）

`gateway/proxy/test_headers.rs:460-598`（15 处）、`gateway/proxy/test_passthrough.rs:204`:

```rust
let client = reqwest::Client::new();
let rb = client.post("http://localhost");
let rb = apply_client_headers(rb, ...);
let h = headers_from_builder(rb);   // rb.build().headers().clone()
```

**判定**: **不违规**。仅构造 RequestBuilder + `.build()` 取 HeaderMap，**从未调 `.send()`**。是纯 header 注入逻辑单测，无真实 HTTP 发送。

---

## 4. TcpListener 本地 stub server（**非违规**）

`gateway/quota/test_http.rs:15`、`gateway/proxy/test_integration.rs:23`、`gateway/proxy/test_group_info.rs`（隐式）:

```rust
let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
```

**判定**: **不违规**。`127.0.0.1:0` = 本机任意空闲端口，启动 in-process stub server 给测试本身用（axum::serve），不触外网、不触用户数据。是标准的 in-process 测试模式。

---

## 统计

- 中严重度违规（真实死连接）: 1 处（test_integration.rs:677）
- 合法的 tempdir DB 文件: 6 处
- 合法的 builder-only reqwest: 16 处
- 合法的本地 stub server: 3 处
