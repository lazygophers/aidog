use super::models::ProxyClientSettings;
/// Shared HTTP client builder with optional upstream proxy support.
use aidog_db::Db;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Load system proxy client settings from DB.
/// 非请求路径用（model_test / price_sync / logo_sync / quota）。请求路径改读 ProxyState 缓存。
pub async fn load_proxy_client_settings(db: &Db) -> ProxyClientSettings {
    aidog_db::get_setting(db, "proxy", "proxy_client")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Client cache key: (use_proxy, proxy 地址串, effective no_proxy, timeout_secs, conn_timeout_secs)
/// proxy 地址与 no_proxy 都入键：这两者在 client 构建期固化，改配置必须重建 client 才生效。
/// 漏掉地址会命中旧 client（审计发现 10：换 socks5 host/port 后请求继续打旧代理直到重启）。
type ClientKey = (bool, String, String, u64, u64);

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

    fn get(&self, key: &ClientKey) -> Option<Arc<reqwest::Client>> {
        self.map.get(key).cloned()
    }

    /// LRU touch：命中后把 key 移到 order 尾部（不更新则 order 只反映插入序 = FIFO）
    fn touch(&mut self, key: &ClientKey) {
        self.order.retain(|k| k != key);
        self.order.push(key.clone());
    }

    fn put(&mut self, key: ClientKey, client: Arc<reqwest::Client>) {
        if self.map.contains_key(&key) {
            return;
        }
        if self.map.len() >= self.capacity
            && let Some(old_key) = self.order.first()
        {
            self.map.remove(old_key);
            self.order.remove(0);
        }
        self.map.insert(key.clone(), client);
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
/// - `None` = field missing → 缺省按直连
/// - `Some(true)` = explicitly enabled
/// - `Some(false)` = explicitly disabled
pub fn platform_proxy_enabled(extra: &str) -> Option<bool> {
    if extra.trim().is_empty() {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(extra)
        .ok()
        .and_then(|v| v.get("proxy_enabled").and_then(|f| f.as_bool()))
}

/// 出站归属：本次出站属于哪一类（issue #64 T01，代理判定唯一 seam 的输入）。
pub enum ProxyAttribution<'a> {
    /// 平台出站：该平台 `extra.proxy_enabled` 开关值（None = 未配置 → 直连）。
    Platform(Option<bool>),
    /// 非平台出站（余额/logo/model_fetch/skills 等）：读「其它请求使用代理」。
    NonPlatform,
    /// MITM 旁路出站：连接绑定分组内官方平台的开关集合（元素 = 各平台 `extra.proxy_enabled`）。
    /// 任一显式开即走代理；集合为空回落非平台规则（已接入 `mitm_bypass.rs`）。
    MitmBypass(&'a [Option<bool>]),
}

/// 平台代理开关三态（review smell 3：封装裸 `Option<bool>`，语义单点在此）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProxySwitch {
    /// 显式开
    On,
    /// 显式关
    Off,
    /// 缺省（缺键 / 空白 / 损坏 JSON）= 直连
    Default,
}

impl ProxySwitch {
    pub fn from_flag(flag: Option<bool>) -> Self {
        match flag {
            Some(true) => Self::On,
            Some(false) => Self::Off,
            None => Self::Default,
        }
    }

    /// 平台行 extra JSON → 三态（空白 / 缺键 / 损坏 = Default）。
    pub fn from_extra(extra: &str) -> Self {
        Self::from_flag(platform_proxy_enabled(extra))
    }

    /// 缺省直连：只有显式 On 走代理。
    pub fn allows_proxy(self) -> bool {
        matches!(self, Self::On)
    }

    /// 旁路集合语义：任一显式开即开。
    pub fn any_on(flags: &[Option<bool>]) -> bool {
        flags.contains(&Some(true))
    }
}

/// 代理判定 seam（纯函数）：本次出站是否走代理。
/// 共同前置：代理主机为空 → 恒直连。免代理名单仍由 HTTP 客户端在走代理时按域名排除
/// （名单优先于平台开关，现状不变），不入本判定。
pub fn should_use_proxy(settings: &ProxyClientSettings, attribution: ProxyAttribution<'_>) -> bool {
    // 共同前置：代理主机为空 = 未配置代理，恒直连。
    if settings.host.trim().is_empty() {
        return false;
    }
    match attribution {
        // 平台开关：缺省（Default）按直连（后端防御，不跟随全局）。
        ProxyAttribution::Platform(flag) => ProxySwitch::from_flag(flag).allows_proxy(),
        ProxyAttribution::NonPlatform => settings.others_enabled,
        // 分组内官方平台任一显式开即走代理；集合为空回落非平台规则。
        ProxyAttribution::MitmBypass(flags) => {
            if flags.is_empty() {
                settings.others_enabled
            } else {
                ProxySwitch::any_on(flags)
            }
        }
    }
}

/// 平台出站判定便捷封装：extra JSON → `proxy_enabled` 三态 → should_use_proxy。
pub fn platform_should_use_proxy(settings: &ProxyClientSettings, extra: &str) -> bool {
    should_use_proxy(
        settings,
        ProxyAttribution::Platform(platform_proxy_enabled(extra)),
    )
}

/// 测试构建专用护栏：只解析回环主机名，其余一律报错点名主机，杜绝测试真实外呼
/// （.claude/rules/test-no-real-calls.md）。shortcut: IP 字面量不经 resolver，非回环 IP 拦不到；出现再换 connector 层。
#[cfg(test)]
struct LoopbackOnlyResolver;

#[cfg(test)]
impl reqwest::dns::Resolve for LoopbackOnlyResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            if host.eq_ignore_ascii_case("localhost") {
                let addrs: Vec<std::net::SocketAddr> = vec![
                    (std::net::Ipv4Addr::LOCALHOST, 0).into(),
                    (std::net::Ipv6Addr::LOCALHOST, 0).into(),
                ];
                Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
            } else {
                Err(format!("blocked non-loopback host {host} in test build").into())
            }
        })
    }
}

/// Build a reqwest client with optional proxy and timeout.
/// `use_proxy` 由 caller 经判定 seam（`should_use_proxy`）算出后传入——本函数只消费判定结果，
/// 不再做任何归属/开关判定（issue #64 T01）。
///
/// `settings` 由 caller 传入：请求路径从 ProxyState 缓存借（避免每请求 DB 读），
/// 非请求路径用 `load_proxy_client_settings(db).await` 现读现用。
///
/// Caches clients by (use_proxy, timeout_secs, conn_timeout_secs) to reuse TLS/connections.
pub async fn build_http_client(
    settings: &ProxyClientSettings,
    timeout_secs: u64,
    conn_timeout_secs: u64,
    use_proxy: bool,
) -> reqwest::Client {

    let cache_key = (
        use_proxy,
        if use_proxy {
            // 代理完整地址（含认证 + DNS 模式）入键：构建期固化，任何一项变更都须重建 client
            format!(
                "{}://{}:{}#{}#{}",
                settings.proxy_type, settings.host, settings.port, settings.username, settings.dns_over_proxy
            )
        } else {
            String::new()
        },
        if use_proxy {
            settings.effective_no_proxy().to_string()
        } else {
            String::new()
        },
        timeout_secs,
        conn_timeout_secs,
    );

    // Try cache first (read lock, fast path)
    {
        let cache = global_client_cache().read().unwrap();
        if let Some(client) = cache.get(&cache_key) {
            drop(cache);
            global_client_cache().write().unwrap().touch(&cache_key);
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
    #[cfg(test)]
    {
        builder = builder.dns_resolver(Arc::new(LoopbackOnlyResolver));
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

/// 按归属构建（issue #64 T04）：`platform_extra` = 平台行 extra JSON（平台归属，读
/// `extra.proxy_enabled`，空白/缺键/损坏 → 缺省直连）；`None` = 非平台归属（others_enabled）。
/// quota 注入与 model_fetch 共用。空白 extra 归平台归属（review 需求轴 3：缺省直连，
/// 不回落 others_enabled）。
pub async fn build_http_client_scoped(
    db: &Arc<Db>,
    timeout_secs: u64,
    conn_timeout_secs: u64,
    platform_extra: Option<&str>,
) -> reqwest::Client {
    let settings = load_proxy_client_settings(db).await;
    let use_proxy = match platform_extra {
        Some(extra) => platform_should_use_proxy(&settings, extra),
        None => should_use_proxy(&settings, ProxyAttribution::NonPlatform),
    };
    build_http_client(&settings, timeout_secs, conn_timeout_secs, use_proxy).await
}

/// Convenience: build client for 非平台出站（归属 NonPlatform，读「其它请求使用代理」）。
/// 非请求路径用（model_fetch/price_sync/logo_sync/quota）：现读 DB settings 再委托。
pub async fn build_http_client_system(
    db: &Arc<Db>,
    timeout_secs: u64,
    conn_timeout_secs: u64,
) -> reqwest::Client {
    let settings = load_proxy_client_settings(db).await;
    let use_proxy = should_use_proxy(&settings, ProxyAttribution::NonPlatform);
    build_http_client(&settings, timeout_secs, conn_timeout_secs, use_proxy).await
}

/// 注册 search / notification 出站 client 工厂（issue #64 review hard issue 2）。
/// 插入 aidog_ctx 的单例 DB 延迟到调用时读取（外壳启动时注册早于开库）；无 DB 时返回错误。
/// 依赖方向保持 core → search / notification，不形成反向依赖环。
pub fn install_non_platform_client_factories() {
    aidog_search::set_client_factory(Arc::new(|| {
        Box::pin(async {
            let Some(db) = aidog_ctx::try_db() else {
                return Err("proxy-aware search client requested before DB init".to_string());
            };
            let db = Arc::new(db.clone());
            Ok(build_http_client_system(&db, 30, 10).await)
        })
    }));
    aidog_search::set_fingerprint_proxy_provider(Arc::new(|| {
        Box::pin(async {
            let db = aidog_ctx::try_db()?;
            let db = Arc::new(db.clone());
            let settings = load_proxy_client_settings(&db).await;
            should_use_proxy(&settings, ProxyAttribution::NonPlatform).then(|| {
                let uri = settings.proxy_uri()?;
                Some(aidog_search::FingerprintProxy {
                    uri,
                    username: settings.username.clone(),
                    password: settings.password.clone(),
                    no_proxy: settings.effective_no_proxy().to_string(),
                })
            })?
        })
    }));
    aidog_notification::channels::set_client_factory(Arc::new(|| {
        Box::pin(async {
            let Some(db) = aidog_ctx::try_db() else {
                return Err("proxy-aware notification client requested before DB init".to_string());
            };
            let db = Arc::new(db.clone());
            Ok(build_http_client_system(&db, 10, 0).await)
        })
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err_chain(e: &dyn std::error::Error) -> String {
        let mut out = e.to_string();
        let mut cur = e.source();
        while let Some(s) = cur {
            out.push_str(" | ");
            out.push_str(&s.to_string());
            cur = s.source();
        }
        out
    }

    /// 护栏：测试构建下连非回环主机必须响亮失败并点名主机（test-no-real-calls.md）。
    #[tokio::test]
    async fn test_build_blocks_non_loopback_host() {
        let c = build_http_client(&ProxyClientSettings::default(), 5, 5, false).await;
        let e = c.get("https://open.bigmodel.cn/").send().await.unwrap_err();
        let chain = err_chain(&e);
        assert!(chain.contains("blocked non-loopback host open.bigmodel.cn"), "{chain}");
    }

    /// 回环假上游（含 localhost 主机名）不受护栏影响。
    #[tokio::test]
    async fn test_build_allows_loopback_host() {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut s, _) = l.accept().await.unwrap();
            let mut b = [0u8; 1024];
            let _ = s.read(&mut b).await;
            s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok").await.unwrap();
        });
        let c = build_http_client(&ProxyClientSettings::default(), 5, 5, false).await;
        let r = c.get(format!("http://localhost:{port}/")).send().await.unwrap();
        assert_eq!(r.text().await.unwrap(), "ok");
    }

    #[test]
    fn platform_proxy_enabled_empty_returns_none() {
        assert_eq!(platform_proxy_enabled(""), None);
        assert_eq!(platform_proxy_enabled("  "), None);
    }

    #[test]
    fn platform_proxy_enabled_explicit_true() {
        assert_eq!(
            platform_proxy_enabled(r#"{"proxy_enabled":true}"#),
            Some(true)
        );
    }

    #[test]
    fn platform_proxy_enabled_explicit_false() {
        assert_eq!(
            platform_proxy_enabled(r#"{"proxy_enabled":false}"#),
            Some(false)
        );
    }

    #[test]
    fn platform_proxy_enabled_missing_field_returns_none() {
        assert_eq!(platform_proxy_enabled(r#"{"other_field":1}"#), None);
    }

    #[test]
    fn platform_proxy_enabled_invalid_json_returns_none() {
        assert_eq!(platform_proxy_enabled("not-json"), None);
    }

    // ── 代理判定 seam（issue #64 T01）──

    fn seam_settings(others: bool) -> ProxyClientSettings {
        ProxyClientSettings {
            others_enabled: others,
            host: "127.0.0.1".into(),
            ..Default::default()
        }
    }

    #[test]
    fn seam_empty_host_always_direct() {
        let s = ProxyClientSettings {
            host: String::new(),
            others_enabled: true,
            ..Default::default()
        };
        assert!(!should_use_proxy(&s, ProxyAttribution::Platform(Some(true))));
        assert!(!should_use_proxy(&s, ProxyAttribution::NonPlatform));
        let flags = [Some(true), Some(false)];
        assert!(!should_use_proxy(&s, ProxyAttribution::MitmBypass(&flags)));
        // 空白主机同样视为未配置
        let s2 = ProxyClientSettings {
            host: "   ".into(),
            others_enabled: true,
            ..Default::default()
        };
        assert!(!should_use_proxy(&s2, ProxyAttribution::NonPlatform));
    }

    #[test]
    fn seam_platform_explicit_and_missing() {
        let s = seam_settings(false);
        assert!(should_use_proxy(&s, ProxyAttribution::Platform(Some(true))));
        assert!(!should_use_proxy(&s, ProxyAttribution::Platform(Some(false))));
        // 缺省 = 直连（后端防御，不跟随全局）
        assert!(!should_use_proxy(&s, ProxyAttribution::Platform(None)));
        // 便捷封装同语义
        assert!(platform_should_use_proxy(&s, r#"{"proxy_enabled":true}"#));
        assert!(!platform_should_use_proxy(&s, r#"{"proxy_enabled":false}"#));
        assert!(!platform_should_use_proxy(&s, "{}"));
        assert!(!platform_should_use_proxy(&s, ""));
    }

    #[test]
    fn seam_non_platform_switch() {
        assert!(should_use_proxy(
            &seam_settings(true),
            ProxyAttribution::NonPlatform
        ));
        assert!(!should_use_proxy(
            &seam_settings(false),
            ProxyAttribution::NonPlatform
        ));
    }

    /// model_fetch 创建态拉模型（issue #64 T04 路径接入测试）：表单「使用代理」开关
    /// Some(v) = 平台归属；None（旧前端不传参）= 非平台归属，读 others_enabled。
    #[test]
    fn seam_model_fetch_form_switch_composition() {
        let s = seam_settings(false);
        assert!(should_use_proxy(
            &s,
            ProxyAttribution::Platform(Some(true))
        ));
        assert!(!should_use_proxy(
            &s,
            ProxyAttribution::Platform(Some(false))
        ));
        assert!(!should_use_proxy(&s, ProxyAttribution::NonPlatform));
        assert!(should_use_proxy(
            &seam_settings(true),
            ProxyAttribution::NonPlatform
        ));
    }

    /// model_test（issue #64 路径接入测试）：平台行 extra → 平台归属；
    /// 空白 / 缺键 extra = 平台缺省直连（不回落 others_enabled）。
    #[test]
    fn seam_model_test_platform_extra_attribution() {
        let s = seam_settings(false);
        assert!(platform_should_use_proxy(&s, r#"{"proxy_enabled":true}"#));
        assert!(!platform_should_use_proxy(&s, r#"{"proxy_enabled":false}"#));
        assert!(!platform_should_use_proxy(&s, "{}"));
        assert!(!platform_should_use_proxy(&s, ""), "空白 extra = 平台缺省直连");
    }

    #[test]
    fn seam_mitm_bypass_any_on_and_empty_fallback() {
        let s = seam_settings(false);
        // 任一显式开 → 走代理
        let flags = [Some(false), Some(true), None];
        assert!(should_use_proxy(&s, ProxyAttribution::MitmBypass(&flags)));
        // 全关（显式 false / 缺省）→ 直连
        let all_off = [Some(false), None];
        assert!(!should_use_proxy(&s, ProxyAttribution::MitmBypass(&all_off)));
        // 集合为空 → 回落非平台规则
        let empty: [Option<bool>; 0] = [];
        assert!(!should_use_proxy(&s, ProxyAttribution::MitmBypass(&empty)));
        let s_on = seam_settings(true);
        assert!(should_use_proxy(&s_on, ProxyAttribution::MitmBypass(&empty)));
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
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        // 1. stub proxy：accept 连接计数后立刻 drop（模拟 proxy 端口；不该被连）。
        let proxy_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        let connect_count = Arc::new(AtomicUsize::new(0));
        let cc = connect_count.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                cc.fetch_add(1, Ordering::SeqCst);
                drop(stream);
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
        let db = aidog_db::test_support::test_db().await;
        let settings = load_proxy_client_settings(&db).await;
        let client = build_http_client(&settings, 0, 0, false).await;

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

    /// stub HTTP 代理：计请求数并回 200（issue #64 T04/T05 行为测试共用 idiom）。
    async fn spawn_hit_counting_proxy() -> (
        std::net::SocketAddr,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let app = axum::Router::new().fallback(axum::routing::any(move || {
            let h = h.clone();
            async move {
                h.fetch_add(1, Ordering::SeqCst);
                (axum::http::StatusCode::OK, "")
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });
        (addr, hits)
    }

    /// issue #64 T04：`build_http_client_scoped` 按平台 extra / 非平台开关切换代理。
    /// others_enabled=true + 平台 proxy_enabled=false → 平台关压过其它开（直连）；
    /// 平台 true / 非平台归属 → 走代理（stub proxy 计数）。直连目标用非回环域名，
    /// 测试构建的 LoopbackOnlyResolver 直接拒解（test-no-real-calls 护栏兼做直连断言）。
    #[tokio::test]
    async fn scoped_client_follows_platform_extra_switch() {
        let (proxy_addr, hits) = spawn_hit_counting_proxy().await;
        let db = Arc::new(aidog_db::test_support::test_db().await);
        aidog_db::set_setting(
            &db,
            aidog_db::SetSettingInput {
                scope: "proxy".into(),
                key: "proxy_client".into(),
                value: serde_json::json!({
                    "proxy_type": "http",
                    "host": "127.0.0.1",
                    "port": proxy_addr.port(),
                    "others_enabled": true,
                    // 覆盖 DEFAULT_NO_PROXY（其中含 127.0.0.1，会让代理目标绕过代理）
                    "no_proxy": "nothing.invalid",
                }),
            },
        )
        .await
        .unwrap();

        let url = "http://example.invalid/x"; // 直连必被 resolver 拦；走代理则由 proxy 端应答

        // 平台开 → 走代理
        let c = build_http_client_scoped(&db, 0, 5, Some(r#"{"proxy_enabled":true}"#)).await;
        c.get(url).send().await.expect("proxy_enabled=true must proxy");
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);

        // 平台关（others_enabled=true 也压不过）→ 直连 → resolver 拦
        let c = build_http_client_scoped(&db, 0, 5, Some(r#"{"proxy_enabled":false}"#)).await;
        assert!(
            c.get(url).send().await.is_err(),
            "platform proxy_enabled=false must direct-connect despite others_enabled=true"
        );
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);

        // 非平台归属（None）→ others_enabled=true → 走代理
        let c = build_http_client_scoped(&db, 0, 5, None).await;
        c.get(url).send().await.expect("None attribution must follow others_enabled");
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);

        // 平台 extra 空白表示平台缺省直连，不能回落到 others_enabled。
        let c = build_http_client_scoped(&db, 0, 5, Some("  ")).await;
        assert!(c.get(url).send().await.is_err(), "blank platform extra must direct-connect");
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
