# backend-scan — Rust 后端规则重建扫描（reconstruct, bootstrap 型）

扫描范围：`src-tauri/`（workspace root package `aidog` + `crates/aidog_core` + `crates/aidog_test_util`），336 个 `.rs` 文件。
方法：只读代码 + 交叉验证 `.skein/spec/.archive/1785510844/` 旧规则。所有结论带 `file:line`。

统计基线（本次实测）：
- `tauri_command!` 宏调用 194 处；裸 `#[tauri::command]` 20 处（`grep -rn` on `crates/ src/`）
- `Result<_, String>` 出现 396 处（`crates/aidog_core/src`）
- `#[test] / #[tokio::test]` 1588 个
- `Arc<Mutex<..>>` 12 处 / `RwLock<..>` 16 处（非测试）
- `tokio::spawn` 20 处，`spawn_traced` 仅 6 处
- 无 `thiserror` / `anyhow` 依赖；无 feature flags；无 crate 级 `#![deny]`

---

## A. 命名约定

### A1 [core / naming] 新增 Tauri command 必须用 `crate::tauri_command! { pub async fn ... -> Result<T, String> }`，禁裸 `#[tauri::command]`
- 证据：`crates/aidog_core/src/command_macro.rs:9-29`（宏自动补 `#[tauri::command]` + `#[tracing::instrument(skip_all, fields(trace_id))]` + entry debug + **Err 分支自动 `tracing::error!`**）；194:20 的使用比压倒性。
- 宏的硬性签名约束：只接受 `pub async fn`、参数列表、返回**必须**是 `Result<_, String>`（`command_macro.rs:13`）。同步函数 / 非 `Result<_,String>` 返回类型无法用宏 → 这正是 20 处裸 command 的来源。
- 裸 command 现存位置（判定为「宏签名不兼容」的合理例外，非违规）：`cli_env.rs:323,446,529`、`sync_settings.rs:7,397,409`、`proxy_cmd/proxy.rs:95,136,150`、`ai_tools_cmd/script_executor.rs:6`、`system_cmd/about.rs:15`、`platform_cmd/model_fetch.rs:35`、`system_cmd/fs_autocomplete.rs:24`、`gateway/codex.rs:134,141,168`。
- 建议：namespace=core，category=naming（值得进 core：新增 command 是高频动作，且写错就丢 tracing 覆盖）

### A2 [recall / naming] `gateway/` 是业务层，`*_cmd/` 是命令薄壳层；`*_cmd` 只做参数解包 + 转发 `gateway::` 函数
- 证据：`crates/aidog_core/src/settings.rs:1-3` 自述「薄壳：转 `gateway::db` 的 `*_setting` 函数」；`settings.rs:12-17` 典型形态（一行 `db::get_setting(...)`）。
- 五个命令族目录：`system_cmd/ platform_cmd/ proxy_cmd/ ai_tools_cmd/ cli_proxy_cmd/`，加单文件族 `cli_env.rs / settings.rs / defaults.rs / popover.rs / tray_render.rs`（`lib.rs:12-30`）。
- category=naming

### A3 [recall / naming] `gateway/proxy/` 子模块用 `pub(crate) use super::*` 集中导入，子文件一律 `use super::*;` 开头
- 证据：`gateway/proxy/mod.rs:1-27`（集中 re-export axum / futures / db / adapter / models / router）；`gateway/proxy/retry.rs:1` = `use super::*;`；`gateway/db/schema.rs:1` 同款。
- 注释明写理由：「拆分后子模块 super=proxy，靠此 re-export 等价」（`proxy/mod.rs:15-16`）。
- category=naming

### A4 [recall / naming] `Protocol` 的 wire 字符串唯一来源是 `Protocol::wire_str()`（走 serde 序列化），禁手写字符串字面量
- 证据：`gateway/models/protocol.rs:173-178`（`serde_json::to_value(self).as_str()`）；消费点 `gateway/router/candidates.rs:41`（`peak_hours_for(extra, &platform_type.wire_str())`）。
- category=naming

---

## B. 错误处理

### B1 [core / error] 跨 Tauri 边界的错误类型固定为 `String`；DB / IO 错误在层内 `.map_err(|e| e.to_string())` 转字符串，禁引入 `thiserror`/`anyhow`
- 证据：`command_macro.rs:13`（宏签名硬编码 `Result<$ret, String>`）；396 处 `Result<_, String>`；`gateway/db/mod.rs:53`（`AsyncConnection::open(path).await.map_err(|e| e.to_string())`）；`Cargo.toml` 无 `thiserror`/`anyhow`（grep 无命中）。
- 建议：namespace=core，category=error（是全库不可协商的边界形状，新增命令必踩）

### B2 [recall / error] 用户可见的代理错误走 `gateway::i18n::ErrorKey` + `Lang`，禁在 proxy 路径拼裸英文错误串
- 证据：`gateway/i18n.rs:4-30`（`Lang` 7 语言 + `from_locale`）；`gateway/i18n.rs:32+`（`ErrorKey::ReadBody / NoMatchingGroup / ParseJson ...`）；`gateway/proxy/mod.rs:24`（`pub(crate) use super::i18n::{self, ErrorKey, Lang}`）。
- 注意跨层不对称：后端 `Lang` 7 个变体（无 `es-ES`），前端 8 语言。`i18n.rs:5-14` 无 EsEs → **西语用户拿到 EnUs 兜底**。这是可验证的现状缺口，值得作为 recall 记一笔。
- category=error

### B3 [recall / error] 「路由无候选」的两类语义必须分流：`peak_disabled` 落审计日志，其余 NoCandidate 只 warn 不落库
- 证据：`gateway/router/candidates.rs:236-241`（全候选被高峰排除 → `Err("peak_disabled")`）与 `candidates.rs:294`；消费侧 `gateway/proxy/handler.rs:385`（`if e == "peak_disabled"` 分支）。
- 这是**用错误字符串做控制流**的既定 idiom —— 改动 `"peak_disabled"` 字面量必须两侧同改（`candidates.rs` 2 处 + `handler.rs` 1 处）。
- category=error

### B4 [recall / error] 迁移 / 兼容路径故意吞错（`let _ = conn.execute(ALTER ...)`）是幂等手段，非疏忽；但吞错必须配注释说明幂等理由
- 证据：`gateway/db/schema_late.rs:152-154`（三条 `let _ = conn.execute("ALTER TABLE model_price ADD COLUMN ...")`，注释说明「NULL = 未知/无限制」）；`schema.rs:97-111`（`migrate_main_notification_out` 用 `.unwrap_or_default()` 吞 SELECT 报错，注释「幂等：表已不存在 → SELECT 报错吞空 Vec」）；`db/mod.rs:16-21`（`let Ok(mut stmt) = ... else { return Ok(HashMap::new()) }`，注释说明缺表语义）。
- 反面案例已被记录在代码里：`schema.rs:126-133` —— 原 046 的 `DELETE FROM stats_agg_hourly` 被 `let _ =` 吞掉后，表迁库导致清理静默失效。**吞错 + 跨库迁移 = 静默失效温床**。
- category=error

### B5 [recall / error] Drop 路径 / fire-and-forget 落库必须 `tokio::runtime::Handle::try_current()` 守卫 + `Instrument(span)`
- 证据：`gateway/proxy/handler.rs:56-70`（`RequestLogGuard::drop` 内：try_current 守卫 → `gen_child_id(&parent)` → `handle.spawn(fut.instrument(span))`）；注释明写「Drop 路径可能在 runtime teardown 后触发，守卫避免 spawn 静默丢失」。
- category=error

---

## C. 测试

### C1 [core / test] 测试文件与源文件 1:1 同目录同名：`X.rs` 的测试只放 `test_X.rs`，并在父 `mod.rs` 用 `#[cfg(test)] mod test_X;` 声明；无 `tests/` 集成目录
- 证据：`gateway/db/mod.rs:1119-1121` 有明文约定注释「测试模块：test_<源文件名> 1:1 命名，每个源文件 X.rs 的测试只在 test_X.rs（同目录）」；声明块 `db/mod.rs:1126-1170`；`find -type d -name tests` 零命中（全库无 `tests/` 目录）。
- 建议：namespace=core，category=test（写测试是高频动作，放错位置即违规且不可自动纠正）

### C2 [core / test] 触 FS / env（HOME / CODEX_HOME / CLAUDE_CONFIG_DIR）的测试必须用 `db::test_support::HomeGuard`，禁裸 `std::env::set_var`
- 证据：`gateway/db/test_support.rs:7-9`（`pub static ENV_LOCK: Mutex<()>`，注释「HOME / CODEX_HOME 是进程全局，所有触 FS 的测试必须串行在**同一把**锁上，跨模块共享」）；`test_support.rs:26-46`（`HomeGuard::new` 持锁 + tempdir + set_var）；`test_support.rs:48-63`（Drop 恢复原值）。
- 建议：namespace=core，category=test（并行 cargo test 下不遵守 = 随机 flaky，且症状不指向根因）

### C3 [recall / test] 跨 crate 测试 harness 走 `aidog_test_util::mock_app_with_db()` / `mock_app_with_db_and_engine()`；`aidog_test_util` 依赖 `aidog_core`，故 `aidog_core` **禁**反向 dev-dep
- 证据：`crates/aidog_test_util/src/lib.rs:1-5`（自述 + 「本 crate 不依赖任何 commands_* crate（禁循环）」）；`lib.rs:14-25`（`Db::new(":memory:")` + `init_tables` + `mock_builder`）；`lib.rs:29-39`（额外 manage `MiddlewareEngine`）。
- 因此 `aidog_core` 内部测试只能用 `gateway::db::test_support`（该模块**去掉了 `#[cfg(test)]` gate、始终 pub**，理由见 `db/mod.rs:1122-1125`：`cfg(test)` 不跨 crate）。
- category=test

### C4 [recall / test] DB 测试一律 `:memory:`；内存库下读池 / log.db / platform.db handle 全部退化复用写连接（这是刻意 fallback，不可「优化」掉）
- 证据：`db/mod.rs:238-257`（`build_read_pool` 内存库 fallback，注释标 🔴「每条物理连接是独立内存库，开新连接读到空库 → 测试全崩」）；`db/mod.rs:96-107 / 120-131`（log.db / platform.db 内存 fallback，path=None）。
- `DbCache` 内嵌于 `Db` 实例而非全局 static，理由同样是测试隔离：`db/cache.rs:59-63`。
- category=test

### C5 [recall / test] 连接死亡等故障注入用 `#[cfg(test)] Db::kill_next_read_slot()`（闭包内 panic 杀后台线程），验证 `call_read_traced` 换槽重试
- 证据：`db/mod.rs:993-1001`；对应测试文件 `db/test_rw_pool.rs`。
- category=test

---

## D. 架构边界

### D1 [core / arch] `crates/aidog_core` 不得依赖任何上层 crate（root `aidog` package / `aidog_test_util`），依赖方向单向向下
- 证据：`crates/aidog_core/src/lib.rs:8`「铁律：core 不依赖任何 commands_* crate（禁循环）」；`aidog_test_util/src/lib.rs:4-5` 同款声明；`aidog_test_util` 的 `use aidog_core::gateway::db::Db`（`lib.rs:7`）证实方向。
- 建议：namespace=core，category=arch

### D2 [core / arch] `src/startup.rs` 的 `tauri::generate_handler![...]` 是前端 invoke 名的唯一真值源；invoke 名 = 函数名，与模块路径无关
- 证据：`src/startup.rs:41-61+`（全量注册，路径形如 `aidog_core::platform_cmd::platform::platform_create`）；`startup.rs` 共 291 行几乎全是注册表。
- 推论（搬迁命令时的可执行自检）：搬模块后对 `generate_handler!` 集合做零差集自比对即可证明 invoke 名未变。
- 建议：namespace=core，category=arch

### D3 [core / arch] 前端 TS 类型由 ts-rs 从 Rust struct 生成，落 `src/services/api/types/generated/`；改 model struct 后必须跑 `yarn gen:types`
- 证据：`crates/aidog_core/Cargo.toml:61`（`ts-rs = { workspace = true }`）；`gateway/models/settings.rs:5,37,56,80,145` + `gateway/models/proxy_log.rs:5,84,118`（`#[ts(export, export_to = "../../../../src/services/api/types/generated/")]`）；`package.json:16`（`"gen:types": "cd src-tauri && cargo test -p aidog_core export_bindings"`）。
- i64/u64 字段一律标 `#[ts(type = "number")]`（`proxy_log.rs:86,88,90,92,94,98,101,108`）—— 否则 ts-rs 生成 `bigint`，前端算术炸。
- 建议：namespace=core，category=arch（改 model 是高频动作，漏跑 = 前后端类型静默漂移）
- 注：这条与 archive 的 `recall/cross-layer/ts-rust-symmetry.md` 主题重叠，但那条是手工对齐语气；**当前代码已是自动生成**，需按生成链重写。

### D4 [recall / arch] 三库拆分拓扑：主库 `aidog.db`（settings / model_price / stats_agg_hourly）、`log.db`（proxy_log / notification）、`platform.db`（platform / group / group_platform / cli_proxy_provider），各自独立写连接 Mutex + 独立 N=8 只读池
- 证据：`db/mod.rs:112-149`（`Db` 8 元组字段注释逐项说明）；`db/mod.rs:11`（`const READ_POOL_SIZE: usize = 8`）；`db/mod.rs:89-107`（log.db 路径推导 = 主库同目录 `log.db`）；`db/mod.rs:114-131`（platform.db 同款）。
- 四个 chokepoint 方法：`call_traced`（主库写）/ `call_read_traced`（主库读）/ `call_proxy_log_traced`（log.db 写）/ `call_platform_traced`（platform.db 写）+ 对应读池版（`db/mod.rs:478 / 573 / 643 / 717`）。
- **跨库禁 SQL JOIN / 子查询** —— 已被强制到 Rust 内存层：`load_auto_from_map`（`db/mod.rs:10-31`）+ `resolve_eff_pid`（`db/mod.rs:38-49`）替代原 `eff_pid_case` SQL CASE。
- category=arch
- 与 archive `recall/arch/cross-db-subquery-handle-selection.md` 主题一致，需按当前三库拓扑复核后重写。

### D5 [recall / arch] `gateway/` 子模块职责边界：`router/` 选候选、`proxy/` 跑请求生命周期、`scheduling.rs` 管熔断内存态、`db/` 管持久化，四者互不改写对方状态
- 证据：`gateway/scheduling.rs:1-15` 明文职责划分注释（「熔断器：临时性…自动半开探测恢复（本模块）」/「auto_disabled：永久性（401/403），状态持久化在 DB」/「候选过滤取 [熔断 Open] ∪ [auto_disabled] 并集，二者状态独立，互不改写」）；`gateway/router/mod.rs:1-9` 三子模块划分注释。
- category=arch

### D6 [recall / arch] `defaults/*.json` 读取回退链：app data (`~/.aidog/*.json`) → **deep merge** bundled `include_str!` → 缺失/损坏纯 bundled；禁走 Tauri resources
- 证据：`crates/aidog_core/src/defaults.rs:1-11`（模块头注释完整描述链路 + 「不走 Tauri resources（项目现行约定）」）；`defaults.rs:22-58`（空串 / parse 失败 / 序列化失败三段 warn + fallback）；`defaults.rs:37-40`（`merge_with_bundled` 补 app data 缺的 protocol key）。
- bundled 解析必须走 `gateway::presets_cache`（单 `OnceLock`，`presets_cache.rs:12`），禁各模块自建 `OnceLock` 重复解析同一份 107KB JSON（`presets_cache.rs:2-3` 明写此前 N 份重复解析的教训）。
- category=arch

---

## E. 并发选型

### E1 [recall / concurrency] `Arc<std::sync::Mutex<AsyncConnection>>` 包写连接**不是**为串行化（tokio-rusqlite 单后台线程已串行），而是为 ConnectionClosed 后整体替换重开；锁仅纳秒级持有，不与 DB 闭包执行重叠
- 证据：`db/mod.rs:117-122`（明文说明）；`db/mod.rs:462-476`（ConnectionClosed 兜底段注释 + 「历史上偶发 ~1% 代理 400 route error: ConnectionClosed」）；重开实现 `db/mod.rs:1027-1046`（`reopen_write_conn`，pragma + profile 全套）。
- category=concurrency

### E2 [recall / concurrency] `RwLock` 用于「读多写少的进程内缓存」（DbCache / scheduling 健康表 / http client 池），`Mutex` 仅用于「需整体替换的句柄槽」
- 证据：`db/cache.rs:64-81`（`DbCache` 全字段 `RwLock`）；`gateway/scheduling.rs:18`（`use std::sync::RwLock`）；`gateway/http_client.rs:5`（`Arc<RwLock<ClientCache>>`）；对比 E1 的 Mutex 用法。
- 实测比例：`RwLock` 16 / `Arc<Mutex>` 12（后者多数是 DB 的 4 个句柄槽 + test 锁）。
- category=concurrency

### E3 [recall / concurrency] 长生命周期 `tokio::spawn` 应走 `logging::spawn_traced(name, fut)` 以承接 trace_id 链；当前仅 6/20 处遵守（实际缺口）
- 证据：`crates/aidog_core/src/logging.rs:320`（`pub fn spawn_traced`）+ `logging.rs:282,297,337,376`（`gen_trace_id / gen_child_id / trace_id_from_request_id / new_trace_id`）；实测 `spawn_traced(` 调用 6 处 vs `tokio::spawn` 20 处。
- Drop 路径因不能用 `spawn_traced`（需 try_current 守卫）而手工展开，见 B5。
- category=concurrency（标注为**现状缺口**，非既成约定）

---

## F. 代理 / 转发层

### F1 [core / proxy] 上游响应头透传必须过 `retry.rs::filter_upstream_resp_headers`，黑名单单一真值源；流式额外剔 `SSE_EXTRA_BLACKLIST`
- 证据：`gateway/proxy/retry.rs:5-18`（`RESP_HEADER_BLACKLIST`：content-encoding / content-length / transfer-encoding + RFC 7230 §6.1 hop-by-hop 七项）；`retry.rs:21`（`SSE_EXTRA_BLACKLIST = ["content-type","cache-control","connection"]`）；`retry.rs:32-53`（实现：黑名单剔除 + 非法 header 跳过不 panic + 多值逐个保留）。
- 建议：namespace=core，category=proxy（漏剔 content-length/content-encoding = 客户端解压失败，症状远离根因）

### F2 [core / proxy] SSE 缓冲上界只有一个常量 `SSE_LINE_BUF_MAX_BYTES`（1MB），三条路径全部引用，禁第二份定义
- 证据：定义 `gateway/proxy/stream.rs:16`；引用 `stream.rs:97`（pending 上界）、`stream.rs:153`（`SseLineReassembler`）、`stream.rs:272`（remainder）；回归测试 `test_stream.rs:531,664,671`。
- 建议：namespace=core，category=proxy
- **archive 复核**：`core/arch/stream-buf-unified-cap.md` 与 `core/arch/stream-buffer-cap-single-source.md` 讲的就是这条，且仍与代码相符（常量名、值、路径全对上）。但两文件**内容重复**（后者是空壳只有标题），前者正文还整段**自我重复两遍**。重建时合成一条即可。

### F3 [recall / proxy] 请求生命周期钉死在 `handle_proxy`（生成 request_id = proxy_log 主键 + 挂 span）→ `handle_proxy_inner`（CONNECT 早期分流 + `RequestLogGuard`）→ `handle_proxy_core`
- 证据：`gateway/proxy/handler.rs:4-18`（request_id = `uuid v4 simple` 32-hex，同时挂 6 字符 base36 trace_id）；`handler.rs:83-89`（CONNECT 分流，注释说明前置理由是**打破 `handle_proxy_core ↔ handle_connect` 互递归导致 tokio::spawn 无法证 Send**）；`handler.rs:92-101`（guard arm/disarm）；`handler.rs:103+`（core）。
- 中断兜底语义：客户端断连 → Drop → 补写 `status_code=499`，靠 `WHERE status_code=0` 谓词保证幂等（`handler.rs:20-25, 43-45`）。
- category=proxy

### F4 [recall / proxy] 协议转换统一入口 `gateway::adapter`，只经 `convert_request / convert_response / parse_sse / parse_upstream_sse / parse_incoming_request / passthrough_api_path / to_client_sse` 七个 re-export
- 证据：`gateway/adapter/mod.rs:1-11`（模块列表 + `pub use converter::{...}`）。
- category=proxy

### F5 [recall / proxy] 熔断三态 `Closed{fails} / Open{until_ms} / HalfOpen{probes}`，且**不计熔断**的四类：401/403（走 auto_disabled）、非 429 的客户端 4xx、probe 请求
- 证据：`gateway/scheduling.rs:8-15`（状态机 + 不计熔断清单）；`scheduling.rs:20-30`（`BreakerState`）；`scheduling.rs:37-46`（`Admission::{Allow, Reject, Probe}`）。
- category=proxy

### F6 [recall / proxy] 平台候选排除有三个**正交于 status 三态**的维度：`expires_at` 过期、`extra.disable_during_peak` 高峰禁用、熔断 Open；三者独立、互不改写 status
- 证据：`gateway/router/mod.rs:41-52`（`candidate_state` doc 明列）；`router/mod.rs:53-63`（实现：expires_at → is_peak_disabled → status）；`candidates.rs:335-360`（`FilteredCandidates` 四分桶：active / probe / breaker_rejected / peak_disabled_count）。
- category=proxy

---

## G. 构建 / 门禁

### G1 [recall / build] cargo workspace = root package `aidog`（过渡态，注释标 C10 才挪 `crates/aidog/`）+ `crates/*` 两个成员；依赖版本集中在 `[workspace.dependencies]`，子 crate 一律 `{ workspace = true }`，禁自定版本
- 证据：`src-tauri/Cargo.toml` 头部注释（「本文件既属 workspace root 又是 root package `aidog`（过渡，C10 才挪 crates/aidog/）」+「共享依赖版本集中声明（design.md 约束 1）：子 crate 用 `{ workspace = true }` 引，禁子 crate 自定版本」）；`[workspace] members = ["crates/*"]`、`resolver = "2"`、`edition = "2024"`。
- category=build

### G2 [recall / build] 无 feature flag、无 crate 级 `#![deny]`；clippy 门禁靠约定（CLAUDE.md「warning 必须清」）而非编译期强制
- 证据：`Cargo.toml` / `crates/*/Cargo.toml` 无 `[features]` 段（grep 零命中）；全库仅两处 allow 属性：`gateway/mitm/mod.rs:26` `#![allow(dead_code)]`、`gateway/mcp/test_domain.rs:5` `#![allow(clippy::await_holding_lock)]`；`.github/workflows/*.yml` 无任何 `cargo` 行（release.yml / deploy-docs.yml 不跑 lint/test）。
- **门禁实际是本地命令**：`cd src-tauri && cargo clippy` + `cargo test`（CLAUDE.md 快速开始段），CI 不兜底。
- category=build（这条对「Done 自检」有直接影响：CI 不跑 = 本地必须跑）

### G3 [recall / build] 类型生成走 `yarn gen:types` = `cargo test -p aidog_core export_bindings`（ts-rs 借测试跑导出），非独立 build script
- 证据：`package.json:16`。见 D3。
- category=build

---

## H. 与 archive 旧规则的对账

### H1 仍相符（可直接沿用 / 轻改重建）
| archive 路径 | 判定 |
|---|---|
| `core/arch/stream-buf-unified-cap.md` | **相符**。常量名 `SSE_LINE_BUF_MAX_BYTES` / 值 1MB / `gateway/proxy/stream.rs` 路径全部对上（`stream.rs:16,97,153,272`）。但正文整段重复两遍，需去重。 |
| `core/db/sqlite-read-cache-config.md` | **相符**。`READ_CACHE_DEFAULT_KB = 64` 见 `db/mod.rs:223`；env 旋钮 `AIDOG_SQLITE_READ_CACHE_KB` 见 `db/mod.rs:267`；「写连接不设此 env（YAGNI）」见 `db/mod.rs:266`。属 protected 类（含基线数值 + 二分方法论），保留合理。 |
| `recall/db/crash-safe-db-split-migration.md` | **相符**。四阶段模式在 `db/schema.rs:94-96` 有对应实现注释（「**不 DROP**，由 Phase 1 主库闭包独立 DROP，避免 notification 049 的 read+DROP→INSERT 顺序在 crash 时丢数据」）。 |
| `recall/arch/cross-db-subquery-handle-selection.md` | **主题相符**，但需按当前三库（main/log/platform）拓扑复核，见 D4。 |

### H2 已与代码不符 / 需重写
| archive 路径 | 不符之处 |
|---|---|
| `core/arch/stream-buffer-cap-single-source.md` | **空壳重复**：全文只有 frontmatter + 一行标题「## 流缓冲上界单一真值源」，无正文。与 `stream-buf-unified-cap.md` 同一条规则重复登记两次。重建时合并为一条。 |
| `rules/perf/stream-buf-no-batching.md` + `rules/perf/stream-buffer-no-batching-delay.md` | 同名重复的第二组（文件名近乎同义）。`rules/` 这个 namespace 在当前 `.skein/spec/` 结构里已不是标准 namespace（标准为 core/recall），重建时需归位。 |
| `core/domain/rule-67.md`（peak-multiplier-symmetry） | **行号已漂**。规则引 `estimate/db_ops.rs:214` 余额扣减 + `:233` 手动预算；当前实际为 `estimate/db_ops.rs:219`（`) * peak_mult;`）与 `:237`（`) * peak_mult`），倍率来源 `db_ops.rs:199-201`。规则**语义仍成立**（两处确实都乘了 `peak_mult`，测试 `test_db_ops.rs:291` 覆盖），仅需更新行号。 |
| `recall/cross-layer/ts-rust-symmetry.md` | **机制已变**：现为 ts-rs 自动生成（`Cargo.toml:61` + `#[ts(export)]` + `yarn gen:types`），不再是「手工对齐 TS 与 Rust 字段」。按 D3 重写。 |
| `recall/db/filter-semantics.md` | 引用 `ai_tools_cmd/model_test.rs:157` / `gateway/quota/http.rs:187` / `proxy_log.rs:564` 三处行号，**未逐条复核**（超出本轮采样）。重建前建议按文件名 grep 复核行号。 |
| `recall/*/trellis-00..21`、`rule-03..67`、`shadcn-infra-*`、`auto-fix-downgrade-*` 等编号命名 | **命名即失效信号**：`rule-49` / `trellis-11` 这类无语义 slug 无法被 `description` 召回（recall 靠语义匹配）。重建时凡保留者必须改语义 slug。 |

### H3 未覆盖（本轮未取证，留给后续）
- `recall/ops/*` 与 `recall/optimization/*` 共 ~20 条（xctrace / phys_footprint / WebKit JIT 等量测方法论）—— 属 protected 类，本轮不动。
- `recall/frontend/*` / `recall/shadcn/*` / `recall/i18n/*` —— 前端侧，不在本 agent 范围。

---

## I. 本轮发现的代码级不一致（不是规则，是可修的事实）

1. **command 入口 debug 日志重复打两遍**：`tauri_command!` 宏已自动发 `tracing::debug!(command = ..., "command invoked")`（`command_macro.rs:18`），但约 200 个命令体内又手写了同一行（`grep -c '"command invoked"'` = 202；例：`settings.rs:13,21,35,41`、`defaults.rs:22`）。每个命令 debug 级重复输出一次。属宏迁移遗留，清理即可（不影响正确性）。
2. **`Lang` 缺 `EsEs`**（`gateway/i18n.rs:5-14`）：前端 8 语言含 `es-ES`，后端代理错误消息只有 7 种，西语落 `EnUs` 兜底。
3. **`spawn_traced` 覆盖率 6/20**：见 E3。
