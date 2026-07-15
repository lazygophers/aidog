# proxy-log-db-split design 细节核查 (只读)

调研对象: 拆 SQLite 为「主库 + proxy_log.db」的 design 前置核查 (4 点)。
主仓只读。引用均为 file:line。

## 1. 跨库事务 (原子性破坏面)

**结论: 无跨库事务原子性破坏** — proxy_log 与元数据表无共享事务, 拆库零原子性损失。

证据:
- aidog_core/src/gateway/db/ 全模块仅 2 处显式事务 (`conn.transaction()`):
  - platform_lifecycle.rs:36 — `delete_platform` 单事务包 `UPDATE platform` + `DELETE FROM group_platform` (纯元数据, 无 proxy_log)
  - platform_lifecycle.rs:238 — `set_tray_platform` 单事务包两条 `UPDATE platform` (纯元数据)
- 无 `with_transaction` / `unchecked_transaction` / `BEGIN` SQL 语句。
- proxy_log 写入 (proxy/log.rs:53-329 → `upsert_log` → `insert_proxy_log_columns`/`update_proxy_log_columns`) 走单条 `call_traced` (autocommit), 无事务包裹。
- `upsert_stats_agg` (stats_agg.rs:165) 同样单条 UPSERT autocommit。
- migration 全程 `let _ = conn.execute(...)` 幂等 ALTER, 无事务。

**拆库唯一影响 = 跨表读, 不是跨库事务 (见 §4)**。

## 2. migration 分流清单

### proxy_log.db 归属 (拆库后随 proxy_log 一起迁的 DDL)
**proxy_log 表**:
- CREATE TABLE: schema_early.rs:76-103 (含列 id/group_name(后 rename)/model/actual_model/source_protocol/target_protocol/platform_id/*headers/*body/status_code/duration_ms/tokens/created_at/updated_at/deleted_at)
- ALTER ADD: schema_early.rs:156 (est_cost), 160 (is_stream), 172 (attempts), 173 (retry_count), 231 (blocked_by), 232 (blocked_reason)
- ALTER RENAME: schema_late.rs:120 (group_name→group_key)
- ALTER DROP COL: schema_late.rs:278 (is_final)
- ALTER ADD: schema_late.rs:434-435 (cli_proxy_provider_id INTEGER)

**proxy_log 索引**:
- CREATE: schema_early.rs:117 (idx_proxy_log_model), 120 (idx_proxy_log_actual_model), 263-266 (idx_proxy_log_stats)
- CREATE: schema_late.rs:225 (idx_proxy_log_platform_id), 230 (idx_proxy_log_group_key), 259 (idx_proxy_log_group_key_stats), 293 (idx_proxy_log_status_created), 298 (idx_proxy_log_platform_created), 303 (idx_proxy_log_group_created)
- DROP: schema_late.rs:285 (idx_proxy_log_group), 286 (idx_proxy_log_platform), 311 (idx_proxy_log_status), 312 (idx_proxy_log_platform_id 旧单列), 313 (idx_proxy_log_group_key 旧单列), 327 (idx_proxy_log_created)

**stats_agg_hourly 表** (research 已定同迁):
- CREATE: `STATS_AGG_HOURLY_SQL` 常量定义在 mod.rs:20-59 (CREATE TABLE @ mod.rs:34; idx_stats_agg_time @ mod.rs:54; idx_stats_agg_platform @ mod.rs:58), 由 schema_late.rs:274 `conn.execute_batch(STATS_AGG_HOURLY_SQL)?` 执行
- DROP: schema_late.rs:325 (idx_stats_agg_model), 326 (idx_stats_agg_group)
- 首建后紧跟 `backfill_stats_agg_if_empty(conn)` (schema_late.rs:275, 定义 stats_agg.rs:127) — **跨表读**: 读 proxy_log + 读 `"group"` 表 (load_auto_from_map → aggregate_proxy_logs, stats_agg.rs:34-88), 拆库后需 ATTACH 或改用双 handle 内存回溯

### 主库归属 (其余 CREATE TABLE, 摘要)
- platform: schema_early.rs:14 (+ ALTER platform: 144/145/146/147/149/150/154/158/163/164/165/240/241/242; schema_late.rs:14/209/210/211/212/334/342/346)
- "group": schema_early.rs:36 (+ ALTER: 152/170; schema_late.rs:218; 两次 rebuild: 32+58, 78+108)
- group_platform: schema_early.rs:52 (+ ALTER: schema_late.rs:239 level_priority)
- setting: schema_early.rs:64
- model_price: schema_early.rs:128 (+ ALTER: schema_late.rs:9/10/11)
- middleware_rule: schema_early.rs:209 (+ idx_mw_rule_lookup:226)
- notification: schema_early.rs:247 (+ DROP col read: 258; idx_notification_created: schema_late.rs:267)
- mcp_server: schema_early.rs:272
- cli_proxy_provider: schema_late.rs:387 (+ idx_cli_proxy_group:400)
- (legacy DROP): mitm_ca / mitm_whitelist — schema_late.rs:515/526/551 (历史已迁 setting)

**注**: proxy_log 表内无 `FOREIGN KEY` 子句 (schema_early.rs:76-103 全文无 FK 声明, platform_id / cli_proxy_provider_id 都是裸 INTEGER 列), 拆库后无跨库 FK 约束损失。PRAGMA foreign_keys=ON 不受影响 (各库内部 FK 照常)。

### migration 拆分实现要点 (供 subtask)
- `init_tables` (schema.rs:9) 现在跑 `run_migrations_early` + `run_migrations_late` 在同一 connection 上, 无版本号机制 (每次启动全跑, 靠 IF NOT EXISTS / `let _ =` 吞 dup 错幂等)。
- 拆库后: proxy_log.db 的 connection 需跑 **proxy_log + stats_agg_hourly 的全部 DDL** (CREATE/ALTER/INDEX/DROP); 主库 connection 跑其余。两套 migration 函数须按归属切分, 不能简单按文件分 (schema_early/late 都混了两类)。
- backfill_stats_agg_if_empty 是跨表 (proxy_log + "group"), 见 §4 处理。

## 3. 备份/VACUUM/单文件假设改造面

**单文件假设清单** (拆库后需推广到双 handle 或多文件):

| 站点 | 类型 | 改造 |
|---|---|---|
| maintenance.rs:79 `incremental_vacuum_conn` | 工具 fn, 传 conn | 无需改 (callers 各自传 conn) |
| maintenance.rs:95 `migrate_auto_vacuum(db)` | 启动迁移 (app_setup.rs:54) | 需对 proxy_log.db 也跑一次 (新库建表前设 INCREMENTAL) |
| maintenance.rs:157 `compact_database(db)` | 整库 VACUUM | **需双库各跑一次** (command 与 scheduled 两处都调) |
| maintenance.rs:182 `db_file_size(db)` | PRAGMA page_count*size | **需双库求和** (阈值判断用) |
| maintenance.rs:192 `db_size_bytes(conn)` | 工具 fn | 无需改 |
| commands_system/backup.rs:107 `db_compact` | Tauri command | 改 compact_database 内部双库 |
| app_setup.rs:265 `db_file_size` + 272 `compact_database` | 24h scheduled | 同上, 阈值用合计大小 |
| backup/scheduler.rs:35 `run_backup` → import_export/collect.rs:14 `collect` | 备份 | **数据级 collect, 不碰 proxy_log** (只收 platform/group/group_platform/setting/mcp/middleware/model_price) — 单 Db handle 仍 OK, 无需改 |
| `purge_all_soft_deleted` (maintenance.rs:34, 表清单含 proxy_log, line 13-21) | 定时软删清理 | 表清单要拆: proxy_log 进 proxy_log.db handle, 其余主 handle |
| proxy_log 三级 retention (`run_retention_cleanup` commands_proxy/proxy_log.rs:138) | 用户主动 + scheduled | 仅 proxy_log.db handle |
| `cleanup_stats_agg` (stats_agg.rs:343) | stats retention (commands_platform/stats.rs:54 + app_setup.rs:253) | 仅 proxy_log.db handle |

**auto_update 备份**: commands_system/auto_update 未走 DB 文件复制 (未 grep 到 .db 文件 copy), backup 走数据级 collect → 加密 → 写 .aidog-backup 文件 (scheduler.rs:64-75), 不依赖单 .db 路径。

**新库建表前 PRAGMA**: Db::new (mod.rs:285-317) 在 sqlite_master 为空时设 `PRAGMA auto_vacuum = INCREMENTAL` (mod.rs:304-306)。proxy_log.db 新建时该机制自动生效, 无需额外处理; 仅老库 (已存数据) 需 migrate_auto_vacuum 跑一次 (见上)。

## 4. Db handle 路由改造面

### 4.1 注入现状
- **主 Db**: app_setup.rs:35 `Db::new(data_dir/aidog.db)` → app_setup.rs:99 `app.manage(db)` → 各 command 经 `State<'_, Db>` 取 (全仓 **151 处** State<Db> 注入: system 23 / platform 50 / proxy 35 / ai_tools 27 / config 5 / tray 4)。
- **代理侧独立 Db**: commands_proxy/proxy.rs:32 `Db::new(db_path)` → Arc<Db> 塞进 `ProxyState.db` (proxy/mod.rs:134)。**代理不共用 Tauri State 的 Db**, 而是开第二把同路径 handle (双连接同 .db 文件, WAL 下安全)。
- 后台 spawn 路径: app_setup.rs / shared.rs / quota.rs / backup/scheduler.rs 用 `app.try_state::<Db>()` 取主 Db (14 处)。

### 4.2 双 handle 候选方案 (改造面估算)

**方案 A: 两 Db 实例都 inject Tauri State (主 Db + ProxyLogDb newtype)**
- 主 Db (元数据): 现有 151 处 State<Db> 大多不动, 仅 ~15 个触及 proxy_log/stats_agg 的 command 改加 `State<ProxyLogDb>` 或换类型。
- 改造面: 
  - 新增 `ProxyLogDb(pub Db)` newtype + app.manage 两份。
  - **触及 proxy_log/stats_agg 的 command** (约 15 个): commands_proxy/proxy_log.rs 13 个 + commands_platform/stats.rs 2 个 (query_stats/query_stats_batch) + commands_platform/platform.rs:276 tray_today_stats + commands_tray/popover.rs 3 处。
  - **db 函数签名改造**: 取 `&Db` 改取 `&ProxyLogDb` 或拆双参 — proxy_log.rs (14 call sites) + stats_agg.rs (4) + query_stats.rs (2) + usage_stats.rs (7) + stats_today.rs (2) + maintenance 局部 (10) ≈ 39 个 call_traced/call_read_traced 站点要选 handle。
- 缺点: 跨表读 (§4.3) 仍需 ATTACH 或预取, 不能靠换 handle 解决。

**方案 B: 主 Db 内嵌 proxy_log handle (ProxyLogDb 作为 Db 的字段)**
- `Db` 结构体新增第 5 元组项 `Arc<Db>` 指向 proxy_log handle (或 lazy init)。
- 所有 db 函数签名不变 (仍 `&Db`), 内部按表名路由 (`call_traced` → 主连接 / `call_proxy_log_traced` → proxy_log 连接)。
- 改造面: 只动 Db 内部 + 各 db 函数把 `db.call_traced` 换成 `db.call_proxy_log_traced` (proxy_log/stats_agg/query_stats/usage_stats/stats_today 文件, 同方案 A 的 ~39 站点), command 层零改动。
- 代理侧 ProxyState.db 仍单 `Arc<Db>`, 内嵌即可, proxy.rs:32 不动。
- 缺点: Db 含两个连接路径, 测试夹具要同步 mock 两 handle; 内存库下两 handle 须共享同一物理库 (`:memory:` 退化为单库, 测试无感)。

**方案 C: ATTACH 模式 (proxy_log.db ATTACH 到主连接)**
- 主连接 `ATTACH 'proxy_log.db' AS pl;` → `pl.proxy_log` / `pl.stats_agg_hourly` 跨库 JOIN 透明可读, 跨表读无需改代码 (load_auto_from_map / aggregate_proxy_logs / list_request_logs LEFT JOIN 全继续工作)。
- 改造面最小: 仅 init 时 ATTACH + DDL 走 `pl.` schema 前缀; 业务代码几乎零改。
- 缺点: ① ATTACH 跨库事务受限 (本任务 §1 已证无跨库事务, 影响小); ② WAL + ATTACH 配合有限制 (proxy_log.db 也需 WAL, 各连接 ATTACH 同一文件); ③ 写仍走单写连接 (主), proxy_log 写并未独立分池, 「拆库隔离写压力」目标打折; ④ VACUUM/backup 仍按 ATTACH 主库做 (单连接内 VACUUM 不会跨 ATTACH 库)。**与 research 选型「独立 handle 隔离写压力」相悖, 不推荐**。

**推荐: 方案 B (主 Db 内嵌 proxy_log handle)** — 改造面集中、command 层零动、与代理侧 Arc<Db> 兼容; 跨表读经 ATTACH-on-demand 或 handle 内存预取 (见 §4.3)。

### 4.3 跨表读依赖 (无论哪个方案都要处理)

**写路径跨表读** (proxy_log.db handle 需读主库):
- `upsert_stats_agg` (stats_agg.rs:165-221) — platform_id=0 时 `load_auto_from_map(conn)` 读 `"group"` 表回溯 eff_pid (proxy_log.db 无 group 表)
- migration `backfill_stats_agg_if_empty` (stats_agg.rs:127) → `aggregate_proxy_logs` (stats_agg.rs:34) → `load_auto_from_map` 读 "group"

**读路径跨表读** (proxy_log/stats_agg 查询需 JOIN 主库):
- `list_request_logs` (proxy_log.rs:430) — `LEFT JOIN cli_proxy_provider cpp ON cpp.id = p.cli_proxy_provider_id` (主库表)
- `today_platform_stats` (stats_today.rs:122) — 读 `platform` 表 (id,name) 拼 platform_name
- `query_stats` (query_stats.rs:308-309) — `load_auto_from_map` (读 group) + `platform_id_name_map` (读 platform)
- `usage_stats.rs:341` — `load_auto_from_map`
- `aggregate_proxy_logs` (stats_agg.rs:34) — 读 "group" 表

**处理选项** (不拍板, 交 main grill):
1. **ATTACH on read**: proxy_log.db handle 连接 ATTACH 主库为只读 schema, JOIN/子查询透明。需 WAL + ATTACH 兼容验证。
2. **内存预取**: 进程启动 / 写前 preload `"group"` + `platform` + `cli_proxy_provider` 三张小表进内存 (各几十~百行), 把 `load_auto_from_map` / `platform_id_name_map` 改读内存; `LEFT JOIN cli_proxy_provider` 改两步查 (先查 proxy_log, 再内存补 name)。代价: 内存三份映射, 写后失效 (group/platform 写频次低, 可接受)。
3. **保留 schema 双写**: 元数据表在 proxy_log.db 维护一份只读副本 (YAGNI 反对, 写放大大)。

**推荐选项 2 (内存预取)** — 三个表都是小表 + 写低频 + 已有缓存先例 (DbCache 内 settings/groups/group_details 三 RwLock, mod.rs:120-137)。可复用 DbCache 模式给 proxy_log.db handle 加 `Arc<RwLock<HashMap<i64,String>>>` platform_names + `Arc<RwLock<HashMap<String,i64>>>` auto_from_map。

## 总结: subtask 拆分依据

1. **migration 分流**: 切 schema_early/schema_late + STATS_AGG_HOURLY_SQL 按表归属拆两套; 跑在各自 handle connection (§2)。
2. **Db handle 结构改造 (方案 B)**: Db 加 proxy_log handle 字段 + `call_proxy_log_traced` 方法; init 双库 (§4.2)。
3. **db 函数路由**: ~39 个 call_traced/call_read_traced 站点改走 proxy_log handle (proxy_log/stats_agg/query_stats/usage_stats/stats_today/maintenance 子集) (§4.2)。
4. **跨表读改内存预取**: 三小表 (group/platform/cli_proxy_provider) preload + 改 6 处读点 (§4.3)。
5. **VACUUM/备份/retention 推广双库**: compact_database/db_file_size/migrate_auto_vacuum/purge_all_soft_deleted 双库 (§3)。backup 不动 (数据级 collect 不碰 proxy_log)。
6. **代理侧**: ProxyState.db 内嵌后自动透传, proxy.rs:32 Db::new 改为开双库 (§4.1)。

无跨库事务原子性破坏 (§1)。
