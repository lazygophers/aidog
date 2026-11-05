//! 每日检查更新：节流纯函数 + 编排。桩 ctx 记录 emit / show_popup，检查由闭包注入，禁真连更新服务器。
use super::update_check::*;
use aidog_ctx::{AppCtx, ProxyHandle};
use aidog_db::Db;
use aidog_db::test_support::test_db;
use aidog_middleware::MiddlewareEngine;
use std::sync::{Arc, Mutex};

struct Stub {
    db: Db,
    mw: Arc<MiddlewareEngine>,
    proxy: ProxyHandle,
    ui: bool,
    popups: Mutex<Vec<String>>,
    events: Mutex<Vec<(String, serde_json::Value)>>,
}

impl Stub {
    async fn new(ui: bool) -> Self {
        Self {
            db: test_db().await,
            mw: Arc::new(MiddlewareEngine::new()),
            proxy: ProxyHandle::new(),
            ui,
            popups: Mutex::new(vec![]),
            events: Mutex::new(vec![]),
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
    fn emit(&self, e: &str, p: serde_json::Value) {
        self.events.lock().unwrap().push((e.into(), p));
    }
    fn show_popup(&self, _t: &str, b: &str) -> bool {
        self.popups.lock().unwrap().push(b.into());
        true
    }
    fn ui_exists(&self) -> bool {
        self.ui
    }
}

const NOW: i64 = 1_800_000_000_000;

async fn set(c: &Stub, key: &str, v: serde_json::Value) {
    aidog_db::set_setting(
        &c.db,
        aidog_db::models::SetSettingInput { scope: "app".into(), key: key.into(), value: v },
    )
    .await
    .unwrap();
}

async fn last(c: &Stub) -> Option<serde_json::Value> {
    aidog_db::get_setting(&c.db, "app", "last_update_check").await.unwrap()
}

#[test]
fn throttle_window_is_24h() {
    assert!(!throttled(None, NOW), "从未检查");
    assert!(throttled(Some(NOW - 3_600_000), NOW));
    assert!(throttled(Some(NOW - THROTTLE_MS + 1), NOW));
    assert!(!throttled(Some(NOW - THROTTLE_MS), NOW));
    assert!(!throttled(Some(NOW + 1_000_000), NOW), "时钟回拨不卡死");
}

#[tokio::test]
async fn disabled_switch_skips_check() {
    let c = Stub::new(true).await;
    set(&c, "auto_update_enabled", serde_json::json!(false)).await;
    let called = Mutex::new(false);
    daily_check(&c, NOW, || async { *called.lock().unwrap() = true; Ok(None) }).await;
    assert!(!*called.lock().unwrap());
    assert_eq!(last(&c).await, None);
}

#[tokio::test]
async fn throttled_skips_and_due_runs_and_persists_now() {
    let c = Stub::new(true).await;
    set(&c, "last_update_check", serde_json::json!(NOW - 1000)).await;
    let n = Mutex::new(0);
    daily_check(&c, NOW, || async { *n.lock().unwrap() += 1; Ok(None) }).await;
    assert_eq!(*n.lock().unwrap(), 0);
    daily_check(&c, NOW + THROTTLE_MS, || async { *n.lock().unwrap() += 1; Ok(None) }).await;
    assert_eq!(*n.lock().unwrap(), 1);
    assert_eq!(last(&c).await, Some(serde_json::json!(NOW + THROTTLE_MS)));
}

#[tokio::test]
async fn failed_check_still_stamps_and_stays_silent() {
    let c = Stub::new(true).await;
    daily_check(&c, NOW, || async { Err("offline".to_string()) }).await;
    assert_eq!(last(&c).await, Some(serde_json::json!(NOW)));
    assert!(c.events.lock().unwrap().is_empty() && c.popups.lock().unwrap().is_empty());
}

#[tokio::test]
async fn update_with_ui_emits_event_only() {
    let c = Stub::new(true).await;
    daily_check(&c, NOW, || async { Ok(Some("9.9.9".to_string())) }).await;
    let ev = c.events.lock().unwrap();
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].0, EVENT);
    assert_eq!(ev[0].1["version"], "9.9.9");
    assert!(c.popups.lock().unwrap().is_empty());
}

#[tokio::test]
async fn update_without_ui_notifies_then_flush_emits_once() {
    let c = Stub::new(false).await;
    set(&c, "locale", serde_json::json!({"locale": "zh-CN"})).await;
    daily_check(&c, NOW, || async { Ok(Some("9.9.9".to_string())) }).await;
    assert_eq!(*c.popups.lock().unwrap(), vec!["aidog 有新版本".to_string()]);
    assert!(c.events.lock().unwrap().is_empty());
    flush_pending(&c).await;
    flush_pending(&c).await;
    let ev = c.events.lock().unwrap();
    assert_eq!(ev.len(), 1, "补发一次后清除");
    assert_eq!(ev[0].1["version"], "9.9.9");
}

#[tokio::test]
async fn no_update_is_silent() {
    let c = Stub::new(false).await;
    daily_check(&c, NOW, || async { Ok(None) }).await;
    flush_pending(&c).await;
    assert!(c.events.lock().unwrap().is_empty() && c.popups.lock().unwrap().is_empty());
}
