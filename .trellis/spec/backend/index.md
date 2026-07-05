---
updated: 2026-07-05
rewrite-version: 1
authored-by: trellisx-spec
mode: optimize
---

# Backend Development

何时被读: 任何涉及 `src-tauri/` 的任务规划 / 代码改动（尤其 DB schema / 模型 / CRUD）
谁读: main / sub-agent
不遵守的代价: schema 漂移 → 前后端契约断裂 / 数据不一致 / 迁移失败

---

## Index

- [DB Conventions](./db-conventions.md) — 数据库表设计强制规范（命名 / 主键 / 时间 / 软删除 / 默认值），唯一 DB spec 入口
- [Mock Platform](./mock-platform.md) — mock 平台类型规范（extra.mock schema / 三层配置覆盖 / 5 协议响应 builder / error_mode 语义 / 拦截点 / 假 token）
- [Claude Code Passthrough](./claude-code-passthrough.md) — Claude Code 订阅纯透传平台类型（原始请求捕获 / 拦截点 / header 剔除 hop-by-hop 保留 Authorization / 不转换不注入 / proxy_log / base_url host 根约定）
- [Platform Error Handling](./platform-error-handling.md) — 平台失败处理契约（auto_disable 触发状态码 / 429 配额-限流按 message 分类禁按 type / 熔断解耦 / purge 只删 401-403 / last_error 存 message / **C6 stream 单向性禁 unwrap_or(false) 区分漏发与显式非流式** / **C7 空流空body 失败落上游真实首块截断**）
- [Proxy CONNECT Relay](./proxy-connect-relay.md) — HTTP CONNECT 隧道契约（axum 0.8 method 早期分流禁 .route / hyper-util downcast TokioIo<TcpStream> / 预读 buf flush 防 ClientHello 丢字节 / upsert_connect_log 独立路径不污染 stats_agg / group_key 列名）
- [HTTP Client Forward](./http-client-forward.md) — 上游转发 reqwest client 契约（**build_http_client use_proxy=false 分支必须 .no_proxy() 禁 env proxy**, 否则读 HTTPS_PROXY env 指向自己形成 CONNECT 隧道无限递归 → h2 stream CANCEL; use_proxy=true 的 .proxy(explicit) 自动禁 env 无需额外调用; 共享函数一处修复全部 forward 调用点受益)
- [Proxy Diagnostic Headers](./proxy-diagnostic-headers.md) — debug build 诊断响应 header 注入契约（**MUST 复用 headers.rs::inject_trace_header helper**, id 取值链 current_trace_id→new_trace_id, header 名小写, **blind_relay 物理豁免** TCP 字节透传不可注入, release LLVM dead branch 消除 0 开销)
