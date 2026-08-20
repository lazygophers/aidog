---
title: 上游转发 reqwest client 契约
layer: recall
category: proxy
keywords: [reqwest,no_proxy,http_client,forward,env,递归]
source: trellis
authored-by: skein-memory
created: 1783832114
---

# HTTP Client Forward (上游转发)

何时被读: 改 `src-tauri/src/gateway/http_client.rs` (build_http_client) / `proxy/forward.rs` / `proxy/passthrough.rs` / `proxy/responses.rs` / `proxy/count_tokens.rs` 任一上游转发调用点时
谁读: main / sub-agent
不遵守的代价: reqwest 读 env proxy 指向 AirDog 自身 → CONNECT 隧道无限递归 → h2 stream CANCEL (err 8) / 资源耗尽。h2-cancel task (`07-05-h2-passthrough-stream-cancel`) 实证。

---

## 禁 env proxy 契约 (MUST)

> 违反代价: AirDog 自身是代理 (监听 :9892), 转发上游时若 reqwest 读 `HTTPS_PROXY`/`HTTP_PROXY` env → env 必指向自己 (用户为让流量走 AirDog 而设) → `CONNECT host:443` 回到 AirDog → 再 MITM → 再 forward → 再读 env → **CONNECT 隧道无限递归** → hyper h2 server 资源耗尽 → stream RST_STREAM → 客户端报 `HTTP/2 stream ... CANCEL (err 8)`。

- **`build_http_client` 的 `use_proxy=false` 分支必须显式 `.no_proxy()`** — reqwest 0.12 默认 `auto_sys_proxy=true`, 会读 `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` env。`use_proxy=false` 表示用户 DB 无显式 proxy 配置, 此时若不调 `.no_proxy()` → reqwest 仍读 env → 递归环。
- **`use_proxy=true` 分支调 `.proxy(explicit)` 即自动禁 env** — reqwest 源码 (`client.rs:1414-1417`): `.proxy(explicit)` 自动置 `auto_sys_proxy=false`。无需额外 `.no_proxy()`。
- **共享函数一处修复** — `build_http_client` 是所有上游转发调用点的唯一 client 构建入口 (`forward.rs` / `passthrough.rs` handle_passthrough + forward_passthrough_to_orig_host / `responses.rs` / `count_tokens.rs`), 修此一处全部受益。
- **上游连通性由用户 DB 显式 proxy 负责** — env proxy 是用户让客户端流量走 AirDog 的手段, 不是 AirDog 自身转发上游的通道。AirDog 转发上游永远直连或走 DB 显式配 proxy, 禁走 env。

## 为何 502 路径不触发 / 200 路径触发

- **502 路径** (上游 `nonexistent.invalid`): reqwest 走 env proxy 回 AirDog → AirDog CONNECT 不可达 host → TCP/DNS 立即失败 → 快速 502 终结, 不形成递归 (上游 host 本身不可达)。
- **200 路径** (baidu 等可达 host): 每层 CONNECT 成功建 TLS 隧道 → 每层 forward 再走 env proxy → 递归展开至资源耗尽 → CANCEL。
- **教训**: forward 单测不能只覆盖 502/失败路径, 200 成功路径的递归根因会被「上游不可达立即失败」掩盖。

## 验证

```bash
# use_proxy=false 分支有 .no_proxy()
grep -n "no_proxy" src-tauri/src/gateway/http_client.rs  # 期望 use_proxy=false 分支命中

# 单测: stub proxy 计数=0 证 env proxy 禁用 (设 HTTPS_PROXY env 指向 stub proxy, 经 build_http_client(use_proxy=false) 请求上游, stub proxy 不被连)
cd src-tauri && cargo test build_http_client_disables_env_proxy_when_no_db_proxy -- --nocapture
```
