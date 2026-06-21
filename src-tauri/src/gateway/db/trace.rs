//! SQL 追踪基础设施：单条 DB 操作的 thread-local 上下文 + profile 回调。
//!
//! tokio-rusqlite 把所有 `.call` 闭包顺序投递到单一后台线程串行执行。本模块持有
//! 该后台线程的「当前操作上下文」（request_id + 业务调用位置），由 `Db::call_traced`
//! 在进入闭包时设置、退出时清空，供 `sql_profile_callback` 读出拼进 SQL 日志。

use crate::gateway::log_util::truncate_sql_literals;

/// 单条 DB 操作（一个 `.call` 闭包）的「当前上下文」，由 chokepoint `call_traced`
/// 在 **DB 后台线程** 进入闭包时设置、退出时清空。
///
/// 为什么是 thread-local 而非 task-local / span 字段：tokio-rusqlite 把所有 `.call`
/// 闭包顺序投递到 **单一后台线程** 串行执行，与调用方的 tokio worker 线程 / tracing
/// span 上下文不在同一执行流，span 字段不会跨线程传播。串行执行保证同一时刻该 DB
/// 线程只跑一个闭包 → thread-local 不会串味。`profile` 回调（同样在 DB 线程触发）
/// 读取本上下文，把 request_id + 调用位置拼进 SQL 日志。
#[derive(Default, Clone)]
pub(crate) struct DbCallCtx {
    /// 发起该操作的真实唯一链路 id：代理请求路径 = request_id；命令 / 后台 / 启动路径 =
    /// 当前 span（command span 的 trace_id / 后台轮询 span / init span）的 id；环境无任何
    /// 带 id 的 span 时由 `call_traced` 当场 `new_trace_id()` 兜底生成。**永不为固定常量**。
    pub(crate) req: Option<String>,
    /// 发起该 `.call` 的 **业务调用位置**（file:line）。由各 Db 公开方法 `#[track_caller]`
    /// 在入口 `Location::caller()` 捕获后显式传给 `call_traced`，故指向 proxy.rs / lib.rs /
    /// router.rs 等业务代码，而非 db.rs 内部 helper 行。
    pub(crate) caller: Option<&'static std::panic::Location<'static>>,
}

thread_local! {
    /// 当前 DB 操作上下文（仅在 DB 后台线程有意义）。
    pub(crate) static CURRENT_DB_CTX: std::cell::RefCell<DbCallCtx> = const { std::cell::RefCell::new(DbCallCtx { req: None, caller: None }) };
}

/// 把 `&Location` 渲染成简短 `文件名:行` 形式（去掉冗长的绝对/相对目录前缀）。
pub(crate) fn fmt_caller(loc: &std::panic::Location<'_>) -> String {
    let file = loc.file();
    // 取最后一段路径（如 src/gateway/db.rs → db.rs），日志更紧凑。
    let short = file.rsplit(['/', '\\']).next().unwrap_or(file);
    format!("{short}:{}", loc.line())
}

/// rusqlite `Connection::profile` 回调（裸 `fn(&str, Duration)`，不可捕获状态）。
/// 在 SQL 执行**后**触发，一次拿到 SQL 文本 + 执行耗时。读取 DB 线程的
/// `CURRENT_DB_CTX`（由 `call_traced` 设置）拼出 request_id + 调用位置 + 耗时。
///
/// 目标格式：`exec sql [fn=<file:line> req=<id或-> dur=<x.xms>] sql=<截断后>`。
/// 取代旧的 `trace` 回调（执行前触发、拿不到耗时），避免同一 SQL 重复输出。
pub(crate) fn sql_profile_callback(sql: &str, dur: std::time::Duration) {
    let (req, caller) = CURRENT_DB_CTX.with(|c| {
        let c = c.borrow();
        (
            // call_traced 保证 req 永远是真实唯一 id（环境捕获或兜底生成），此处
            // unwrap_or 仅防御未经 call_traced 的极端旁路，同样不输出固定常量。
            c.req
                .clone()
                .unwrap_or_else(crate::logging::new_trace_id),
            c.caller.map(fmt_caller).unwrap_or_else(|| "-".to_string()),
        )
    });
    let dur_ms = dur.as_secs_f64() * 1000.0;
    tracing::debug!(
        target: "sql",
        fn = %caller,
        req = %req,
        dur = format!("{dur_ms:.1}ms"),
        sql = %truncate_sql_literals(sql),
        "exec sql"
    );
}
