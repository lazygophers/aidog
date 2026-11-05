//! 内核通知：文案与「只发一次」。桩 ctx 记录 show_popup，禁真发系统通知。
use super::ui_notice::*;
use crate::proxy_cmd::proxy::{ProxyStartError, ProxyStartErrorKind};
use aidog_ctx::{AppCtx, ProxyHandle};
use aidog_db::Db;
use aidog_db::test_support::test_db;
use aidog_i18n::Lang;
use aidog_middleware::MiddlewareEngine;
use std::sync::{Arc, Mutex};

struct Stub {
    db: Db,
    mw: Arc<MiddlewareEngine>,
    proxy: ProxyHandle,
    popups: Mutex<Vec<(String, String)>>,
}

impl Stub {
    async fn new() -> Self {
        Self {
            db: test_db().await,
            mw: Arc::new(MiddlewareEngine::new()),
            proxy: ProxyHandle::new(),
            popups: Mutex::new(vec![]),
        }
    }
}

impl AppCtx for Stub {
    fn db(&self) -> &Db {
        &self.db
    }
    fn middleware(&self) -> &Arc<MiddlewareEngine> {
        &self.mw
    }
    fn proxy_handle(&self) -> &ProxyHandle {
        &self.proxy
    }
    fn emit(&self, _: &str, _: serde_json::Value) {}
    fn show_popup(&self, t: &str, b: &str) -> bool {
        self.popups.lock().unwrap().push((t.into(), b.into()));
        true
    }
}

#[test]
fn body_matches_frontend_wording_and_fills_port() {
    assert_eq!(
        proxy_start_failed_body(Lang::ZhCn, ProxyStartErrorKind::AddrInUse, 9890),
        "端口 9890 被占用，代理未启动"
    );
    assert_eq!(
        proxy_start_failed_body(Lang::EnUs, ProxyStartErrorKind::Other, 1),
        "Failed to bind port 1, proxy did not start"
    );
}

#[tokio::test]
async fn proxy_start_failed_pops_exactly_one_localized_notice() {
    let c = Stub::new().await;
    aidog_db::set_setting(
        &c.db,
        aidog_db::models::SetSettingInput {
            scope: "app".into(),
            key: "locale".into(),
            value: serde_json::json!({"locale": "zh-CN"}),
        },
    )
    .await
    .unwrap();
    let e = ProxyStartError { kind: ProxyStartErrorKind::AddrInUse, port: 9890, message: "x".into() };
    notify_proxy_start_failed(&c, &e).await;
    let p = c.popups.lock().unwrap();
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].1, "端口 9890 被占用，代理未启动");
}

#[tokio::test]
async fn first_tray_close_notifies_once_and_persists_flag() {
    let c = Stub::new().await;
    notify_first_tray_close(&c).await;
    notify_first_tray_close(&c).await;
    assert_eq!(c.popups.lock().unwrap().len(), 1, "第二次不再发");
    let flag = aidog_db::get_setting(&c.db, "app", "tray_close_notified")
        .await
        .unwrap();
    assert_eq!(flag, Some(serde_json::json!(true)));
}

#[tokio::test]
async fn first_tray_close_skipped_when_flag_already_set() {
    let c = Stub::new().await;
    aidog_db::set_setting(
        &c.db,
        aidog_db::models::SetSettingInput {
            scope: "app".into(),
            key: "tray_close_notified".into(),
            value: serde_json::json!(true),
        },
    )
    .await
    .unwrap();
    notify_first_tray_close(&c).await;
    assert!(c.popups.lock().unwrap().is_empty());
}
