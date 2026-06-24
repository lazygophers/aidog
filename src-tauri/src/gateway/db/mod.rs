use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use tokio_rusqlite::Connection as AsyncConnection;

use crate::gateway::models::*;

/// 只读连接池大小（单点可调）。WAL 模式下「单写 + 多读并发」红利由这 N 条只读连接吃下，
/// 让 UI 读查询不再排在代理密集写日志之后。动态扩容（空闲回收 / 加锁扩容）本轮不做。
const READ_POOL_SIZE: usize = 8;

/// Migration 032（旧 011 文件）: 小时级聚合统计表 stats_agg_hourly（建表 + 索引 + 存量一次性回填）。
/// 统计读取改查预聚合表，写入解耦于日志开关（关日志也写聚合）。回填幂等（NOT EXISTS 空表守卫）。
/// 内联自原 migrations/011_stats_agg_hourly.sql（逐字保留）。init_tables 与回填测试共用。
const STATS_AGG_HOURLY_SQL: &str = r#"-- Migration 011 (file) / 032 (inline): 小时级聚合统计表 stats_agg_hourly。
--
-- 目的：统计读取（today_stats / today_platform_stats / group usage / Stats hourly+daily）
-- 从逐请求扫 proxy_log 改为查预聚合表，且【不受 ProxyLogSettings.enabled 日志开关影响】
-- （关日志也写聚合）。聚合粒度 = 本地时区小时桶 × model × group_key × eff_pid(回溯后平台)。
--
-- 列语义：
--  - time_hour: 本地时区小时桶 "YYYY-MM-DD HH:00:00"（与 bucket_time_expr 'localtime' 对齐）。
--  - model: actual_model 非空优先，否则 model（与 Stats actual_model 优先一致）。
--  - group_key: proxy_log.group_key（gk_<hex>，非显示名）。
--  - platform_id: 存 eff_pid（platform_id=0 经 group.auto_from_platform 回溯后的源平台 id）。
--  - sum_duration_ms 用 SUM 非 AVG，便于跨桶再聚合；avg 在查询时 = sum/request_count。
--  - success_count = 2xx；error_count = 终态非 2xx（status_code 不在 200..300）。
-- UNIQUE(time_hour,model,group_key,platform_id) 是 upsert 的 ON CONFLICT 目标键。
CREATE TABLE IF NOT EXISTS stats_agg_hourly (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    time_hour         TEXT NOT NULL,
    model             TEXT NOT NULL DEFAULT '',
    group_key         TEXT NOT NULL DEFAULT '',
    platform_id       INTEGER NOT NULL DEFAULT 0,
    request_count     INTEGER NOT NULL DEFAULT 0,
    success_count     INTEGER NOT NULL DEFAULT 0,
    error_count       INTEGER NOT NULL DEFAULT 0,
    sum_input_tokens  INTEGER NOT NULL DEFAULT 0,
    sum_output_tokens INTEGER NOT NULL DEFAULT 0,
    sum_cache_tokens  INTEGER NOT NULL DEFAULT 0,
    sum_est_cost      REAL NOT NULL DEFAULT 0,
    sum_duration_ms   INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL DEFAULT 0,
    updated_at        INTEGER NOT NULL DEFAULT 0,
    deleted_at        INTEGER NOT NULL DEFAULT 0,
    UNIQUE(time_hour, model, group_key, platform_id)
);

CREATE INDEX IF NOT EXISTS idx_stats_agg_time     ON stats_agg_hourly(time_hour);
-- idx_stats_agg_model / idx_stats_agg_group 已删（未被任何查询用：model/group_key 等值
-- 过滤总伴随 time_hour 范围谓词，规划器走 idx_stats_agg_time；纯单列索引仅增写放大）。
-- 旧库由 migration 035 DROP。详见 SQL/索引审计任务。
CREATE INDEX IF NOT EXISTS idx_stats_agg_platform ON stats_agg_hourly(platform_id);

-- 一次性回填：把存量 proxy_log 按 (本地小时桶, actual_model优先, group_key, eff_pid) 聚合写入。
-- 幂等：仅当 stats_agg_hourly 为空时回填（NOT EXISTS 守卫），避免重复执行翻倍。
-- eff_pid 回溯：platform_id=0 时经 group.auto_from_platform（十进制字符串）回溯到源平台。
-- 仅聚合 deleted_at=0 的有效日志。2xx → success，终态非 2xx → error。
INSERT INTO stats_agg_hourly
    (time_hour, model, group_key, platform_id,
     request_count, success_count, error_count,
     sum_input_tokens, sum_output_tokens, sum_cache_tokens,
     sum_est_cost, sum_duration_ms, created_at, updated_at, deleted_at)
SELECT
    strftime('%Y-%m-%d %H:00:00', created_at/1000, 'unixepoch', 'localtime') AS time_hour,
    CASE WHEN actual_model != '' THEN actual_model ELSE model END AS model,
    group_key,
    CASE WHEN platform_id = 0 THEN COALESCE(
        (SELECT CAST(g.auto_from_platform AS INTEGER)
         FROM "group" g
         WHERE g.group_key = proxy_log.group_key
           AND g.auto_from_platform != ''
           AND g.deleted_at = 0
         LIMIT 1), 0)
    ELSE platform_id END AS eff_pid,
    COUNT(*),
    SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END),
    SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END),
    COALESCE(SUM(input_tokens), 0),
    COALESCE(SUM(output_tokens), 0),
    COALESCE(SUM(cache_tokens), 0),
    COALESCE(SUM(est_cost), 0.0),
    COALESCE(SUM(duration_ms), 0),
    CAST(strftime('%s','now') AS INTEGER) * 1000,
    CAST(strftime('%s','now') AS INTEGER) * 1000,
    0
FROM proxy_log
WHERE deleted_at = 0
  AND NOT EXISTS (SELECT 1 FROM stats_agg_hourly LIMIT 1)
-- 位置引用 1..4 绑定到 SELECT 输出表达式（time_hour / model别名 / group_key / eff_pid）。
-- 不可写 `GROUP BY ..., model, ...`：SQLite 会把裸 `model` 优先绑定到 proxy_log 真实列，
-- 而 SELECT/UNIQUE 用的是 `CASE actual_model 非空优先` 别名；两个 raw model 映射到同一
-- actual_model 时聚合后输出同一复合键 → 撞 UNIQUE(time_hour,model,group_key,platform_id)。
GROUP BY 1, 2, 3, 4;
"#;

/// setting 缓存键的借用探测接口：让 `(&str, &str)` 与拥有所有权的 `(String, String)`
/// 共享同一套 `Hash`/`Eq` 语义，从而命中路径用借用键查 map，零 String 分配。
///
/// 标准 `HashMap<(String,String), _>::get` 要求 `Q: Borrow<(String,String)>`，
/// 而 `(String,String)` 并不 `Borrow<(&str,&str)>`，无法直接借用查找；stable Rust
/// 也没有 `raw_entry`。用 trait 对象作为 `Borrow` 目标是该场景的惯用解：owned key 与
/// borrowed key 都实现本 trait，`HashMap<(String,String)>` 借用为 `dyn KeyPair`，
/// `Hash`/`Eq` 委托到 `(scope, key)` 二元组，二者必然一致。
trait KeyPair {
    fn scope(&self) -> &str;
    fn key(&self) -> &str;
}

impl KeyPair for (String, String) {
    fn scope(&self) -> &str {
        &self.0
    }
    fn key(&self) -> &str {
        &self.1
    }
}

impl KeyPair for (&str, &str) {
    fn scope(&self) -> &str {
        self.0
    }
    fn key(&self) -> &str {
        self.1
    }
}

impl std::hash::Hash for dyn KeyPair + '_ {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // 必须与 `(String, String)` 的派生 Hash 字节序一致：依次 hash 两个 str。
        self.scope().hash(state);
        self.key().hash(state);
    }
}

impl PartialEq for dyn KeyPair + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.scope() == other.scope() && self.key() == other.key()
    }
}

impl Eq for dyn KeyPair + '_ {}

impl<'a> std::borrow::Borrow<dyn KeyPair + 'a> for (String, String) {
    fn borrow(&self) -> &(dyn KeyPair + 'a) {
        self
    }
}

/// 进程内热路径缓存（随 Db 实例生命周期，clone 共享同一份）。
///
/// 为什么挂在 `Db` 内而非全局 static：cargo test 单进程多线程跑，每个 test 各开一个
/// `:memory:` Db；全局缓存会跨 test 串味（test A 写 proxy/logging，test B 读到脏值）。
/// 内嵌 `Arc<RwLock<..>>` 保证「每个 Db 实例独立缓存 + clone 共享」两个性质同时成立。
#[derive(Default)]
struct DbCache {
    /// setting 表 (scope,key)→JSON 值缓存。`None` 槽位表示「已查过且不存在」，
    /// 用 `Option<Option<Value>>`：外层 = 是否缓存，内层 = 行是否存在。
    settings: RwLock<HashMap<(String, String), Option<serde_json::Value>>>,
    /// list_groups() 结果缓存（resolve_group 热路径用），写 group 表时整体失效。
    groups: RwLock<Option<Vec<Group>>>,
    /// list_group_details() 结果缓存（Groups 页一次拉全量用）。
    ///
    /// 内嵌完整 GroupDetail（含 platform 易变字段：est_balance_remaining / status /
    /// auto_disabled_until / last_real_query_at 等），故须**写时全失效**：任何 group /
    /// group_platform 结构写、platform create/update/delete、以及 estimate/breaker 对
    /// platform 易变列的写都失效（宁全勿漏，见 invalidate_group_details_cache 调用点）。
    ///
    /// 关键：list_group_details **不在代理 resolve 热路径**（proxy/router 走
    /// get_group_platforms 直查单组），故 estimate.rs 每请求级写带来的频繁失效只代价
    /// 「下次 Groups 页打开重建一次」，不影响代理吞吐。
    group_details: RwLock<Option<Vec<GroupDetail>>>,
}

/// 只读连接池句柄：一组只读 `AsyncConnection` + 轮询游标，`Clone` 廉价（仅 Arc 引用计数）。
///
/// 每条 `AsyncConnection` 自带独立后台线程，故 `CURRENT_DB_CTX` thread-local 在各读连接间
/// 天然隔离，不与写连接 / 其他读连接串味。WAL 模式下只读连接看到「最后已提交快照」，
/// UI 读允许微秒级陈旧（本就异步），换来不被代理密集写串行阻塞。
///
/// 🔴 `:memory:` fallback：`:memory:` 下每条物理连接是独立内存库，开新连接会读到空库 →
/// 测试与单库语义全破。故 `Db::new` 检测到内存库时，`conns` 只放 1 个写连接的 clone，
/// `pick()` 退化为返回写连接 sender，读写共享同一物理库，一致性保持。
#[derive(Clone)]
pub struct ReadPoolHandle {
    conns: Arc<Vec<AsyncConnection>>,
    cursor: Arc<AtomicUsize>,
}

impl ReadPoolHandle {
    /// 轮询取一条读连接（clone，仅 channel sender，廉价）。`Relaxed` 足够：仅需大致均匀
    /// 分发，无跨连接顺序依赖。`conns` 非空由 `Db::new` 保证（至少 1 条 fallback 写连接）。
    fn pick(&self) -> AsyncConnection {
        let idx = self.cursor.fetch_add(1, Ordering::Relaxed) % self.conns.len();
        self.conns[idx].clone()
    }
}

/// 异步 SQLite 连接封装。
///
/// tokio-rusqlite 内部以单后台线程顺序执行所有 `call` 闭包，天然串行化，
/// 故无需 `Mutex`。`AsyncConnection` 自身 `Clone + Send + Sync`（内部仅一个 channel sender），
/// 可直接 `app.manage(Db)` / `State<Db>`，克隆廉价（共享同一后台线程连接）。
///
/// - `self.0`：**写连接**（WAL 仅允许单写），承载全部写 / DDL / 事务 / cache 失效路径。
/// - `self.1`：`Arc<DbCache>` 进程内热缓存（不变，与连接数无关）。
/// - `self.2`：`ReadPoolHandle` 只读连接池，供 UI 热读路径（stats / 列表 / 日志查询）走
///   `call_read_traced` 并发查询，不阻塞于写连接队列。
#[derive(Clone)]
pub struct Db(pub AsyncConnection, Arc<DbCache>, ReadPoolHandle);

/// 有效 platform_id（eff_pid）派生 CASE 表达式——单一事实源。
///
/// 业务规则：直挂日志取原 `platform_id`；自动分组日志（`platform_id = 0`）经
/// `group.auto_from_platform`（十进制字符串）回溯到源平台 id，按 `group.group_key`
/// 匹配 `proxy_log.group_key`（gk_<hex>，非显示名）。回溯不到则归 0。
///
/// `col_prefix` 为外层 `platform_id` 列的限定前缀：
/// - `"proxy_log."`：query_stats.rs 内联进 SELECT/GROUP BY（dimension platform 分支 LEFT JOIN
///   platform 后裸列名歧义，须 proxy_log. 前缀）；
/// - `""`：usage_stats.rs recent-5 窗口子表（FROM proxy_log 单表，无歧义）。
///
/// 相关子查询对外层表的关联恒用表名 `proxy_log.group_key`（关联引用须用表名而非裸列），
/// 两处一致，故无需参数化。
pub(crate) fn eff_pid_case(col_prefix: &str) -> String {
    format!(
        "CASE WHEN {col_prefix}platform_id = 0 THEN COALESCE(\
(SELECT CAST(g.auto_from_platform AS INTEGER) FROM \"group\" g \
 WHERE g.group_key = proxy_log.group_key AND g.auto_from_platform != '' AND g.deleted_at = 0 LIMIT 1), 0) \
ELSE {col_prefix}platform_id END"
    )
}

/// 从 JSON 字符串反序列化 models
fn parse_models(json: &str) -> PlatformModels {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse platform models failed, using default (stored JSON corrupt?)");
        PlatformModels::default()
    })
}

/// 将 models 序列化为 JSON 字符串
fn serialize_models(models: &PlatformModels) -> String {
    serde_json::to_string(models).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize platform models failed, persisting empty object");
        "{}".to_string()
    })
}

/// 从 JSON 字符串反序列化可用模型列表
fn parse_available_models(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse available_models failed, using empty list (stored JSON corrupt?)");
        Vec::new()
    })
}

/// 将可用模型列表序列化为 JSON 字符串
fn serialize_available_models(models: &[String]) -> String {
    serde_json::to_string(models).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize available_models failed, persisting empty array");
        "[]".to_string()
    })
}

/// 从 JSON 字符串反序列化协议端点列表
fn parse_endpoints(json: &str) -> Vec<PlatformEndpoint> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "parse platform endpoints failed, using empty list (stored JSON corrupt?)");
        Vec::new()
    })
}

/// 将协议端点列表序列化为 JSON 字符串
fn serialize_endpoints(endpoints: &[PlatformEndpoint]) -> String {
    serde_json::to_string(endpoints).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "serialize platform endpoints failed, persisting empty array");
        "[]".to_string()
    })
}

impl Db {
    pub async fn new(path: &str) -> Result<Self, String> {
        let conn = AsyncConnection::open(path).await.map_err(|e| e.to_string())?;
        // pragma 是 connection 级状态，绑定后台线程那条物理连接，设一次永久生效。
        // WAL 下 synchronous=NORMAL 安全；单连接模型下 busy_timeout 实际罕触发，设置无害。
        conn.call(|c| {
            c.execute_batch(
                "PRAGMA journal_mode=WAL; \
                 PRAGMA foreign_keys=ON; \
                 PRAGMA busy_timeout=5000; \
                 PRAGMA synchronous=NORMAL;",
            )?;
            // 新库（无任何表）建表前设 auto_vacuum=INCREMENTAL，让后续 DELETE/free pages
            // 可被 incremental_vacuum 回收。auto_vacuum 只能建库前设；老库走 migrate_auto_vacuum。
            // 仅在 sqlite_master 空时设，避免对已有库误改（老库 =NONE 需 VACUUM 重建切换，见 migrate）。
            let table_count: i64 = c.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |r| r.get(0),
            )?;
            if table_count == 0 {
                c.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;")?;
            }
            // SQL 追踪：用 `profile`（sqlite3_profile）替代 legacy `trace`。profile 在 SQL
            // 执行**后**触发，回调签名 `fn(&str, Duration)`（裸函数，不能捕获，故走
            // sql_profile_callback），一次拿到内联了 `?` 实际值的 SQL 文本 + 执行耗时。
            // request_id / 调用位置经 call_traced 设的 thread-local 读出。超长字段值由
            // log_util::truncate_sql_literals 截断；仅影响日志输出，不碰 DB 写入。
            // 不再注册 trace 回调，避免同一 SQL 被 trace+profile 重复打印。
            c.profile(Some(sql_profile_callback));
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;

        let read_pool = Self::build_read_pool(path, &conn).await?;
        Ok(Self(conn, Arc::new(DbCache::default()), read_pool))
    }

    /// 构造只读连接池。
    ///
    /// 🔴 `:memory:` / in-memory 硬约束：每条物理连接是独立内存库，开新连接读到空库 → 测试
    /// 全崩。故内存库下池退化为复用写连接（放 1 个 `write_conn.clone()`），读写共享同一物理库。
    ///
    /// 非内存库：开 `READ_POOL_SIZE` 条 `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` 连接，
    /// 各设与写连接一致的 pragma（`journal_mode=WAL` 读 WAL 必需 / `busy_timeout` / `foreign_keys`）
    /// + 注册同一 `sql_profile_callback`（否则该连接 SQL 不进日志）。任一连接构造失败即整体失败。
    async fn build_read_pool(
        path: &str,
        write_conn: &AsyncConnection,
    ) -> Result<ReadPoolHandle, String> {
        // 内存库判定：":memory:" / 含 "mode=memory"（URI 形式）/ 空路径（rusqlite 视为匿名临时库，
        // 多连接亦不共享）。任一命中即 fallback 复用写连接。
        let is_memory =
            path == ":memory:" || path.contains("mode=memory") || path.is_empty();
        if is_memory {
            return Ok(ReadPoolHandle {
                conns: Arc::new(vec![write_conn.clone()]),
                cursor: Arc::new(AtomicUsize::new(0)),
            });
        }

        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let mut conns = Vec::with_capacity(READ_POOL_SIZE);
        for _ in 0..READ_POOL_SIZE {
            let c = AsyncConnection::open_with_flags(path, flags)
                .await
                .map_err(|e| e.to_string())?;
            // 只读连接 pragma：与写连接保持一致（auto_vacuum / synchronous 是写侧概念，只读连接
            // 无需设；WAL 必设以读到 WAL 已提交快照）。profile 回调让此连接 SQL 同样进 sql 日志。
            c.call(|c| {
                c.execute_batch(
                    "PRAGMA journal_mode=WAL; \
                     PRAGMA foreign_keys=ON; \
                     PRAGMA busy_timeout=5000;",
                )?;
                c.profile(Some(sql_profile_callback));
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?;
            conns.push(c);
        }
        Ok(ReadPoolHandle {
            conns: Arc::new(conns),
            cursor: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// DB 调用 chokepoint：与 `tokio_rusqlite::Connection::call` 同形（同闭包签名 / 返回
    /// 类型）。
    ///
    /// 链路 id（日志 `req=`）取值优先级：
    /// 1. 显式 `req`（代理请求路径传 request_id = proxy_log.id）；
    /// 2. 环境捕获：当前活跃 span（command span 的 trace_id / 后台轮询 span / init span）
    ///    的链路 id（`crate::logging::current_trace_id()`），免逐站点传参；
    /// 3. 兜底：`crate::logging::new_trace_id()` 当场生成真实唯一 id。
    ///
    /// **永不为 "bg" / "-" 等固定常量。**
    ///
    /// `caller` 为 **业务调用位置**，由各 Db 公开方法 `#[track_caller]` 在入口捕获后显式
    /// 传入（指向 proxy.rs / lib.rs 等业务代码而非 db.rs 内部行）。req + caller 在闭包进入
    /// DB 线程时写入 `CURRENT_DB_CTX`，供 `sql_profile_callback` 读出拼进 SQL 日志，闭包
    /// 结束（含 panic）后清空。
    ///
    /// 串行执行保证（tokio-rusqlite 单后台线程）→ 同一时刻仅一个闭包持有该 thread-local，
    /// 不会串味；set/clear 各一次，开销可忽略，无锁竞争。
    pub fn call_traced<F, R>(
        &self,
        req: Option<&str>,
        caller: &'static std::panic::Location<'static>,
        f: F,
    ) -> impl std::future::Future<Output = tokio_rusqlite::Result<R>>
    where
        F: FnOnce(&mut Connection) -> tokio_rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        // 在调用方线程（投递 DB 前）解析链路 id：显式 req > 环境 span 捕获 > 兜底生成。
        // current_trace_id 必须在此（调用方 tokio worker 线程，span 处于活跃态）读取，
        // 不能在 DB 后台线程读（那里无 span 上下文）。
        let req = req
            .map(|s| s.to_string())
            .or_else(crate::logging::current_trace_id)
            .unwrap_or_else(crate::logging::new_trace_id);
        let req = Some(req);
        // clone AsyncConnection（仅 channel sender，廉价）并 move 进 async block，使返回
        // future 为 `'static`（不借 &self），形态等价于原 `db.0.call(..).await`。
        let conn = self.0.clone();
        async move {
            // 包装用户闭包：进入 DB 线程时 set 上下文，离开（含 panic）时 guard 清空。
            conn.call(move |conn| {
                CURRENT_DB_CTX.with(|c| {
                    *c.borrow_mut() = DbCallCtx {
                        req: req.clone(),
                        caller: Some(caller),
                    };
                });
                struct Clear;
                impl Drop for Clear {
                    fn drop(&mut self) {
                        CURRENT_DB_CTX.with(|c| *c.borrow_mut() = DbCallCtx::default());
                    }
                }
                let _clear = Clear;
                f(conn)
            })
            .await
        }
    }

    /// 只读路径 chokepoint：与 `call_traced` **完整同形 / 同语义**（同闭包签名 + 同 req 解析
    /// 链 + 同 CURRENT_DB_CTX set/Clear guard + profile 拼日志），唯一差异是连接来源 ——
    /// 取读池一条只读连接（`self.2.pick()`）而非写连接 `self.0.clone()`。
    ///
    /// 仅供**纯 SELECT 无写副作用**的 UI 热读路径使用（stats / 列表 / 日志查询）。写 / DDL /
    /// 事务 / cache 失效路径必须继续走 `call_traced`（写连接）—— WAL 仅允许单写，且只读连接
    /// 执行写会 `SQLITE_READONLY` 失败。
    ///
    /// thread-local 隔离仍成立：每条读连接自带独立后台线程，`CURRENT_DB_CTX` 不串味。
    pub fn call_read_traced<F, R>(
        &self,
        req: Option<&str>,
        caller: &'static std::panic::Location<'static>,
        f: F,
    ) -> impl std::future::Future<Output = tokio_rusqlite::Result<R>>
    where
        F: FnOnce(&mut Connection) -> tokio_rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        // 与 call_traced 一致：调用方线程（投递前）解析链路 id，span 在此活跃。
        let req = req
            .map(|s| s.to_string())
            .or_else(crate::logging::current_trace_id)
            .unwrap_or_else(crate::logging::new_trace_id);
        let req = Some(req);
        // 取读池连接（轮询 clone，仅 channel sender）。:memory: fallback 下即写连接 clone。
        let conn = self.2.pick();
        async move {
            conn.call(move |conn| {
                CURRENT_DB_CTX.with(|c| {
                    *c.borrow_mut() = DbCallCtx {
                        req: req.clone(),
                        caller: Some(caller),
                    };
                });
                struct Clear;
                impl Drop for Clear {
                    fn drop(&mut self) {
                        CURRENT_DB_CTX.with(|c| *c.borrow_mut() = DbCallCtx::default());
                    }
                }
                let _clear = Clear;
                f(conn)
            })
            .await
        }
    }

    /// 失效全部 setting 缓存槽（写入端粗粒度失效，settings 写入低频，无需按 key 精修）。
    fn invalidate_settings_cache(&self) {
        if let Ok(mut g) = self.1.settings.write() {
            g.clear();
        }
    }

    /// 失效 list_groups 缓存（任意 group 表写入后调用）。
    /// group 表写同时影响 GroupDetail（其内嵌 Group），故连带失效 group_details。
    fn invalidate_groups_cache(&self) {
        if let Ok(mut g) = self.1.groups.write() {
            *g = None;
        }
        self.invalidate_group_details_cache();
    }

    /// 失效 list_group_details 缓存（group_platform 结构写 / platform 写后调用）。
    /// 独立于 invalidate_groups_cache：group_platform / platform 写不动 group 表，
    /// 不必清 groups 缓存，但必须清 group_details（其内嵌 platform 关联与易变字段）。
    pub fn invalidate_group_details_cache(&self) {
        if let Ok(mut g) = self.1.group_details.write() {
            *g = None;
        }
    }

    /// 同时失效 setting + group 两类热路径缓存。
    /// 供绕过 set_setting/group 函数直接写表的路径（如 import_export 事务批量写入）调用，
    /// 防止 setting/group 表被旁路改写后缓存仍返回旧值。
    pub fn invalidate_hot_caches(&self) {
        self.invalidate_settings_cache();
        self.invalidate_groups_cache();
    }
}

/// 当前毫秒级 Unix 时间戳
pub(crate) fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// 计算保留期截止时间戳（毫秒）。`days == 0` 表示跳过清理，返回 None。
pub(crate) fn retention_cutoff(days: u32) -> Option<i64> {
    if days == 0 {
        return None;
    }
    Some((chrono::Utc::now() - chrono::Duration::days(days as i64)).timestamp_millis())
}

// ─── 领域子模块（按 concern 拆分，纯结构搬移，行为零变更）───
mod trace;
mod schema;
mod schema_early;
mod schema_late;
mod platform;
mod platform_lifecycle;
mod stats_today;
mod group;
mod group_platform;
mod settings;
mod middleware;
mod proxy_log;
mod stats_agg;
mod maintenance;
mod usage_stats;
mod query_stats;
mod model_price;
mod mcp;

// 对外 re-export：保持 `gateway::db::X` 调用路径不变（外部代码无需改）。
// pub use 按各项自身可见性导出（pub → pub，pub(crate) → pub(crate)），
// 故跨子模块 `use super::*` 也能拿到 pub(crate) 共享 helper。
pub(crate) use trace::*;
pub(crate) use schema::*;
pub(crate) use schema_early::*;
pub(crate) use schema_late::*;
pub use platform::*;
pub use platform_lifecycle::*;
pub use stats_today::*;
pub use group::*;
pub use group_platform::*;
pub use settings::*;
pub use middleware::*;
pub use proxy_log::*;
pub use stats_agg::*;
pub use maintenance::*;
pub use usage_stats::*;
pub use query_stats::*;
pub use model_price::*;
pub use mcp::*;

// 测试模块：test_<源文件名> 1:1 命名，每个源文件 X.rs 的测试只在 test_X.rs（同目录）。
// 因 db/ 为扁平目录，所有子模块声明须由父模块 mod.rs 持有（test_X.rs 是 db 的兄弟文件，
// 非 X 的子目录文件，无法挂在 X.rs 名下）。test_support 持共享夹具（test_db / sample_* 等）。
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod test_mod;
#[cfg(test)]
mod test_trace;
#[cfg(test)]
mod test_model_price;
#[cfg(test)]
mod test_query_stats;
#[cfg(test)]
mod test_stats_agg;
#[cfg(test)]
mod test_usage_stats;
#[cfg(test)]
mod test_stats_today;
#[cfg(test)]
mod test_group;
#[cfg(test)]
mod test_group_platform;
#[cfg(test)]
mod test_platform;
#[cfg(test)]
mod test_platform_lifecycle;
#[cfg(test)]
mod test_settings;
#[cfg(test)]
mod test_proxy_log;
#[cfg(test)]
mod test_middleware;
#[cfg(test)]
mod test_maintenance;
#[cfg(test)]
mod test_schema;
#[cfg(test)]
mod test_mcp;
#[cfg(test)]
mod test_rw_pool;

