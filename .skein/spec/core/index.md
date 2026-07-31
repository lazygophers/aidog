# SKEIN core 规则索引 (章节粒度: 一行一条规则)

类目: arch(5), cross-layer(5), db(7), domain(5), frontend(3), i18n(4), proxy(4) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | inclusion | anchors | status/出链 | summary |
|---|---|---|---|---|---|---|---|
| arch/mock-platform-short-circuit.md#Mock 平台绕开转发流水线短路 | arch | Mock 平台绕开转发流水线短路 | mock,platform,short-circuit,proxy | auto | - | active | Mock 平台在转发流水线早期短路，不走真实上游请求逻辑。  - `handler.rs:412` `matches!(… |
| arch/stream-buf-unified-cap.md#关联 | arch | 关联 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active / →hot-path-buffers,stream-buf-no-batching | [[stream-buf-no-batching]] [[hot-path-buffers]] |
| arch/stream-buf-unified-cap.md#案例 | arch | 案例 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active | **正例** —— usage 侧定义 `SSE_LINE_BUF_MAX_BYTES = 1MB`，内容侧转换分支引用… |
| arch/stream-buf-unified-cap.md#硬约则 | arch | 硬约则 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active | 同一隐患在代码库内多条执行路径出现时，上界值**禁止多处定义**。一处常量定义，其余路径引用 → 防止行为割裂。  ##… |
| arch/stream-buf-unified-cap.md#适用 | arch | 适用 | buffer,cap,single-source-of-truth,stream,stateful,SSE | auto | - | active | - 任何多路径并发处理同一数据流的缓冲 - 多个解析器共用一个上界（如 SSE / WebSocket 等流协议） - … |
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
| proxy/auto-disable-401-403-402.md#关联 | proxy | 关联 | auto-disable,401,403,402,stateless,throttle | auto | - | active / →http-client-no-env-proxy,mock-platform-short-circuit | [[mock-platform-short-circuit]] [[http-client-no-env-proxy]] |
| proxy/auto-disable-401-403-402.md#硬约则 | proxy | 硬约则 | auto-disable,401,403,402,stateless,throttle | auto | - | active | 平台自动禁用（auto_disabled）仅由三个 HTTP 状态码触发：**401 / 403 / 402**，**禁… |
| proxy/auto-disable-401-403-402.md#禁用 | proxy | 禁用 | auto-disable,401,403,402,stateless,throttle | auto | - | active | ❌ 429 触发 auto_disabled → 永久禁用平台，虽然是临时故障   ❌ 其他 4xx（如 400）触发 … |
| proxy/auto-disable-401-403-402.md#触发条件 | proxy | 触发条件 | auto-disable,401,403,402,stateless,throttle | auto | - | active | 见 `crates/aidog_core/src/gateway/proxy/non_success.rs:68`：  … |
