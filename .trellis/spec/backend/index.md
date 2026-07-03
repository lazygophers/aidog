---
updated: 2026-06-11
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
