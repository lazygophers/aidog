// ProxyState 设置缓存：避免每请求 ≥4 次 db::settings 缓存读
// （RwLock read + HashMap get + Value clone + serde_json::from_value 反序列化）。
// 请求路径改为 read lock 一借获取已反序列化的 typed struct（小结构 clone 远廉价于 serde 反序列化）。
//
// 一致性：settings_set 写 DB 后由 command 层调 refresh_proxy_settings_cache 重建（禁陈旧）。
// proxy 未启动时 slot 为 None 或 weak stale → refresh 无操作。
// ponytail: 全局 Weak slot 解耦 ProxyState 生命周期与 command 层，proxy_stop 后 weak 自动失效。
use super::*;
use super::models::{MiddlewareSettings, ProxyClientSettings};

#[derive(Clone, Default)]
pub(crate) struct ProxySettingsCache {
    pub log_settings: ProxyLogSettings,
    pub lang: Lang,
    pub middleware_settings: MiddlewareSettings,
    pub system_timeout: ProxyTimeoutSettings,
    pub proxy_client: ProxyClientSettings,
}

impl ProxySettingsCache {
    pub(crate) async fn load_from(db: &Db) -> Self {
        Self {
            log_settings: get_log_settings(db).await,
            lang: get_lang(db).await,
            middleware_settings: super::db::get_middleware_settings(db).await,
            system_timeout: get_system_timeout(db).await,
            proxy_client: super::http_client::load_proxy_client_settings(db).await,
        }
    }
}

type CacheArc = Arc<tokio::sync::RwLock<ProxySettingsCache>>;

/// 全局 weak 槽：proxy 启动时 register，停止后 weak 自动 stale。
/// 用 Mutex<Option<Weak>> 而非 OnceLock：允许 stop/start 循环重新注册。
fn slot() -> &'static std::sync::Mutex<Option<std::sync::Weak<tokio::sync::RwLock<ProxySettingsCache>>>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<std::sync::Weak<tokio::sync::RwLock<ProxySettingsCache>>>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

pub(crate) fn register(cache: &CacheArc) {
    *slot().lock().unwrap() = Some(std::sync::Arc::downgrade(cache));
}

/// settings_set 写 DB 后调用：重建缓存。proxy 未启动 → no-op（weak stale）。
/// 5 次顺序 DB 读仅在用户改设置时发生（非热路径），可接受。
pub async fn refresh_proxy_settings_cache(db: &Db) {
    let arc = slot().lock().unwrap().as_ref().and_then(std::sync::Weak::upgrade);
    if let Some(arc) = arc {
        *arc.write().await = ProxySettingsCache::load_from(db).await;
    }
}
