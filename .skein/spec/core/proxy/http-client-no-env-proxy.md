---
inclusion: auto
name: http-client-no-env-proxy
title: 上游转发 reqwest 禁读 env proxy
layer: core
category: proxy
keywords: [reqwest,no_proxy,http_client,forward,env,递归,CONNECT]
source: -
authored-by: skein-memory
created: 1722556800
---
---
# 上游转发 reqwest 禁读 env proxy

何时被读: `build_http_client` (上游转发客户端构建) + `forward.rs` / `passthrough.rs` / `responses.rs` / `count_tokens.rs` 任一调用时

不遵守的代价: reqwest 读 `HTTPS_PROXY`/`HTTP_PROXY` env → env 指向 AiDog 自身 (用户为让客户端流量走 AiDog 而设置的 env proxy) → CONNECT 隧道回到 AiDog 自身 → 无限递归 → h2 stream CANCEL (err 8) / 资源耗尽

---

## MUST 硬约束

- **`build_http_client` 的 `use_proxy=false` 分支必须显式 `.no_proxy()`**
  - reqwest 0.12 默认 `auto_sys_proxy=true`，会读 `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` env
  - `use_proxy=false` 表示用户 DB 无显式 proxy 配置，此时若不调 `.no_proxy()` → reqwest 仍读 env → 递归环

- **`use_proxy=true` 分支调 `.proxy(explicit)` 自动禁 env**
  - reqwest 源码：`.proxy(explicit)` 自动置 `auto_sys_proxy=false`
  - 无需额外 `.no_proxy()`

- **共享函数一处修复**
  - `build_http_client` 是所有上游转发调用点的唯一 client 构建入口 (forward.rs / passthrough.rs handle_passthrough / responses.rs / count_tokens.rs)
  - 修此一处全部受益，禁在转发层各自拼凑

## 反例

❌ **缺 `.no_proxy()` 导致递归**：
```rust
if use_proxy {
    builder.proxy(explicit)
} else {
    // 缺少 .no_proxy() → reqwest 仍读 env → CONNECT 回自己
}
```

✅ **正确做法**：
```rust
if use_proxy {
    builder.proxy(explicit)
} else {
    builder.no_proxy()
}
```

## 症状差异（为何 502 路径不触发）

- **502 路径**（上游不可达）：reqwest 走 env proxy 回 AiDog → CONNECT 不可达 host → TCP/DNS 立即失败 → 快速 502 终结，不形成递归
- **200 路径**（百度等可达）：每层 CONNECT 成功 → 每层 forward 再走 env proxy → 递归展开至资源耗尽 → CANCEL
- **教训**：单测不能只覆盖 502 失败路径，200 成功路径的递归根因会被掩盖

## 验证

```bash
# use_proxy=false 分支必有 .no_proxy()
grep -A5 "use_proxy.*false" src-tauri/crates/aidog_core/src/gateway/http_client.rs | grep no_proxy

# 单测验证（HTTPS_PROXY env 指向 stub proxy，构建后 no_proxy 禁用，stub 不被连）
cd src-tauri && cargo test build_http_client_disables_env_proxy_when_no_db_proxy
```

## 关联

[[auto-disable-401-403-402]]
