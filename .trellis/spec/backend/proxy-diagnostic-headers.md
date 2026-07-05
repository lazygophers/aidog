---
updated: 2026-07-05
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# Proxy 诊断响应 Header (debug build)

何时被读: 改 `src-tauri/src/gateway/proxy/` 的**响应构造点** / 新加诊断响应 header 时
谁读: main / sub-agent
不遵守的代价: 47 注入点逐个手写 cfg gate 漂移 / blind_relay 误注入破 TLS 字节流 / id 取值发散失诊断关联。`07-05-proxy-trace-id-header` 实证。

---

## Helper 复用契约 (MUST)

> 违反代价: 各响应构造点重复实现 `cfg!(debug_assertions)` gate, 新加诊断 header 时 47 处逐个改, 漂移 / 遗漏站点 / id 取值发散。

- **MUST 复用 `headers.rs::inject_trace_header(&mut axum::response::Response)`** —— 唯一诊断 header 注入入口, gate + id 取值 + header 名规范全收敛于此
- **禁各响应构造点重复实现 `cfg!(debug_assertions)` gate** —— 改加新诊断 header 时只动 helper 一处, 47 调用点自动受益
- **新加诊断响应 header (除 `x-aidog-trace` 外) 也走同一 helper** —— 在 helper 内部加 header 写入, 调用点不变
- **MITM 明文服务路径** (`connect.rs` 用 `hyper::Response<Builder>` 直构处, helper 抽 `axum::Response` 类型不匹配): 内联等价注入, **禁省略** —— 内联点写明"等价 inject_trace_header, hyper::Response 类型差异", 方便 grep 找全

## id 取值链 (MUST)

> 违反代价: 各处自造 id 失去与 proxy_log / span 的关联, 诊断时无法客户端报错 → AirDog 侧日志映射。

- **id 取值顺序 MUST**: `crate::logging::current_trace_id()` → `unwrap_or_else(crate::logging::new_trace_id)`
- **禁自造固定常量** id (如 `"-"` / `"unknown"`) —— 失关联
- **禁各响应点独立 `new_trace_id()`** —— 同一请求的响应 id 必须能映射回请求 span 的 request_id / trace_id; `current_trace_id()` 读线程活跃 span 链最内层 id, 自然继承
- request_id (代理请求 span, 32-hex = proxy_log.id) 优先于 trace_id (命令 / 后台 span, 8-hex), 由 `TraceIdLayer` 维护线程本地栈自动选

## header 名规范 (MUST)

- **header 名 MUST 小写** (`x-aidog-trace` 等), 用 `HeaderName::from_static` —— HTTP header 不区分大小写, 小写合 hyper / axum 规范, `from_static` 编译期校验合法
- 自定义诊断 header prefix 建议 `x-aidog-*`, 与应用自有命名空间一致

## blind_relay 物理豁免 (MUST NOT)

> 违反代价: blind_relay 是 CONNECT 隧道建好后 TCP 字节透传, AirDog 看见的是加密 TLS 字节流 —— 尝试注入 header 会破坏字节流致客户端 TLS 解析失败。

- **`connect.rs::blind_relay_after_connect` MUST NOT 注入响应 header** —— 字节透传非 AirDog 构造响应, header 物理不可注入
- **CONNECT 200 OK 响应本身可注入** —— 那是 AirDog 直构的 `Response::builder()` 响应, 不走 blind_relay 字节透传
- **豁免处 MUST 加注释** 标明"TCP 字节透传非 AirDog 构造响应, header 物理不可注入", 防 grep 验收时误判漏注入

## release build 行为 (MUST)

- **release build MUST 不注入** —— helper 内 `if cfg!(debug_assertions) { ... }` runtime gate, release 编译时常量 `false` 分支经 LLVM dead branch elimination 消除, 0 开销
- **禁在调用点再加 `#[cfg(debug_assertions)]` compile-time gate** —— helper runtime gate 已足够, 重复 gate 是过度设计

## 验收基准 (可复用)

- [ ] debug build: 所有 AirDog **直构**响应含诊断 header (grep `inject_trace_header` 计数 + `into_response()` / `Response::builder()` 返路径核全覆盖)
- [ ] release build: 0 header 注入 (helper 编译期消除)
- [ ] blind_relay: 物理豁免 + 注释 (grep 标注)
- [ ] `cargo clippy --lib` 0 warning
- [ ] 新增诊断 header 单测 (helper 行为, 不依赖编译模式)

## 验证命令

```bash
# helper 调用计数 (debug 注入点)
grep -rn "inject_trace_header" src-tauri/src/gateway/proxy/*.rs | grep -v test_ | wc -l

# 返路径遗漏检查 (每处返路径要么有 helper 调用, 要么 blind_relay 豁免注释)
grep -rn "\.into_response()\|Response::builder()\|ok_empty!" src-tauri/src/gateway/proxy/*.rs | grep -v test_

# blind_relay 豁免注释存在
grep -n "blind_relay\|物理不可注入\|字节透传" src-tauri/src/gateway/proxy/connect.rs
```

## 跨协议注入选址参考

`07-05-proxy-trace-id-header` 实施时枚举的 47 调用点分布:
- `handler.rs` / `forward.rs` / `finish.rs` / `passthrough.rs` (handle_passthrough + forward_passthrough_to_orig_host) / `responses.rs` / `count_tokens.rs` / `group_info.rs` (含 `ok_empty!` 宏内部) / `notify.rs` / `mock.rs` / `non_success.rs` / `log.rs` / `health.rs` / `connect.rs` (CONNECT 200 + MITM 明文服务返路径)
- 新加响应构造点 MUST 加入 helper 调用 (或 hyper::Response 类型处内联等价注入)
