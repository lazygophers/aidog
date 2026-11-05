//! aidog:// deep link 协议层。
//!
//! 协议注册 + URL 唤起由 `tauri-plugin-deep-link` 接管：
//! - macOS：bundle 期写 `Info.plist` `CFBundleURLTypes`（scheme=aidog），运行时经
//!   `RunEvent::Opened { urls }` emit `deep-link://new-url`；
//! - Windows/Linux：启动期 `register_all()` 写注册表 / `.desktop`，URL 作为 CLI 参数
//!   启动新实例经 `handle_cli_arguments` emit 同事件。
//!
//! 本模块只做协议层：解析 URL → `{entity, action, data}` → emit `aidog-deep-link`
//! 给前端；前端按 entity 二次分发到 `aidog:<entity>`（children 各自订阅）。具体 entity
//! 的 import / 分享逻辑由 D2/D3/D4 实现。
//!
//! URL 格式：`aidog://<entity>/<action>?data=<base64>`
//! - entity = platform | mcp | skill（落在 URL authority/host 段，`aidog://platform/...`）
//! - action 默认 `import`（path 首段，预留扩展）
//! - query data = base64 编码的实体内容
//!
//! **dev 模式 macOS 注册限制**：scheme 注册在 bundle 期写 `Info.plist`，`cargo tauri dev`
//! 运行的是未打包二进制，Info.plist 不生效 → dev 下浏览器点 `aidog://` 不唤起 dev 实例。
//! 验证 deep-link 行为需跑生产构建（`cargo tauri build`），或 dev 下手动
//! `/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister`
//! 注册 `.app` 后测试。Win/Linux dev 经 `register_all()` 运行时注册，无此限制。
use serde::Serialize;
use tauri::{AppHandle, Emitter, Listener, Runtime};
use tauri_plugin_deep_link::DeepLinkExt;

/// 解析后的 deep link 载荷，emit 给前端 `aidog-deep-link` 事件。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DeepLinkPayload {
    pub entity: String,
    pub action: String,
    /// base64 编码的实体内容（URL query 原值，未 decode base64；decode 由 entity 侧做）。
    pub data: String,
}

/// 解析单个 URL 为 DeepLinkPayload。
///
/// `aidog://<entity>/<action>?data=<base64>` 经 url crate 解析后：
/// - entity 落在 **authority (host)** 段（`aidog://platform/...` → host=platform）；
/// - action 落在 path 首段（`/import` → segments[0]=import）；
/// - data 在 query。
///
/// 容错：缺 entity / 非 aidog scheme → None（调用方 log warn 后跳过，不 panic）。
/// `data` 缺省为空串（部分 action 可能不带 data，如未来 `aidog://app/open`）。
///
/// `url::Url::query_pairs` 已处理百分号编码，base64 字符集 `[A-Za-z0-9+/=]` 无需编码，
/// 但接收方若对 `+/` 做了百分号编码也能正确还原。
pub fn parse_url(url: &url::Url) -> Option<DeepLinkPayload> {
    if url.scheme() != "aidog" {
        return None;
    }
    // entity 取 host（authority）；缺 host（`aidog://?...` / `aidog:///...`）→ None。
    let entity = url.host_str()?.to_string();
    if entity.is_empty() {
        return None;
    }
    // action 取 path 首段；缺省 import（兼容 `aidog://mcp?data=...`）。多余段忽略。
    let action = url
        .path_segments()
        .and_then(|mut s| s.next())
        .map(|p| p.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "import".to_string());
    // query：取首个 data=（同字段多值取首个，base64 无歧义）。
    let data = url
        .query_pairs()
        .find_map(|(k, v)| (k == "data").then(|| v.into_owned()))
        .unwrap_or_default();
    Some(DeepLinkPayload {
        entity,
        action,
        data,
    })
}

/// 前端挂好 `aidog-deep-link` 监听后 emit 的就绪事件（`App.tsx`）。
pub const UI_READY_EVENT: &str = "aidog-ui-ready";

/// 收件箱：界面不存在或前端未就绪时 payload 先排队，就绪后补发（界面是按需新建的，
/// 创建完到 React 挂载监听之间 emit 会丢）。界面关闭即复位。
struct Inbox {
    ready: bool,
    pending: Vec<DeepLinkPayload>,
}

static INBOX: std::sync::Mutex<Inbox> = std::sync::Mutex::new(Inbox { ready: false, pending: Vec::new() });

fn inbox() -> std::sync::MutexGuard<'static, Inbox> {
    INBOX.lock().unwrap_or_else(|e| e.into_inner())
}

fn emit_payload<R: Runtime>(app: &AppHandle<R>, payload: &DeepLinkPayload) {
    if let Err(e) = app.emit("aidog-deep-link", payload) {
        tracing::warn!(error = %e, entity = %payload.entity, "deep-link: emit failed");
    }
}

/// 投递一条链接：先确保界面已打开（关窗状态下重建），前端就绪则直接 emit，否则排队。
pub fn deliver<R: Runtime>(app: &AppHandle<R>, payload: DeepLinkPayload) {
    let ready = {
        let mut ib = inbox();
        if !ib.ready {
            ib.pending.push(payload.clone());
        }
        ib.ready
    };
    crate::ui_window::open(app);
    if ready {
        emit_payload(app, &payload);
    }
}

/// 前端就绪：置位并补发排队的 payload。
pub fn mark_ready<R: Runtime>(app: &AppHandle<R>) {
    let pending = {
        let mut ib = inbox();
        ib.ready = true;
        std::mem::take(&mut ib.pending)
    };
    for p in &pending {
        emit_payload(app, p);
    }
}

/// 界面关闭：前端监听随之消失，复位就绪。
pub fn mark_closed() {
    inbox().ready = false;
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    let mut ib = inbox();
    ib.ready = false;
    ib.pending.clear();
}

/// 处理一组唤起 URL：逐个 parse → `deliver`。
///
/// 单 URL 解析失败仅 warn 跳过，不影响同批其他 URL。
pub fn dispatch_urls<R: Runtime>(app: &AppHandle<R>, urls: &[url::Url]) {
    for url in urls {
        match parse_url(url) {
            Some(payload) => {
                tracing::info!(
                    entity = %payload.entity,
                    action = %payload.action,
                    data_len = payload.data.len(),
                    "deep-link: dispatch"
                );
                deliver(app, payload);
            }
            None => tracing::warn!(url = %url, "deep-link: skipped malformed/non-aidog url"),
        }
    }
}

/// 在 setup 阶段挂上 deep-link 唤起处理。
///
/// 1. `on_open_url`：app 已运行时，URL 经 `deep-link://new-url` 事件回调（闭包捕获
///    `app.clone()` 调 `dispatch_urls`）；
/// 2. `get_current`：app 经 deep link 冷启动时，URL 在 plugin init 期已被捕获，setup
///    里取一次补发（避免 on_open_url 注册晚于事件发出导致首启漏接）；
/// 3. Win/Linux 启动期 `register_all()`：写注册表 / `.desktop`（生产构建确保注册；
///    macOS bundle 期已写 Info.plist，register_all 在 mac 返回 UnsupportedPlatform，
///    静默忽略）。
///
/// 全部失败仅 warn，不阻塞启动（deep-link 非关键路径）。
pub fn setup<R: Runtime>(app: &AppHandle<R>) {
    let app_for_ready = app.clone();
    app.listen(UI_READY_EVENT, move |_| mark_ready(&app_for_ready));

    // on_open_url：运行时唤起（app 已开）。闭包 move 捕获 app clone，事件回调里直接 dispatch。
    let app_for_callback = app.clone();
    app.deep_link().on_open_url(move |event| {
        dispatch_urls(&app_for_callback, &event.urls());
    });

    // Win/Linux 运行时注册（macOS 不支持，返回 Err 静默忽略；bundle 期已写 Info.plist）
    if let Err(e) = app.deep_link().register_all() {
        tracing::debug!(error = %e, "deep-link: register_all skipped (macOS bundle-time / unsupported)");
    }

    // 冷启动补发：URL 在 plugin init 期捕获，setup 时再取一次 emit
    match app.deep_link().get_current() {
        Ok(Some(urls)) if !urls.is_empty() => dispatch_urls(app, &urls),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "deep-link: get_current failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> url::Url {
        s.parse().expect("valid url")
    }

    #[test]
    fn parse_platform_import_with_data() {
        let p = parse_url(&url("aidog://platform/import?data=dGVzdA==")).unwrap();
        assert_eq!(
            p,
            DeepLinkPayload {
                entity: "platform".into(),
                action: "import".into(),
                data: "dGVzdA==".into(),
            }
        );
    }

    #[test]
    fn parse_action_defaults_to_import() {
        // 无 action 段：缺省 import（兼容 `aidog://mcp?data=...`）
        let p = parse_url(&url("aidog://mcp?data=ew==")).unwrap();
        assert_eq!(p.action, "import");
        assert_eq!(p.entity, "mcp");
        assert_eq!(p.data, "ew==");
    }

    #[test]
    fn parse_data_defaults_empty() {
        let p = parse_url(&url("aidog://skill/import")).unwrap();
        assert_eq!(p.data, "");
    }

    #[test]
    fn parse_empty_data_value() {
        let p = parse_url(&url("aidog://platform/import?data=")).unwrap();
        assert_eq!(p.data, "");
    }

    #[test]
    fn parse_percent_encoded_data() {
        // base64 `+/` 若被发送方百分号编码，query_pairs 应还原
        let p = parse_url(&url("aidog://platform/import?data=YQ%2B%2F")).unwrap();
        assert_eq!(p.data, "YQ+/");
    }

    #[test]
    fn parse_trailing_slash_in_entity() {
        let p = parse_url(&url("aidog://platform/?data=x")).unwrap();
        assert_eq!(p.entity, "platform");
    }

    #[test]
    fn parse_unknown_entity_still_emits() {
        // 未知 entity 不在 parse 层拒（前端按需处理 / 日志可识别）
        let p = parse_url(&url("aidog://app/open")).unwrap();
        assert_eq!(p.entity, "app");
        assert_eq!(p.action, "open");
    }

    #[test]
    fn parse_rejects_wrong_scheme() {
        assert!(parse_url(&url("https://aidog.app/platform/import?data=x")).is_none());
        assert!(parse_url(&url("http://localhost/platform")).is_none());
    }

    #[test]
    fn parse_rejects_missing_entity() {
        assert!(parse_url(&url("aidog://?data=x")).is_none());
        assert!(parse_url(&url("aidog:///import?data=x")).is_none());
    }

    #[test]
    fn parse_takes_first_data_when_duplicated() {
        let p = parse_url(&url("aidog://platform/import?data=first&data=second")).unwrap();
        assert_eq!(p.data, "first");
    }

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn payload() -> DeepLinkPayload {
        DeepLinkPayload { entity: "platform".into(), action: "import".into(), data: "x".into() }
    }

    fn counter(app: &tauri::App<tauri::test::MockRuntime>) -> Arc<AtomicUsize> {
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = n.clone();
        app.listen("aidog-deep-link", move |_| {
            n2.fetch_add(1, Ordering::SeqCst);
        });
        n
    }

    #[test]
    fn link_while_ui_closed_opens_ui_and_waits_for_ready() {
        let (_g, app) = crate::ui_window::test_setup();
        let n = counter(&app);
        deliver(app.handle(), payload());
        assert!(crate::ui_window::exists());
        assert_eq!(n.load(Ordering::SeqCst), 0, "前端未就绪不得 emit");
        mark_ready(app.handle());
        assert_eq!(n.load(Ordering::SeqCst), 1, "就绪后补发");
        mark_ready(app.handle());
        assert_eq!(n.load(Ordering::SeqCst), 1, "不重复补发");
    }

    #[test]
    fn link_while_ui_ready_emits_immediately() {
        let (_g, app) = crate::ui_window::test_setup();
        let n = counter(&app);
        crate::ui_window::open(app.handle());
        mark_ready(app.handle());
        deliver(app.handle(), payload());
        assert_eq!(n.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn closing_ui_resets_ready() {
        let (_g, app) = crate::ui_window::test_setup();
        let n = counter(&app);
        crate::ui_window::open(app.handle());
        mark_ready(app.handle());
        crate::ui_window::on_closed(app.handle());
        deliver(app.handle(), payload());
        assert_eq!(n.load(Ordering::SeqCst), 0);
        assert!(crate::ui_window::exists());
    }
}
