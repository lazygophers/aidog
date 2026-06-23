/// Shared HTTP client builder with optional upstream proxy support.
use super::db::Db;
use super::models::ProxyClientSettings;
use std::sync::Arc;
use std::time::Duration;

/// Load system proxy client settings from DB.
pub async fn load_proxy_client_settings(db: &Arc<Db>) -> ProxyClientSettings {
    super::db::get_setting(db, "proxy", "proxy_client")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Parse platform `extra` JSON for `proxy_enabled` field.
/// Returns:
/// - `None` = field missing → follow system default
/// - `Some(true)` = explicitly enabled
/// - `Some(false)` = explicitly disabled
pub fn platform_proxy_enabled(extra: &str) -> Option<bool> {
    if extra.trim().is_empty() { return None; }
    serde_json::from_str::<serde_json::Value>(extra).ok()
        .and_then(|v| v.get("proxy_enabled").and_then(|f| f.as_bool()))
}

/// Build a reqwest client with optional proxy and timeout.
/// `force_proxy`: overrides platform-level setting for callers that always want proxy or never.
/// - `None` = check system proxy + platform `proxy_enabled`
/// - `Some(true)` = always use system proxy if configured
/// - `Some(false)` = never use proxy
pub async fn build_http_client(
    db: &Arc<Db>,
    timeout_secs: u64,
    conn_timeout_secs: u64,
    platform_extra: Option<&str>,
    force_proxy: Option<bool>,
) -> reqwest::Client {
    let settings = load_proxy_client_settings(db).await;

    let use_proxy = match force_proxy {
        Some(v) => v && settings.enabled,
        None => {
            let platform_ok = platform_extra
                .and_then(platform_proxy_enabled)
                .unwrap_or(true); // field missing → follow system
            settings.enabled && platform_ok
        }
    };

    let mut builder = reqwest::Client::builder();
    if timeout_secs > 0 {
        builder = builder.timeout(Duration::from_secs(timeout_secs));
    }
    if conn_timeout_secs > 0 {
        builder = builder.connect_timeout(Duration::from_secs(conn_timeout_secs));
    }
    if use_proxy {
        if let Some(proxy) = settings.to_reqwest_proxy() {
            builder = builder.proxy(proxy);
        }
    }
    builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

/// Convenience: build client without platform context (always follows system proxy).
pub async fn build_http_client_system(
    db: &Arc<Db>,
    timeout_secs: u64,
    conn_timeout_secs: u64,
) -> reqwest::Client {
    build_http_client(db, timeout_secs, conn_timeout_secs, None, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_proxy_enabled_empty_returns_none() {
        assert_eq!(platform_proxy_enabled(""), None);
        assert_eq!(platform_proxy_enabled("  "), None);
    }

    #[test]
    fn platform_proxy_enabled_explicit_true() {
        assert_eq!(platform_proxy_enabled(r#"{"proxy_enabled":true}"#), Some(true));
    }

    #[test]
    fn platform_proxy_enabled_explicit_false() {
        assert_eq!(platform_proxy_enabled(r#"{"proxy_enabled":false}"#), Some(false));
    }

    #[test]
    fn platform_proxy_enabled_missing_field_returns_none() {
        assert_eq!(platform_proxy_enabled(r#"{"other_field":1}"#), None);
    }

    #[test]
    fn platform_proxy_enabled_invalid_json_returns_none() {
        assert_eq!(platform_proxy_enabled("not-json"), None);
    }
}
