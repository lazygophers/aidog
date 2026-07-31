# SKEIN core 规则索引 (章节粒度: 一行一条规则)

类目: arch(14), cross-layer(7), db(21), domain(5), frontend(3), i18n(4), perf(4), proxy(9) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | inclusion | anchors | status/出链 | summary |
|---|---|---|---|---|---|---|---|
| arch/mock-platform-bypasses-forward-pipeline.md#mock 平台绕开真实转发流水线，无法验证 finish.rs 挂载的 cap/累积逻辑 | arch | mock 平台绕开真实转发流水线，无法验证 finish.rs 挂载的 cap/累积逻辑 | mock,StreamAggregator,STREAM_BODY_MAX_BYTES,finish.rs,loadgen,footprint | auto | src-tauri/crates/aidog_core/src/gateway/proxy/handler.rs,src-tauri/crates/aidog_core/src/gateway/proxy/mock.rs,src-tauri/crates/aidog_core/src/gateway/proxy/stream.rs | active | - |
| arch/mock-platform-bypasses-forward-pipeline.md#关联 | arch | 关联 | mock,StreamAggregator,STREAM_BODY_MAX_BYTES,finish.rs,loadgen,footprint | auto | src-tauri/crates/aidog_core/src/gateway/proxy/handler.rs,src-tauri/crates/aidog_core/src/gateway/proxy/mock.rs,src-tauri/crates/aidog_core/src/gateway/proxy/stream.rs | active | proxy-hotpath-buffers s9-bigbody-footprint（`.scratch/perf-20… |
| arch/mock-platform-bypasses-forward-pipeline.md#硬约束 | arch | 硬约束 | mock,StreamAggregator,STREAM_BODY_MAX_BYTES,finish.rs,loadgen,footprint | auto | src-tauri/crates/aidog_core/src/gateway/proxy/handler.rs,src-tauri/crates/aidog_core/src/gateway/proxy/mock.rs,src-tauri/crates/aidog_core/src/gateway/proxy/stream.rs | active | `platform_type=mock`（`gateway/proxy/mock.rs::handle_mock`）在 … |
| arch/mock-platform-short-circuit.md#Mock 平台绕开转发流水线短路 | arch | Mock 平台绕开转发流水线短路 | mock,platform,short-circuit,proxy | auto | - | active | Mock 平台在转发流水线早期短路，不走真实上游请求逻辑。  - `handler.rs:412` `matches!(… |
| arch/protocol-wire-str.md#关联 | arch | 关联 | protocol,serde,wire,codegen,enum | always | - | active / →rule-05 | [[rule-05]] |
| arch/protocol-wire-str.md#案例 | arch | 案例 | protocol,serde,wire,codegen,enum | always | - | active | - gateway/models/protocol.rs:173 定义 wire_str() - arch-deepen… |
| arch/protocol-wire-str.md#正解 | arch | 正解 | protocol,serde,wire,codegen,enum | always | - | active | 统一用 `Protocol::wire_str()` 方法序列化协议名。 |
| arch/protocol-wire-str.md#触发场景 | arch | 触发场景 | protocol,serde,wire,codegen,enum | always | - | active | 在 proxy/forward 层需要获取协议名或序列化 Protocol enum 时。 |
| arch/protocol-wire-str.md#适用 | arch | 适用 | protocol,serde,wire,codegen,enum | always | - | active | - Protocol enum 序列化时 - adapter 分发时协议名判定 |
| arch/protocol-wire-str.md#陷阱 | arch | 陷阱 | protocol,serde,wire,codegen,enum | always | - | active | 禁手写 `serde_json::to_string(&x).trim_matches('"')` 或其他字符串转换，容… |
| arch/stream-buf-unified-cap.md#关联 | arch | 关联 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active / →hot-path-buffers,stream-buf-no-batching | [[stream-buf-no-batching]] [[hot-path-buffers]] |
| arch/stream-buf-unified-cap.md#案例 | arch | 案例 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active | **正例** —— usage 侧定义 `SSE_LINE_BUF_MAX_BYTES = 1MB`，内容侧转换分支引用… |
| arch/stream-buf-unified-cap.md#硬约则 | arch | 硬约则 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active | 同一隐患在代码库内多条执行路径出现时，上界值**禁止多处定义**。一处常量定义，其余路径引用 → 防止行为割裂。  ##… |
| arch/stream-buf-unified-cap.md#适用 | arch | 适用 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active | - 任何多路径并发处理同一数据流的缓冲 - 多个解析器共用一个上界（如 SSE / WebSocket 等流协议） - … |
| cross-layer/sole-platform-symmetry.md#单启用平台判定对称性 (Rust ↔ TS) | cross-layer | 单启用平台判定对称性 (Rust ↔ TS) | cross-layer,symmetry,sole_platform,Rust,TypeScript,判定对称 | always | - | active | - |
| cross-layer/sole-platform-symmetry.md#跨层对称硬规 (Rust ↔ TS) | cross-layer | 跨层对称硬规 (Rust ↔ TS) | cross-layer,symmetry,sole_platform,Rust,TypeScript,判定对称 | always | - | active | ### 约束  **同一判定逻辑在 Rust 与 TS 各有一份实现，改一处必改另一处。**  口径须与互指注释锁定对称… |
| cross-layer/tauri-ts-boundary-contract.md#三层契约 | cross-layer | 三层契约 | tauri,rust,typescript,invoke,snake_case,serde | auto | - | active | 1. **Rust struct 字段** → 2. **#[tauri::command] 签名** → 3. **前… |
| cross-layer/tauri-ts-boundary-contract.md#关联 | cross-layer | 关联 | tauri,rust,typescript,invoke,snake_case,serde | auto | - | active / →sole-platform-symmetry | [[sole-platform-symmetry]] |
| cross-layer/tauri-ts-boundary-contract.md#硬约则 | cross-layer | 硬约则 | tauri,rust,typescript,invoke,snake_case,serde | auto | - | active | - 新增 Tauri command 必须同时补前端 `src/services/api/<domain>.ts` in… |
| cross-layer/tauri-ts-boundary-contract.md#禁用 | cross-layer | 禁用 | tauri,rust,typescript,invoke,snake_case,serde | auto | - | active | ❌ 仅后端加 command，前端漏 invoke 包装 → 形同死代码   ❌ 字段非 snake_case → se… |
| cross-layer/tauri-ts-boundary-contract.md#验证（file:line） | cross-layer | 验证（file:line） | tauri,rust,typescript,invoke,snake_case,serde | auto | - | active | - `src-tauri/src/startup.rs:41+`：generate_handler! 注册表（invok… |
| db/connectionclosed-retry.md#关联 | db | 关联 | db,connection,call_traced,reconnect,pool,rusqlite | auto | - | active / →crash-safe-db-split,sqlite-read-cache-config | [[crash-safe-db-split]] [[sqlite-read-cache-config]] |
| db/connectionclosed-retry.md#根因 | db | 根因 | db,connection,call_traced,reconnect,pool,rusqlite | auto | - | active | `tokio_rusqlite` 0.6.0 特性：`Connection` 后台 event_loop 线程 pani… |
| db/connectionclosed-retry.md#硬约则 | db | 硬约则 | db,connection,call_traced,reconnect,pool,rusqlite | auto | - | active | - `call_traced`/`call_read_traced` 检测 `ConnectionClosed` MUS… |
| db/connectionclosed-retry.md#验证（file:line） | db | 验证（file:line） | db,connection,call_traced,reconnect,pool,rusqlite | auto | - | active | - `crates/aidog_core/src/gateway/db/mod.rs:526,1031`：重连入口 - … |
| db/crash-safe-db-split.md#拆库迁移四阶段 Crash-Safe 范式 | db | 拆库迁移四阶段 Crash-Safe 范式 | migration,crash-safe,multi-db,state-machine | auto | - | active | 多库分离迁移必须走四阶段状态机，确保任一阶段 crash 可恢复：  1. 新库建表 + 读旧库，写旧库 2. 后台增量… |
| db/db-table-conventions.md#Column Naming (MUST) | db | Column Naming (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | always | - | active | - 平台主类型列名为 `platform_type`（禁 `protocol`）；其值用 `serde_json::to… |
| db/db-table-conventions.md#Migration (MUST) | db | Migration (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | always | - | active | - schema 破坏式变更必须提供独立一次性迁移脚本（`scripts/`，非 app 运行时代码），迁移完成后删除 … |
| db/db-table-conventions.md#No NULL (MUST) | db | No NULL (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | always | - | active | - 所有 `TEXT` 列 `NOT NULL DEFAULT ''`；所有 `INTEGER` 列 `NOT NULL… |
| db/db-table-conventions.md#Primary Key (MUST) | db | Primary Key (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | always | - | active | - 业务表主键必须 `id INTEGER PRIMARY KEY AUTOINCREMENT`，Rust 映射 `u6… |
| db/db-table-conventions.md#Relations & Mappings (MUST) | db | Relations & Mappings (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | always | - | active | - 关联表（如 `group_platform`）加代理 `id` 自增主键 + 保留业务复合 `UNIQUE(grou… |
| db/db-table-conventions.md#Soft Delete (MUST) | db | Soft Delete (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | always | - | active | - 删除必须逻辑删：`UPDATE <table> SET deleted_at = <now_ms> WHERE id… |
| db/db-table-conventions.md#Table Naming (MUST) | db | Table Naming (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | always | - | active | - 表名必须**单数**，禁复数：`platform` / `group` / `group_platform` / `… |
| db/db-table-conventions.md#Time Fields (MUST) | db | Time Fields (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | always | - | active | - 每个表必须含 `created_at` / `updated_at` / `deleted_at`，类型 `INTE… |
| db/db-table-conventions.md#Verification | db | Verification | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | always | - | active | ```bash # 复数表名残留 sqlite3 ~/.aidog/aidog.db ".tables" / grep … |
| db/db-table-conventions.md#专属表 → setting 迁移模式 (MUST) | db | 专属表 → setting 迁移模式 (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | always | - | active | 域数据从专属表迁通用 `setting` 表时（`scope=<域>, key=<实体>` JSON），走 app 内置… |
| db/sqlite-connection-resilience.md#反例（禁） | db | 反例（禁） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | always | - | active | - 禁在 handler 层才重试 route（只覆盖 route 路径，写连接死亡无法兜底；Db 层统一兜底全覆盖）。… |
| db/sqlite-connection-resilience.md#契约（MUST） | db | 契约（MUST） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | always | - | active | - `call_traced` / `call_read_traced` 检测 `Error::ConnectionCl… |
| db/sqlite-connection-resilience.md#根因（tokio_rusqlite 0.6.0 已知行为，库层不可改） | db | 根因（tokio_rusqlite 0.6.0 已知行为，库层不可改） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | always | - | active | - `Connection` 内部 `event_loop`（`tokio-rusqlite-0.6.0/src/lib… |
| db/sqlite-connection-resilience.md#验证（可 grep / 可 test） | db | 验证（可 grep / 可 test） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | always | - | active | - `grep -n "ConnectionClosed\/reopen_write_conn\/pool.pick" … |
| db/sqlite-read-cache-config.md#SQLite 只读缓存定值 | db | SQLite 只读缓存定值 | sqlite,cache,readonly,memory,hardcoded | auto | - | active | 通过 `PRAGMA cache_size = -64` 限制每条只读连接的页缓存驻留，实测指标达标。  ### 硬约束… |
| db/sqlite-read-cache-config.md#关联 | db | 关联 | sqlite,cache,readonly,memory,hardcoded | auto | - | active / →sqlite-cache-residency-probe-method | [[sqlite-cache-residency-probe-method]] |
| domain/delete-platform-no-cascade.md#delete_platform 软删禁连带删组 | domain | delete_platform 软删禁连带删组 | cascade,lifecycle,platform,group | auto | - | active | `delete_platform` 仅软删平台，禁物理删，且禁连带删关联组。  - `db/platform_lifec… |
| domain/peak-multiplier-symmetry.md#关联 | domain | 关联 | peak,multiplier,estimate,budget,symmetry,billing,财务 | always | - | active / →rule-66,time-tiers-apply-idiom | [[rule-66]] [[time-tiers-apply-idiom]] |
| domain/peak-multiplier-symmetry.md#硬约则 | domain | 硬约则 | peak,multiplier,estimate,budget,symmetry,billing,财务 | always | - | active | estimate 流程中**任意处加 peak 倍率，对边必补同倍率**（既存 bug 根因）：  - 余额扣减（`es… |
| domain/peak-multiplier-symmetry.md#禁用 | domain | 禁用 | peak,multiplier,estimate,budget,symmetry,billing,财务 | always | - | active | ❌ 仅余额扣减乘倍率，手动预算漏乘 → 成本显示 ≠ 实际扣款   ❌ 仅某处乘倍率，其他相关路径不补 → 高峰期估算 … |
| domain/resolve-price-now-ms.md#resolve_price 末位 now_ms 传值约定 | domain | resolve_price 末位 now_ms 传值约定 | billing,pricing,cache,timestamp | auto | - | active | `resolve_price` 最后一个参数 `now_ms: i64` 为价表缓存的时间戳校验位。各调用点传值约定： … |
| frontend/tailwind-cascade-layer-base.md#硬约则 | frontend | 硬约则 | tailwind,layer,css,reset,global | auto | - | active | `src/styles/globals.css` 中 UA reset 和全局元素规则（如 body/html）MUST… |
| frontend/tailwind-cascade-layer-base.md#禁用 | frontend | 禁用 | tailwind,layer,css,reset,global | auto | - | active | ❌ v3 风格 `@tailwind base/components/utilities` → v4 语法错误 |
| frontend/tailwind-cascade-layer-base.md#验收 | frontend | 验收 | tailwind,layer,css,reset,global | auto | - | active | - `src/styles/globals.css:4-6` 无任何 `@tailwind` 指令 - `package… |
| i18n/i18n-key-sync-8lang.md#关联 | i18n | 关联 | i18n,locale,zh-Hans,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES | auto | - | active / →zh-hans-literal-sync | [[zh-hans-literal-sync]] |
| i18n/i18n-key-sync-8lang.md#硬约则 | i18n | 硬约则 | i18n,locale,zh-Hans,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES | auto | - | active | `src/locales/` 8 个 locale 文件 MUST 保持 key 集合等值：  - **语言**：zh-… |
| i18n/i18n-key-sync-8lang.md#禁用 | i18n | 禁用 | i18n,locale,zh-Hans,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES | auto | - | active | ❌ 漏某语言 → 用户切该语言见裸 key   ❌ 模板变量未展开 → 动态内容显示变量本身 |
| i18n/i18n-key-sync-8lang.md#验收 | i18n | 验收 | i18n,locale,zh-Hans,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES | auto | - | active | ```bash yarn check:i18n  # 4 类检查 + 清单输出 # 期望 exit 0 ``` |
| perf/hot-path-buffers.md#mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝 | perf | mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝 | mpsc,capacity,try_send,背压,深拷贝,热路径,TOCTOU,hotspot,frequency,profiling,clone | auto | src-tauri/crates/aidog_core/src/gateway/proxy/log.rs | active | mpsc 队列热路径丢弃分支：先 `Sender::capacity() == 0` 判队满再 return，避免为「确… |
| perf/hot-path-buffers.md#热点判定维度：调用频次优先于字节量 | perf | 热点判定维度：调用频次优先于字节量 | mpsc,capacity,try_send,背压,深拷贝,热路径,TOCTOU,hotspot,frequency,profiling,clone | auto | src-tauri/crates/aidog_core/src/gateway/proxy/log.rs | active | ### 核心决策  **热点判定的决定变量是调用频次（每请求 N 次），不是单次操作的字节量。**深拷贝值不值得优化，取… |
| perf/hot-path-buffers.md#热点判定维度：调用频次优先于字节量 | perf | 热点判定维度：调用频次优先于字节量 | mpsc,capacity,try_send,背压,深拷贝,热路径,TOCTOU,hotspot,frequency,profiling,clone | auto | src-tauri/crates/aidog_core/src/gateway/proxy/log.rs | active | - |
| perf/stream-buf-no-batching.md# | perf | - | - | auto | - | active | --- |
| proxy/auto-disable-401-403-402.md#关联 | proxy | 关联 | auto-disable,401,403,402,stateless,throttle | auto | - | active / →http-client-no-env-proxy,mock-platform-short-circuit | [[mock-platform-short-circuit]] [[http-client-no-env-proxy]] |
| proxy/auto-disable-401-403-402.md#硬约则 | proxy | 硬约则 | auto-disable,401,403,402,stateless,throttle | auto | - | active | 平台自动禁用（auto_disabled）仅由三个 HTTP 状态码触发：**401 / 403 / 402**，**禁… |
| proxy/auto-disable-401-403-402.md#禁用 | proxy | 禁用 | auto-disable,401,403,402,stateless,throttle | auto | - | active | ❌ 429 触发 auto_disabled → 永久禁用平台，虽然是临时故障   ❌ 其他 4xx（如 400）触发 … |
| proxy/auto-disable-401-403-402.md#触发条件 | proxy | 触发条件 | auto-disable,401,403,402,stateless,throttle | auto | - | active | 见 `crates/aidog_core/src/gateway/proxy/non_success.rs:68`：  … |
| proxy/wire-protocol-whitelist-sync.md#MUST 硬约束 | proxy | MUST 硬约束 | - | always | - | active | 新增 wire protocol 时必须同步更新以下白名单，否则新协议会导致 route fail： - forward… |
| proxy/wire-protocol-whitelist-sync.md#关联 | proxy | 关联 | - | always | - | active / →rule-52,rule-53 | [[rule-52]] [[rule-53]] |
| proxy/wire-protocol-whitelist-sync.md#反例 | proxy | 反例 | - | always | - | active | - 新增 protocol X 但未加入白名单 → matched_ep=None 时 fallback 到 platf… |
| proxy/wire-protocol-whitelist-sync.md#触发场景 | proxy | 触发场景 | - | always | - | active | - converter-reasoning-content task：bug1 根因分析发现 matched_ep=No… |
| proxy/wire-protocol-whitelist-sync.md#适用 | proxy | 适用 | - | always | - | active | - 所有新增 wire protocol（endpoint 协议层）的变更 - 非 platform_type（平台别名… |
