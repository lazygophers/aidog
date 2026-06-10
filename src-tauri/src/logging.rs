use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Layer, Registry};
use tracing_appender::rolling::RollingFileAppender;
use std::time::Duration;

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

/// Initialize logging: dev → console only; release → console + optional file
pub fn init_logging(data_dir: &std::path::Path, settings: &AppLogSettings) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&settings.level));

    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_filter(env_filter.clone());

    if cfg!(debug_assertions) {
        // Dev mode: console only
        let subscriber = Registry::default().with(console_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
        tracing::info!("logging initialized (dev mode, console only)");
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
            .with_filter(env_filter);

        let subscriber = Registry::default()
            .with(console_layer)
            .with(file_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
        tracing::info!("logging initialized (release mode, console + file, retention={}h)", settings.retention_hours);
    } else {
        // Release with file logging disabled
        let subscriber = Registry::default().with(console_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
        tracing::info!("logging initialized (release mode, console only, file disabled)");
    }
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
