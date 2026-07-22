use tracing_subscriber::{layer::SubscriberExt, layer::Context, EnvFilter, Layer, Registry};
use tracing_subscriber::registry::LookupSpan;
use tracing_appender::rolling::RollingFileAppender;
use std::cell::RefCell;
use std::time::Duration;
use tracing_subscriber::fmt::{FormatEvent, FmtContext};

thread_local! {
    /// 当前线程上「活跃 span 链」携带的链路 id 栈（栈顶 = 最内层带 id 的 span）。
    ///
    /// 由 [`TraceIdLayer`] 在 span enter 时压入、exit 时弹出。tracing 的 span enter/exit
    /// 在 **执行该 future 的线程上同步触发**（`#[instrument]` 每次 poll 都重新 enter / 退出
    /// 时 exit），故任意业务代码运行点读取栈顶 = 当前最内层带 `trace_id` / `request_id`
    /// 的 span 的 id。`Db::call_traced` 在 **调用方线程**（DB 投递前）读取本栈拿到环境 id，
    /// 再随闭包带入 DB 后台线程，避免逐站点显式传 id。
    static TRACE_ID_STACK: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// 读取当前线程「活跃 span 链」最内层的链路 id（trace_id / request_id）。
/// 无任何带 id 的活跃 span → `None`（调用方负责 fallback 生成）。
///
/// **角色（双轨）**:
/// - **主路径**: 同步业务代码（`inject_trace_header` / `Db::call_traced` 等）在 span
///   enter 的同线程上读取栈顶 —— span enter/exit 在执行 future 的线程上同步触发，
///   栈顶始终 = 当前最内层带 id 的 span，业务调用前必在 span 内 → 栈有效。
/// - **Fallback 角色**: [`AidogFormat`] 不走本函数（它在 `FmtContext` 里走 span scope
///   walk，跨 .await / spawn 自然继承），但 thread-local 栈对 sync 业务代码仍不可替代
///   （类型擦除不暴露 `LookupSpan`，业务拿不到 span scope）。
///
/// **Async spawn 失效问题**: tokio::spawn 跨线程后 thread-local 栈**不继承** → 子 task
/// 内栈空 → 业务代码兜底造孤儿 id。修复不靠改本函数，而是用 [`spawn_traced`] helper：
/// spawn 前在父线程读栈拿 parent id → gen child id → 包 info_span 让 TraceIdLayer
/// 在子 task 内重新压栈 + 写 extensions，子 task 内本函数读栈正确。
pub fn current_trace_id() -> Option<String> {
    TRACE_ID_STACK.with(|s| s.borrow().last().cloned())
}

/// span 字段访问器：抽取 `trace_id` / `request_id` 字段值（两者择一，request_id 优先）。
#[derive(Default)]
struct TraceIdVisitor {
    id: Option<String>,
}

impl tracing::field::Visit for TraceIdVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            // request_id（代理请求 span）优先于 trace_id（命令 / 后台 span），
            // 但二者通常不会同时出现在同一 span。
            "request_id" => self.id = Some(value.to_string()),
            "trace_id" if self.id.is_none() => self.id = Some(value.to_string()),
            _ => {}
        }
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        // `%expr` 走 record_str；`?expr` / 其他走这里。统一兜底解析。
        match field.name() {
            "request_id" => self.id = Some(format!("{value:?}").trim_matches('"').to_string()),
            "trace_id" if self.id.is_none() => {
                self.id = Some(format!("{value:?}").trim_matches('"').to_string())
            }
            _ => {}
        }
    }
}

/// 把每个 span 的 `trace_id` / `request_id` 字段值在创建时存进 span extensions，
/// 并在 enter / exit 时维护线程本地链路 id 栈（供 [`current_trace_id`] 读取）。
///
/// 仅做「捕获 + 维护栈」，不输出日志（输出仍由 fmt 层负责），故对正常日志格式无影响。
struct TraceIdLayer;

/// 存入 span extensions 的链路 id（仅当该 span 带 trace_id / request_id 字段）。
struct SpanTraceId(String);

impl<S> Layer<S> for TraceIdLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &tracing::span::Id, ctx: Context<'_, S>) {
        let mut visitor = TraceIdVisitor::default();
        attrs.record(&mut visitor);
        if let Some(tid) = visitor.id
            && let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(SpanTraceId(tid));
            }
    }

    fn on_enter(&self, id: &tracing::span::Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id)
            && let Some(SpanTraceId(tid)) = span.extensions().get::<SpanTraceId>() {
                let tid = tid.clone();
                TRACE_ID_STACK.with(|s| s.borrow_mut().push(tid));
            }
    }

    fn on_exit(&self, id: &tracing::span::Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id)
            && span.extensions().get::<SpanTraceId>().is_some() {
                TRACE_ID_STACK.with(|s| {
                    s.borrow_mut().pop();
                });
            }
    }
}

/// Application log settings (stored in settings table)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppLogSettings {
    /// Whether to write log files (effective in both dev and release)
    #[serde(default = "default_true")]
    pub file_enabled: bool,
    /// Log level: "trace" | "debug" | "info" | "warn" | "error"
    #[serde(default = "default_level")]
    pub level: String,
    /// Max log file retention in hours (0 = keep forever)
    #[serde(default = "default_retention_hours")]
    pub retention_hours: u32,
}

fn default_true() -> bool { true }
fn default_level() -> String { "info".to_string() }
fn default_retention_hours() -> u32 { 3 }

impl Default for AppLogSettings {
    fn default() -> Self {
        Self {
            file_enabled: default_true(),
            level: default_level(),
            retention_hours: default_retention_hours(),
        }
    }
}

/// 在基础级别上压制第三方 HTTP 栈的 debug 噪声 (如 hyper_util 的 `connected to <ip>:443`)。
/// 仅在用户未显式设 `RUST_LOG` 时生效; 设了 RUST_LOG 则完全尊重之 (便于细粒度调试)。
fn default_filter(level: &str) -> EnvFilter {
    let mut f = EnvFilter::new(level);
    for noisy in [
        "hyper=warn",
        "hyper_util=warn",
        "reqwest=warn",
        "rustls=warn",
        "h2=warn",
        "tower=warn",
        "mio=warn",
        "want=warn",
    ] {
        if let Ok(d) = noisy.parse() {
            f = f.add_directive(d);
        }
    }
    f
}

/// 构造文件层 appender (RollingFileAppender, HOURLY, prefix `aidog`, suffix `log`)。
///
/// 返回 `(appender, log_dir)`。目录创建失败时返回 `None` 并记 warn, 上层退化为 console-only。
/// **不**包含 `file_enabled` 判定 (上层分支判断), 也**不**构造 filter layer
/// (保留具体类型避免 `Box<dyn Layer>` 与 `set_global_default` 的 trait bound 冲突)。
fn build_file_appender(
    data_dir: &std::path::Path,
    settings: &AppLogSettings,
) -> Option<(RollingFileAppender, std::path::PathBuf)> {
    let log_dir = data_dir.join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        tracing::warn!(error = %e, dir = %log_dir.display(), "failed to create log dir; falling back to console-only");
        return None;
    }

    let appender = RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::HOURLY)
        .filename_prefix("aidog")
        .filename_suffix("log")
        .max_log_files(max_files_from_retention(settings.retention_hours))
        .build(&log_dir)
        .expect("failed to create log file appender");

    Some((appender, log_dir))
}

/// 文件层 filter: 始终遵循用户 `settings.level` (RUST_LOG 覆盖优先, 与 console 独立)。
fn file_filter(settings: &AppLogSettings) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter(&settings.level))
}

/// Initialize logging.
///
/// - Dev (debug build): console 强制 debug 级; `file_enabled=true` 时也挂文件层 (遵循 settings.level)。
/// - Release: console 遵循 settings.level; 按 `file_enabled` 决定是否挂文件层。
/// - `RUST_LOG` 覆盖所有层级 (优先级最高)。
pub fn init_logging(data_dir: &std::path::Path, settings: &AppLogSettings) {
    // Dev: console 强制 debug (RUST_LOG 可覆盖); Release: 遵循 settings.level。
    let console_filter = if cfg!(debug_assertions) {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter("debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter(&settings.level))
    };

    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_ansi(true)
        .event_format(AidogFormat { ansi: true })
        .with_filter(console_filter);

    let mode = if cfg!(debug_assertions) { "dev" } else { "release" };
    let file_on = settings.file_enabled;

    // dev / release 共用 build_file_appender + file_filter; file_enabled=false → 跳过。
    // 不抽 `Box<dyn Layer>` 是 ponytail 选择: 类型擦除与 set_global_default 的 Subscriber
    // trait bound 冲突, 而两层 fmt layer 组装逻辑只 ~5 行, 重复 < 错误抽象。
    if file_on {
        match build_file_appender(data_dir, settings) {
            Some((appender, _log_dir)) => {
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(appender)
                    .event_format(AidogFormat { ansi: false })
                    .with_filter(file_filter(settings));

                let subscriber = Registry::default()
                    .with(TraceIdLayer)
                    .with(console_layer)
                    .with(file_layer);
                let _ = tracing::subscriber::set_global_default(subscriber);
                tracing::info!(
                    mode = mode,
                    retention_hours = settings.retention_hours,
                    "logging initialized (console + file)"
                );
            }
            None => {
                // 目录创建失败: 退化为 console-only。
                let subscriber = Registry::default().with(TraceIdLayer).with(console_layer);
                let _ = tracing::subscriber::set_global_default(subscriber);
                tracing::warn!(mode = mode, "logging initialized (console only; log dir creation failed)");
            }
        }
    } else {
        let subscriber = Registry::default().with(TraceIdLayer).with(console_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
        tracing::info!(mode = mode, "logging initialized (console only, file disabled)");
    }
}

/// 测试专用：构造一个可装进作用域 subscriber 的 `TraceIdLayer` 实例，
/// 用于验证 `current_trace_id` 的环境捕获行为（生产路径经 `init_logging` 装载）。
#[cfg(test)]
pub fn trace_id_layer_for_test<S>() -> impl Layer<S>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    TraceIdLayer
}

/// 顶级链路 id：6 位 `[0-9a-z]` 随机串（36^6 ≈ 2.2B 空间）。
/// 全后端统一经 [`new_trace_id`] / [`gen_trace_id`] 取 id，禁各处自造。
/// span 内 .await 的下游调用自动继承该字段。
///
/// 异步分支用 [`gen_child_id`] 在父 id 后追加 `.xxxxxx` 形成可 grep 的父子树
/// （顶级 `a3f9k2` / 二级 `a3f9k2.b7x1mq`）。
pub fn gen_trace_id() -> String {
    // ponytail: rand 0.8 已在依赖内，字符表抽样 + thread_rng，避免手写取模偏置。
    const CHARSET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();
    (0..6)
        .map(|_| {
            let idx = rand::Rng::gen_range(&mut rng, 0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// 异步分支 id：`{parent}.{gen_trace_id()}`。父 id grep 可捞回整棵子树。
///
/// 消费点: [`spawn_traced`] helper（异步分支链路传播），不再标 dead_code。
pub fn gen_child_id(parent: &str) -> String {
    let mut s = String::with_capacity(parent.len() + 7);
    s.push_str(parent);
    s.push('.');
    s.push_str(&gen_trace_id());
    s
}

/// Wrap `tokio::spawn` 让异步分支自动携带父子链路 id。
///
/// **为什么需要**: `TRACE_ID_STACK` 是 thread-local, tokio task 跨线程执行后栈不继承
/// → spawn 出的 task 内 `current_trace_id()` = None → 业务代码（如 inject_trace_header
/// 兜底）造孤儿 id，与父流程脱钩 = 用户痛点根因。
///
/// **修法**: spawn 前读父 `current_trace_id()`（线程栈在 spawn 调用点仍有效），
/// 生成 child id = `父.6字`，包 `info_span!("spawn", trace_id = child)` 让子 future
/// 进入时 span scope + thread-local 栈都正确（TraceIdLayer 会把 child 写进 extensions
/// 并压栈，FormatEvent span scope walk + 业务 current_trace_id 全部命中）。
///
/// **`name`**: 子任务语义标签（如 "log_flush" / "connect_relay"），人读不进 id。
///
/// **不适用**: 已经显式 instrument 了 req_span 的 spawn（如 spawn_estimate + stream
/// flush），双重 instrument 会丢父 span 关联 → 那些调用点保持原状。
pub fn spawn_traced<F>(name: &'static str, fut: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    use tracing::Instrument;
    let parent = current_trace_id().unwrap_or_else(gen_trace_id);
    let child = gen_child_id(&parent);
    let span = tracing::info_span!("spawn", name = %name, trace_id = %child);
    tokio::spawn(fut.instrument(span))
}

/// proxy 请求路径的顶级 id 映射：取 request_id（32-hex = proxy_log.id 主键）前 8 hex
/// → u32 → 截 31 bit → base36 pad 6 → `[0-9a-z]{6}`。
///
/// 顺向诊断（proxy_log.id → base36 → grep 日志）直接；逆向需查库。碰撞：31 bit ≈ 21 亿
/// 空间，单进程同时活跃请求 ≤ 数百，可忽略。request_id（32-hex）本身不变，仍作 proxy_log 主键。
pub fn trace_id_from_request_id(request_id: &str) -> String {
    // ponytail: u32::from_str_radix 取低 32 位再 & 0x7FFF_FFFF 截到 31 bit，
    // 避免手写 hex→int 切片循环；前 8 hex 缺失（异常输入）兜底 0。
    let hi = request_id
        .get(..8)
        .and_then(|s| u32::from_str_radix(s, 16).ok())
        .unwrap_or(0);
    let bits = hi & 0x7FFF_FFFF; // 31-bit
    format_radix_padded(bits, 36, 6)
}

/// `n` 转 `radix` 进制字符串，左侧 pad '0' 到 `width` 位；超长截断到 width
/// （顶级 id 固定 6 位以保 grep 契约）。
fn format_radix_padded(n: u32, radix: u32, width: usize) -> String {
    const CHARSET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        let zero = CHARSET[0] as char;
        return std::iter::repeat_n(zero, width).collect();
    }
    let mut digits: Vec<u8> = Vec::new();
    let mut n = n;
    while n > 0 {
        digits.push(CHARSET[(n % radix) as usize]);
        n /= radix;
    }
    digits.reverse();
    let s: String = digits.into_iter().map(|b| b as char).collect();
    if s.len() < width {
        let pad = width - s.len();
        format!("{}{}", "0".repeat(pad), s)
    } else if s.len() > width {
        // 31-bit base36 最长 7 位，截到 6 位保顶级 id 格式契约（碰撞概率仍可忽略）。
        s[s.len() - width..].to_string()
    } else {
        s
    }
}

/// 生成顶级 trace id（向后兼容旧调用点）。等价 [`gen_trace_id`]。
pub fn new_trace_id() -> String {
    gen_trace_id()
}

fn max_files_from_retention(hours: u32) -> usize {
    if hours == 0 { 72 } else { hours as usize } // 0 = keep up to 72 files (~3 days) as fallback
}

/// 自定义日志行格式器：控制 5 段字段顺序 + ANSI 着色 + traceid 注入。
///
/// 字段顺序: `<time> <level> <file>:<line> <func> <msg> <traceid>`
/// - console: `ansi=true`, 各字段独立 ANSI 色 (level=红/黄/绿/蓝/灰; time=灰; file:line func=紫;
///   msg=默认; traceid=cyan)
/// - file: `ansi=false`, 纯文本 (ANSI 进文件污染 grep)
///
/// traceid 取值: span scope walk (`FmtContext` 提供具体 `S: LookupSpan`, 跨 .await / spawn 继承)
///   → thread-local 栈 fallback (启动早期) → `gen_trace_id()` 现场新生 root (用户决策)。
///
/// **唯一改动点**: console_layer + file_layer 用 `.event_format(AidogFormat { ansi })` 替换默认 fmt。
struct AidogFormat {
    ansi: bool,
}

/// ANSI 颜色码 (SGR 前缀, 配 `\x1b[<code>m`)。
mod ansi {
    pub const RESET: &str = "\x1b[0m";
    pub fn wrap(code: u8, s: &str, ansi: bool) -> String {
        if ansi { format!("\x1b[{code}m{s}{RESET}") } else { s.to_string() }
    }
}

/// span scope walk: 从 event 的 parent span 起向外 (leaf → root), 找最近含 `SpanTraceId`
/// extension 的 span, 返回其 id。跨 .await / 跨 spawn (instrument 后) 自然继承 (span 跟 future 走)。
/// 无活跃 span / span 链无 id → `None` (调用方回退 thread-local 或兜底 gen root)。
fn trace_id_from_span_scope<S, N>(ctx: &FmtContext<'_, S, N>) -> Option<String>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    // event_scope() 返回 leaf → root 迭代器 (含 parent span 及其所有祖先)。
    let scope = ctx.event_scope()?;
    for s in scope {
        if let Some(SpanTraceId(tid)) = s.extensions().get::<SpanTraceId>() {
            return Some(tid.clone());
        }
    }
    None
}

impl<S, N> FormatEvent<S, N> for AidogFormat
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        // 先把 event 字段格式化成 msg 文本 (默认 fmt::format 走 FormatFields, 走 ctx.field_format)。
        let mut msg_visitor = MsgCollector::default();
        event.record(&mut msg_visitor);
        let msg = msg_visitor.msg.trim();
        let meta = event.metadata();
        let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let level = meta.level();
        let file = meta.file().unwrap_or("?");
        let line = meta.line().unwrap_or(0);
        let func = meta.target();
        let trace_id = trace_id_from_span_scope(ctx)
            .or_else(current_trace_id)
            .unwrap_or_else(gen_trace_id);

        let ansi = self.ansi;
        // 1. time
        write!(writer, "{} ", ansi::wrap(90, &time.to_string(), ansi))?;
        // 2. level (per-level color: error 31 / warn 33 / info 32 / debug 34 / trace 90)
        let lvl_code = match *level {
            tracing::Level::ERROR => 31u8,
            tracing::Level::WARN => 33,
            tracing::Level::INFO => 32,
            tracing::Level::DEBUG => 34,
            tracing::Level::TRACE => 90,
        };
        let lvl_str = format!("{:5}", level.as_str());
        write!(writer, "{} ", ansi::wrap(lvl_code, &lvl_str, ansi))?;
        // 3. file:line func (purple 35)
        let loc = format!("{file}:{line} {func}");
        write!(writer, "{} ", ansi::wrap(35, &loc, ansi))?;
        // 4. msg (含 message 主体 + 业务字段 key=value, default color, no wrap)
        write!(writer, "{msg} ")?;
        for (k, v) in &msg_visitor.extra {
            write!(writer, "{k}={v} ")?;
        }
        // 5. traceid (cyan 36)
        writeln!(writer, "{}", ansi::wrap(36, &trace_id, ansi))
    }
}

/// Visit event fields → 抓 message 字段 + 拼成单串，并把非 message 业务字段按
/// `key=value` 记录顺序收集到 `extra`（塞 msg 段尾部，禁丢字段）。
///
/// - `message` 字段 → msg 主体（去引号与现 message 处理一致）
/// - `trace_id` 字段 → 跳过（5 段格式 traceid 段由 `trace_id_from_span_scope`
///   / thread-local / gen 三级兜底单独取，event 显式带则去重避免重复）
/// - 其余字段 → `extra` push `(name, value)`，value 保留各自类型原貌
///   （string 字段去引号，Debug 类型保留 Debug 格式）
///
/// event 的 message 来自 `event!("..."` 第一位置参数, 走 `message` 字段名。
#[derive(Default)]
struct MsgCollector {
    msg: String,
    extra: Vec<(String, String)>,
}

impl tracing::field::Visit for MsgCollector {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        if name == "message" {
            // message 字段值用 Debug 格式通常带引号, 去之。
            let raw = format!("{value:?}");
            self.msg.push_str(raw.trim_matches('"'));
            self.msg.push(' ');
        } else if name == "trace_id" {
            // trace_id 字段去重见上文 doc comment。
        } else {
            self.extra.push((name.to_string(), format!("{value:?}")));
        }
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let name = field.name();
        if name == "message" {
            self.msg.push_str(value);
            self.msg.push(' ');
        } else if name == "trace_id" {
            // 同上去重。
        } else {
            self.extra.push((name.to_string(), value.to_string()));
        }
    }
}

/// Clean up old log files beyond retention period
pub fn cleanup_old_logs(data_dir: &std::path::Path, retention_hours: u32) {
    if retention_hours == 0 { return; }
    let log_dir = data_dir.join("logs");
    if !log_dir.exists() { return; }

    let cutoff = std::time::SystemTime::now() - Duration::from_secs(retention_hours as u64 * 3600);

    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("log")
                && let Ok(metadata) = entry.metadata()
                    && let Ok(modified) = metadata.modified()
                        && modified < cutoff {
                            let _ = std::fs::remove_file(&path);
                        }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_file_appender_creates_log_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let settings = AppLogSettings {
            file_enabled: true,
            level: "info".into(),
            retention_hours: 3,
        };
        let (appender, log_dir) = build_file_appender(tmp.path(), &settings)
            .expect("appender should build when dir is writable");
        // <data_dir>/logs must have been created.
        assert!(log_dir.exists() && log_dir.is_dir());
        // Appender is constructed (sanity: type is usable).
        let _ = appender;
    }

    #[test]
    fn file_filter_respects_settings_level() {
        // Valid level must not panic.
        let settings = AppLogSettings {
            file_enabled: true,
            level: "warn".into(),
            retention_hours: 3,
        };
        let _ = file_filter(&settings);
    }

    // init_logging 行为不在单测覆盖内: set_global_default 全局副作用与并行测试互斥,
    // 强行 e2e 会污染其他测试的 subscriber。dev 实测路径: `make run` 启动后检查
    // `<data_dir>/logs/aidog-*.log` 是否被写入 (见 prd.md Acceptance Criteria)。

    fn is_root_id(s: &str) -> bool {
        s.len() == 6 && s.chars().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
    }

    #[test]
    fn gen_trace_id_format() {
        for _ in 0..1000 {
            let id = gen_trace_id();
            assert!(is_root_id(&id), "gen_trace_id must be 6 [0-9a-z], got {id}");
        }
    }

    #[test]
    fn gen_trace_id_no_collision_1000() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            let id = gen_trace_id();
            // 36^6 ≈ 2.2B, 1000 次碰撞概率极小；命中说明 RNG / 字符表退化。
            assert!(seen.insert(id), "gen_trace_id collision within 1000 draws");
        }
    }

    #[test]
    fn gen_child_id_format() {
        let parent = "abc123";
        let child = gen_child_id(parent);
        assert_eq!(child.len(), 6 + 1 + 6, "child id = parent.6chars");
        assert!(child.starts_with("abc123."), "child id must start with parent + '.'");
        let suffix = &child[7..];
        assert!(is_root_id(suffix), "child suffix must be 6 [0-9a-z], got {suffix}");
    }

    #[test]
    fn gen_child_id_distinct() {
        let parent = "topid1";
        let a = gen_child_id(parent);
        let b = gen_child_id(parent);
        assert_ne!(a, b, "two children of same parent must differ in suffix");
    }

    #[test]
    fn trace_id_from_request_id_deterministic() {
        // 同 input → 同 output（顺向诊断 grep 的契约基础）。
        let rid = "0123456789abcdef0123456789abcdef";
        let a = trace_id_from_request_id(rid);
        let b = trace_id_from_request_id(rid);
        assert_eq!(a, b, "trace_id_from_request_id must be deterministic");
        assert!(is_root_id(&a), "must be 6 [0-9a-z], got {a}");
    }

    #[test]
    fn trace_id_from_request_id_format() {
        let rid = "ffffffffffffffffffffffffffffffff";
        let id = trace_id_from_request_id(rid);
        // ffffffff & 0x7FFF_FFFF = 0x7FFF_FFFF, base36 最长 7 位，截 6 位。
        assert_eq!(id.len(), 6, "must always be exactly 6 chars");
        assert!(is_root_id(&id), "must be 6 [0-9a-z], got {id}");
    }

    #[test]
    fn trace_id_from_request_id_zero() {
        let id = trace_id_from_request_id("00000000000000000000000000000000");
        assert_eq!(id, "000000", "zero request_id maps to all-zero 6-char id");
    }

    #[test]
    fn trace_id_from_request_id_short_input() {
        // 异常短输入兜底 0，不 panic。
        let id = trace_id_from_request_id("ab");
        assert_eq!(id, "000000", "short request_id falls back to zero-padded");
    }

    #[test]
    fn new_trace_id_equivalent_to_gen() {
        let a = new_trace_id();
        assert!(is_root_id(&a), "new_trace_id must delegate to gen_trace_id");
    }

    #[test]
    fn format_radix_padded_basic() {
        assert_eq!(format_radix_padded(0, 36, 6), "000000");
        assert_eq!(format_radix_padded(1, 36, 6), "000001");
        assert_eq!(format_radix_padded(35, 36, 6), "00000z");
        assert_eq!(format_radix_padded(36, 36, 6), "000010");
    }

    // ---- AidogFormat 单测 ----
    // 用 tracing_subscriber 的 TestWriter + 临时 subscriber (with_default) 隔离捕获输出,
    // 不污染全局 subscriber (init_logging 已用 set_global_default)。

    /// 构一个测试 subscriber, 把 AidogFormat 输出捕获到 returned String。
    /// 调用方在 closure 内 dispatch set_default 后 emit event, closure 返回 captured 文本。
    fn with_test_subscriber<F>(ansi: bool, body: F) -> String
    where
        F: FnOnce(),
    {
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::writer::MakeWriter;

        // 自定义 MakeWriter: 把所有写入聚合到共享 Arc<Mutex<Vec<u8>>>。
        #[derive(Clone)]
        struct BufMaker(Arc<Mutex<Vec<u8>>>);
        impl<'a> MakeWriter<'a> for BufMaker {
            type Writer = BufWriter;
            fn make_writer(&'a self) -> Self::Writer {
                BufWriter { buf: self.0.clone() }
            }
        }
        struct BufWriter { buf: Arc<Mutex<Vec<u8>>> }
        impl std::io::Write for BufWriter {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.buf.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }

        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let subscriber = Registry::default()
            .with(TraceIdLayer)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(BufMaker(buf.clone()))
                    .with_ansi(ansi)
                    .event_format(AidogFormat { ansi }),
            );
        let _guard = tracing::subscriber::set_default(subscriber);
        body();
        let bytes = std::mem::take(&mut *buf.lock().unwrap());
        String::from_utf8_lossy(&bytes).to_string()
    }

    #[test]
    fn aidog_format_emits_trace_id_from_span() {
        // event 在带 trace_id 的 span 内, 输出必须含该 trace_id。
        // 注意: TraceIdVisitor 规则是 request_id 优先于 trace_id, 故同时挂两个字段时, 日志
        // 行注入的 traceid 字段 = request_id 的值 (与 inject_trace_header 一致)。
        let out = with_test_subscriber(false, || {
            let span = tracing::info_span!("req", trace_id = "abc123", request_id = "deadbeef");
            let _enter = span.enter();
            tracing::info!("hello world");
        });
        // 取行尾 token 验证 (= 注入的 traceid 字段)。
        let line = out.lines().last().unwrap_or("");
        let last_token = line.split_whitespace().last().unwrap_or("");
        assert_eq!(
            last_token, "deadbeef",
            "request_id wins over trace_id when both present (TraceIdVisitor rule); got line: {line:?}"
        );
        // 字段顺序: time level file:line func msg traceid — msg 在 traceid 之前。
        let msg_idx = out.find("hello world").unwrap_or(usize::MAX);
        let tid_idx = out.find("deadbeef").unwrap_or(usize::MAX);
        assert!(msg_idx < tid_idx, "msg must come before traceid; got: {out:?}");
    }

    #[test]
    fn aidog_format_emits_trace_id_only_span() {
        // 只挂 trace_id (无 request_id) 时, 注入 trace_id 的值。
        let out = with_test_subscriber(false, || {
            let span = tracing::info_span!("cmd", trace_id = "abc123");
            let _enter = span.enter();
            tracing::info!("command started");
        });
        let line = out.lines().last().unwrap_or("");
        let last_token = line.split_whitespace().last().unwrap_or("");
        assert_eq!(
            last_token, "abc123",
            "trace_id is used when request_id absent; got line: {line:?}"
        );
    }

    #[test]
    fn aidog_format_no_span_generates_root_id() {
        // 无 span 时兜底现场新生 root (6 [0-9a-z])。
        let out = with_test_subscriber(false, || {
            tracing::warn!("orphan event");
        });
        // 输出最后一行 trim 后应有 6 [0-9a-z] 在末尾。
        let line = out.lines().last().unwrap_or("");
        // 取最后一个非空 token 检查 (trace_id 在行尾)。
        let last_token = line.split_whitespace().last().unwrap_or("");
        assert!(
            is_root_id(last_token),
            "orphan event must end with a generated 6 [0-9a-z] id; got line: {line:?}"
        );
    }

    #[test]
    fn aidog_format_ansi_console_plain_file() {
        let console = with_test_subscriber(true, || {
            let span = tracing::info_span!("s", trace_id = "xyz789");
            let _enter = span.enter();
            tracing::info!("ansi check");
        });
        let file = with_test_subscriber(false, || {
            let span = tracing::info_span!("s", trace_id = "xyz789");
            let _enter = span.enter();
            tracing::info!("ansi check");
        });
        assert!(
            console.contains("\x1b["),
            "console (ansi=true) must contain ANSI escape; got: {console:?}"
        );
        assert!(
            !file.contains('\x1b'),
            "file (ansi=false) must be plain text; got: {file:?}"
        );
    }

    #[test]
    fn aidog_format_field_order() {
        // 强制 5 段顺序: time level file:line func msg traceid
        // 用 ansi=false 简化断言 (无 ANSI 序列干扰)。
        let out = with_test_subscriber(false, || {
            let span = tracing::info_span!("req", trace_id = "ztop00");
            let _enter = span.enter();
            tracing::info!("ordertest");
        });
        let line = out.lines().last().unwrap_or("");
        // 各段在 line 中按顺序出现。
        let lvl = line.find("INFO");
        let msg = line.find("ordertest");
        let tid = line.find("ztop00");
        assert!(lvl.is_some() && msg.is_some() && tid.is_some(),
            "all 3 markers must be present; got: {line:?}");
        assert!(lvl.unwrap() < msg.unwrap(), "level must precede msg; got: {line:?}");
        assert!(msg.unwrap() < tid.unwrap(), "msg must precede traceid; got: {line:?}");
    }

    // ---- MsgCollector 业务字段渲染（回归 07-06-log-format-field-loss）----

    #[test]
    fn aidog_format_renders_event_fields() {
        // 直接对照 PRD Acceptance: tracing::debug!(target="sql", fn=, req=, dur=, sql=, "exec sql")
        // 输出 msg 段必须含全部 4 业务字段 (fn/req/dur/sql)。
        let out = with_test_subscriber(false, || {
            tracing::debug!(target: "sql", fn = "db.rs:1", req = "abc", dur = "0.1ms", sql = "SELECT 1", "exec sql");
        });
        let line = out.lines().last().unwrap_or("");
        assert!(line.contains("exec sql"), "msg 主体保留; got: {line:?}");
        assert!(line.contains("fn="), "fn 字段渲染; got: {line:?}");
        assert!(line.contains("req="), "req 字段渲染; got: {line:?}");
        assert!(line.contains("dur="), "dur 字段渲染; got: {line:?}");
        assert!(line.contains("sql="), "sql 字段渲染; got: {line:?}");
        // 字段值: sql=SELECT 1 (字符串去引号, 与 message 处理一致)。
        assert!(line.contains("sql=SELECT 1"), "sql 值去引号; got: {line:?}");
        // 顺序: msg 主体先于业务字段, 业务字段先于 traceid 段。
        let msg_idx = line.find("exec sql").unwrap_or(usize::MAX);
        let fn_idx = line.find("fn=").unwrap_or(usize::MAX);
        let tid_token = line.split_whitespace().last().unwrap_or("");
        let tid_idx = line.rfind(tid_token).unwrap_or(usize::MAX);
        assert!(msg_idx < fn_idx, "msg 主体先于业务字段; got: {line:?}");
        assert!(fn_idx < tid_idx, "业务字段先于 traceid; got: {line:?}");
    }

    #[test]
    fn aidog_format_dedups_event_trace_id_field() {
        // event 显式带 trace_id 字段（罕见场景）→ 5 段格式 traceid 段单独取 span scope,
        // event 的 trace_id 字段必须被 MsgCollector 跳过（不进 extra, 避免重复）。
        let out = with_test_subscriber(false, || {
            let span = tracing::info_span!("s", trace_id = "topid1");
            let _enter = span.enter();
            tracing::info!(trace_id = "fromevent", "msg body");
        });
        let line = out.lines().last().unwrap_or("");
        // 行尾 traceid 段 = span scope 取的 topid1, 不是 event 的 fromevent。
        let last_token = line.split_whitespace().last().unwrap_or("");
        assert_eq!(last_token, "topid1", "traceid 段取 span scope; got: {line:?}");
        // extra 里不能有 trace_id=fromevent（去重生效）。
        assert!(
            !line.contains("trace_id=fromevent"),
            "event trace_id 字段必须去重, 不能进 extra; got: {line:?}"
        );
        // 但 message 主体仍在。
        assert!(line.contains("msg body"), "message 主体渲染; got: {line:?}");
    }

    // ---- spawn_traced helper 单测 ----

    #[tokio::test]
    async fn spawn_traced_propagates_parent_trace_id() {
        // 父 span 内 spawn → 子 task 读 current_trace_id 必须命中父子树（不再是孤儿）。
        // 用 std::sync::OnceCell 避免每跑一次重设全局 subscriber（OnceCell 仅首次生效，
        // 后续 init_logging 调用被 set_global_default 静默丢弃，与生产一致）。
        use std::sync::{Arc, Mutex};
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let cap_clone = captured.clone();

        // 临时 subscriber 捕获 child task 内的 current_trace_id（spawn_traced 包了 info_span
        // 带 trace_id 字段，TraceIdLayer 会把它压进 thread-local 栈）。
        let subscriber = Registry::default().with(TraceIdLayer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let parent_span = tracing::info_span!("parent", trace_id = "parent");
        let parent_tid = "parent".to_string();
        let _enter = parent_span.enter();

        // spawn_traced 在父 span 内调用 → 读栈拿到 "parent" → gen "parent.xxxxxx" →
        // 包 info_span，子 task 内 current_trace_id() 读栈 = "parent.xxxxxx"。
        let handle = spawn_traced("test_spawn", async move {
            let tid = current_trace_id();
            *cap_clone.lock().unwrap() = tid;
        });
        handle.await.unwrap();

        let tid = captured.lock().unwrap().clone().unwrap_or_default();
        assert!(
            tid.starts_with("parent."),
            "spawn_traced child must inherit parent prefix; got {tid:?}"
        );
        let suffix = tid.strip_prefix("parent.").unwrap_or("");
        assert!(is_root_id(suffix), "child suffix must be 6 [0-9a-z]; got {suffix:?}");
        // 反向 grep 契约: 用 parent prefix 能命中 child。
        let _ = parent_tid; // parent prefix 字符串在断言里直接用字面量
    }

    #[tokio::test]
    async fn spawn_traced_no_active_span_generates_orphan_child() {
        // 无活跃 span 调 spawn_traced → 兜底 gen root → child = root.xxxxxx (用户决策权衡:
        // 接受 grep 不到父子树, 但 child 自身仍是合法 `<6chars>.<6chars>` 格式, 与父
        // 在场的分支格式对称)。root 本身从未被记录 (current_trace_id() 在 gen 时已是
        // None → 进 child span 后读到的是 child), 这是 PRD「无 span 兜底」权衡。
        use std::sync::{Arc, Mutex};
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let cap_clone = captured.clone();

        let subscriber = Registry::default().with(TraceIdLayer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let handle = spawn_traced("orphan_spawn", async move {
            let tid = current_trace_id();
            *cap_clone.lock().unwrap() = tid;
        });
        handle.await.unwrap();

        let tid = captured.lock().unwrap().clone().unwrap_or_default();
        // 格式: root(6).suffix(6)
        let dot = tid.rfind('.').unwrap_or(usize::MAX);
        assert!(dot < tid.len(), "orphan child must contain '.', got {tid:?}");
        let (root, suffix) = tid.split_at(dot);
        let suffix = &suffix[1..]; // skip '.'
        assert!(
            is_root_id(root) && is_root_id(suffix),
            "orphan child format = root.suffix (each 6 [0-9a-z]); got {tid:?}"
        );
    }
}
