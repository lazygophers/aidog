use tracing_subscriber::{layer::SubscriberExt, layer::Context, EnvFilter, Layer, Registry};
use tracing_subscriber::registry::LookupSpan;
use tracing_appender::rolling::RollingFileAppender;
use std::cell::RefCell;
use std::time::Duration;

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
        if let Some(tid) = visitor.id {
            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(SpanTraceId(tid));
            }
        }
    }

    fn on_enter(&self, id: &tracing::span::Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            if let Some(SpanTraceId(tid)) = span.extensions().get::<SpanTraceId>() {
                let tid = tid.clone();
                TRACE_ID_STACK.with(|s| s.borrow_mut().push(tid));
            }
        }
    }

    fn on_exit(&self, id: &tracing::span::Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            if span.extensions().get::<SpanTraceId>().is_some() {
                TRACE_ID_STACK.with(|s| {
                    s.borrow_mut().pop();
                });
            }
        }
    }
}

/// Application log settings (stored in settings table)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppLogSettings {
    /// Whether to write log files (only effective in release)
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

/// Initialize logging: dev → console only; release → console + optional file
pub fn init_logging(data_dir: &std::path::Path, settings: &AppLogSettings) {
    // Dev 模式: 无论用户配置如何, 控制台永远 debug 级 (仍允许 RUST_LOG 覆盖以便更细粒度调试)。
    // Release 模式: 遵循用户 settings.level。
    let console_filter = if cfg!(debug_assertions) {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter("debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter(&settings.level))
    };
    // 文件层始终遵循用户配置 (release-only)。
    let file_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| default_filter(&settings.level));

    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_filter(console_filter);

    if cfg!(debug_assertions) {
        // Dev mode: console only (forced debug)
        let subscriber = Registry::default().with(TraceIdLayer).with(console_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
        tracing::info!("logging initialized (dev mode, console only, forced debug)");
    } else if settings.file_enabled {
        // Release mode: console + file
        let log_dir = data_dir.join("logs");
        let _ = std::fs::create_dir_all(&log_dir);

        let file_appender = RollingFileAppender::builder()
            .rotation(tracing_appender::rolling::Rotation::HOURLY)
            .filename_prefix("aidog")
            .filename_suffix("log")
            .max_log_files(max_files_from_retention(settings.retention_hours))
            .build(&log_dir)
            .expect("failed to create log file appender");

        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(file_appender)
            .with_filter(file_filter);

        let subscriber = Registry::default()
            .with(TraceIdLayer)
            .with(console_layer)
            .with(file_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
        tracing::info!("logging initialized (release mode, console + file, retention={}h)", settings.retention_hours);
    } else {
        // Release with file logging disabled
        let subscriber = Registry::default().with(TraceIdLayer).with(console_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
        tracing::info!("logging initialized (release mode, console only, file disabled)");
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

/// 生成 8 位短 trace id（链路追踪），用于 tracing span 的 trace_id 字段。
/// 全后端统一经此函数取 id，禁各处自造。span 内 .await 的下游调用自动继承该字段。
pub fn new_trace_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..8].to_string()
}

fn max_files_from_retention(hours: u32) -> usize {
    if hours == 0 { 72 } else { hours as usize } // 0 = keep up to 72 files (~3 days) as fallback
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
            if path.extension().and_then(|e| e.to_str()) == Some("log") {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }
}
