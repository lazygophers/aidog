/// Shared HTTP client builder with optional upstream proxy support.
use super::db::Db;
use super::models::ProxyClientSettings;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
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

/// Client cache key: (use_proxy, timeout_secs, conn_timeout_secs)
type ClientKey = (bool, u64, u64);

/// Simple LRU cache for reqwest::Client instances.
/// Reuses TLS/connections across requests with same proxy+timeout config.
struct ClientCache {
    map: HashMap<ClientKey, Arc<reqwest::Client>>,
    order: Vec<ClientKey>, // For LRU eviction
    capacity: usize,
}

impl ClientCache {
    fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: Vec::new(),
            capacity,
        }
    }

    fn get(&self, key: ClientKey) -> Option<Arc<reqwest::Client>> {
        self.map.get(&key).cloned()
    }

    fn put(&mut self, key: ClientKey, client: Arc<reqwest::Client>) {
        if self.map.contains_key(&key) {
            return;
        }
        if self.map.len() >= self.capacity {
            if let Some(old_key) = self.order.first() {
                self.map.remove(old_key);
                self.order.remove(0);
            }
        }
        self.map.insert(key, client);
        self.order.push(key);
    }
}

/// Global client cache: lazy-initialized on first use.
/// Capacity=16 covers common (proxy, timeout) combinations.
fn global_client_cache() -> &'static Arc<RwLock<ClientCache>> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Arc<RwLock<ClientCache>>> = OnceLock::new();
    CACHE.get_or_init(|| Arc::new(RwLock::new(ClientCache::new(16))))
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
///
/// Caches clients by (use_proxy, timeout_secs, conn_timeout_secs) to reuse TLS/connections.
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

    let cache_key = (use_proxy, timeout_secs, conn_timeout_secs);

    // Try cache first (read lock, fast path)
    {
        let cache = global_client_cache().read().unwrap();
        if let Some(client) = cache.get(cache_key) {
            return (*client).clone();
        }
    }

    // Cache miss: build new client
    // proxy 协商：
    // - `use_proxy=true`（用户显式配 DB proxy）：`.proxy(explicit)` 自动关 auto_sys_proxy
    //   （reqwest 文档：调 .proxy() 后不再读 env），env 代理天然失效。
    // - `use_proxy=false`：必须显式 `.no_proxy()` 关闭 reqwest 默认的 auto_sys_proxy。
    //   否则 reqwest 读 HTTPS_PROXY/HTTP_PROXY env —— AirDog 自身就是代理，用户场景常设
    //   HTTPS_PROXY=127.0.0.1:<aidog_port>，致 forward 到上游的请求又走回 AirDog 自己，
    //   形成 CONNECT 隧道递归（MITM→forward→reqwest(env proxy=AirDog)→CONNECT→MITM→…），
    //   最终某层资源耗尽 / h2 stream 中途被 reset，客户端看到 HTTP/2 stream CANCEL。
    //   转发链禁依赖 env proxy —— 上游连通性由用户在 DB 显式配的 proxy 负责。
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
    } else {
        builder = builder.no_proxy();
    }
    let client = builder.build().unwrap_or_else(|_| reqwest::Client::new());

    // Cache for reuse (write lock)
    {
        let mut cache = global_client_cache().write().unwrap();
        // Double-check in case another thread already inserted
        if !cache.map.contains_key(&cache_key) {
            cache.put(cache_key, Arc::new(client.clone()));
        }
    }

    client
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

    // ── build_http_client 禁 env proxy 回归（修 HTTP/2 stream CANCEL 根因）──
    // 背景: 用户场景常设 HTTPS_PROXY=http://127.0.0.1:<aidog_port> 让流量走 AirDog。
    // forward 到上游的 reqwest 若读 env proxy → 又走 AirDog → CONNECT 隧道递归
    // → 某层 h2 stream reset → 客户端 HTTP/2 stream CANCEL。
    // 修复: use_proxy=false 时显式 .no_proxy() 关 reqwest auto_sys_proxy。
    // 本测试起 stub proxy 计数连接 + stub 上游，设 HTTPS_PROXY env 指向 stub proxy，
    // 断言经 build_http_client 构的 client 请求上游时不连 stub proxy（env proxy 被禁）。
    //
    // ponytail: env::set_var 临时改 + 恢复，单测内顺序执行；其他 #[test] 不触 env，无并行污染。
    #[tokio::test]
    async fn build_http_client_disables_env_proxy_when_no_db_proxy() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        // 1. stub proxy：accept 连接计数后立刻 drop（模拟 proxy 端口；不该被连）。
        let proxy_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        let connect_count = Arc::new(AtomicUsize::new(0));
        let cc = connect_count.clone();
        tokio::spawn(async move {
            loop {
                match proxy_listener.accept().await {
                    Ok((stream, _)) => {
                        cc.fetch_add(1, Ordering::SeqCst);
                        drop(stream);
                    }
                    Err(_) => break,
                }
            }
        });

        // 2. stub 上游 axum server（http，返 200 + 固定 body）。
        let upstream_body = "upstream-ok";
        let upstream_app = axum::Router::new().fallback(axum::routing::any(move || async move {
            (axum::http::StatusCode::OK, upstream_body)
        }));
        let up_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let up_addr = up_listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(up_listener, upstream_app).await.ok() });

        // 3. 设 HTTPS_PROXY env 指向 stub proxy（模拟用户配 AirDog 为系统代理）。
        // edition 2024：std::env::set_var / remove_var 标记 unsafe（多线程 mutating env UB），
        // 单测内顺序执行无并发，包 unsafe 块满足编译检查。
        let prev_https = std::env::var("HTTPS_PROXY").ok();
        unsafe {
            std::env::set_var("HTTPS_PROXY", format!("http://{proxy_addr}"));
            std::env::set_var("NO_PROXY", ""); // 清 NO_PROXY 绕过
        }

        // 4. test_db（proxy_client settings 默认空 → use_proxy=false）。
        let db = crate::gateway::db::test_support::test_db().await;
        let client = build_http_client(&Arc::new(db), 0, 0, None, None).await;

        // 5. 请求上游：use_proxy=false + .no_proxy() → 直连上游，不连 stub proxy。
        let url = format!("http://{up_addr}/");
        let outcome: Result<(), reqwest::Error> = async {
            let resp = client.get(&url).send().await?;
            let body = resp.text().await?;
            assert_eq!(body, "upstream-ok", "必须直连上游拿到 body");
            Ok::<(), reqwest::Error>(())
        }
        .await;

        // 6. 恢复 env（防污染后续测试）。
        match prev_https {
            Some(v) => unsafe { std::env::set_var("HTTPS_PROXY", v) },
            None => unsafe { std::env::remove_var("HTTPS_PROXY") },
        }

        outcome.expect("request must succeed via direct connection, not env proxy");
        // stub proxy 不该被连（连了 = env proxy 生效 = 修复回归）。
        assert_eq!(
            connect_count.load(Ordering::SeqCst),
            0,
            "use_proxy=false 时 reqwest 必须禁 env proxy（.no_proxy），stub proxy 不应被连"
        );
    }
}
