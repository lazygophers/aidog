---
updated: 2026-07-08
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
- [DB Connection Resilience](./db-connection-resilience.md) — tokio_rusqlite 连接韧性契约（**MUST `call_traced`/`call_read_traced` 检测 `ConnectionClosed` 自动重连重试 1 次**, 写连接 reopen 替换槽位 / 读连接 pool.pick 轮询, 内存库跳过, FnOnce cell 重取, warn 日志反向定位 panic 源)
- [Mock Platform](./mock-platform.md) — mock 平台类型规范（extra.mock schema / 三层配置覆盖 / 5 协议响应 builder / error_mode 语义 / 拦截点 / 假 token）
- [Claude Code Passthrough](./claude-code-passthrough.md) — Claude Code 订阅纯透传平台类型（原始请求捕获 / 拦截点 / header 剔除 hop-by-hop 保留 Authorization / 不转换不注入 / proxy_log / base_url host 根约定）
- [Platform Error Handling](./platform-error-handling.md) — 平台失败处理契约（auto_disable 触发状态码 / 429 配额-限流按 message 分类禁按 type / 熔断解耦 / purge 只删 401-403 / last_error 存 message / **C6 stream 单向性禁 unwrap_or(false) 区分漏发与显式非流式** / **C7 空流空body 失败落上游真实首块截断**）
- [Platform Lifecycle](./platform-lifecycle.md) — 平台生命周期契约（**delete_platform MUST 软删 platform + 清所有 group_platform + 禁连带销毁任何分组含孤儿 auto 组**, purge_auto_disabled 复用 delete_platform 同步语义, 空组保留交用户手动清, force_delete_group 仅 delete_group 等场景调用）
- [Proxy CONNECT Relay](./proxy-connect-relay.md) — HTTP CONNECT 隧道契约（axum 0.8 method 早期分流禁 .route / hyper-util downcast TokioIo<TcpStream> / 预读 buf flush 防 ClientHello 丢字节 / upsert_connect_log 独立路径不污染 stats_agg / group_key 列名）
- [Proxy Fallback Host Routing](./proxy-fallback-host-routing.md) — handler fallback 路由判定契约（**MUST host self 判定前置于 path/is_api_endpoint**, MITM 解密灌入 host=上游域名 path=上游真实 API path 仅靠 host 区分, 禁 path 早返拦死 /api/... 上游 API)
- [Proxy Forward Absolute-Form](./proxy-forward-absolute-form.md) — forward proxy absolute-form HTTP/HTTPS forward 契约（**MUST Router 顶层 middleware 识别 `scheme_str() && host()` 绕过 `.route("/")` 健康端点劫持**, **scheme 自适应 `unwrap_or("https")` 不硬编码**, **复用 handle_proxy + 虚拟桶「未匹配」与 MITM fallback 同语义**, 与 CONNECT relay 互为对偶）
- [HTTP Client Forward](./http-client-forward.md) — 上游转发 reqwest client 契约（**build_http_client use_proxy=false 分支必须 .no_proxy() 禁 env proxy**, 否则读 HTTPS_PROXY env 指向自己形成 CONNECT 隧道无限递归 → h2 stream CANCEL; use_proxy=true 的 .proxy(explicit) 自动禁 env 无需额外调用; 共享函数一处修复全部 forward 调用点受益)
- [Proxy Diagnostic Headers](./proxy-diagnostic-headers.md) — debug build 诊断响应 header 注入契约（**MUST 复用 headers.rs::inject_trace_header helper**, id 取值链 current_trace_id→new_trace_id, header 名小写, **blind_relay 物理豁免** TCP 字节透传不可注入, release LLVM dead branch 消除 0 开销)
- [Logging Format](./logging-format.md) — 日志格式 + traceid 取值链契约（**MUST 5 段字段顺序** time/level/file:line func/msg/traceid, **console ANSI on / file ANSI off**, **traceid 三级兜底** span scope→thread-local→gen, **id 6 [0-9a-z] 多级 `.`**, **base36 双轨映射** proxy 请求反查, **spawn_traced MUST** 异步分支禁裸 tokio::spawn, 健康端点 span MUST, 与 header spec 互为对偶）
- [Platform Logo Sync](./platform-logo-sync.md) — 协议 logo 三路 fallback 契约（**MUST 顺序固定** simpleicons slug → favicon → clearbit 首成功即止, **命中=size>0** 空文件视 miss, **统一 .png** ICO 也强存, **0 字节拒写** 防永久污染, **复用 build_http_client_system** 禁 env proxy 防递归, presets JSON 读取同 get_defaults_json 优先级独立 include_str!）
