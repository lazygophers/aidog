//! 朗读后端选择：只断言分发选择，禁真调 `say` / tts crate（test-no-real-calls.md）。
use crate::tts::{effective_backend, speak};
use aidog_ctx::AppCtx;
use aidog_db::models::TtsBackend;
use aidog_ctx::ProxyHandle;
use aidog_db::Db;
use aidog_middleware::MiddlewareEngine;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Stub {
    ui: bool,
    via_ui: AtomicUsize,
    proxy: ProxyHandle,
}

impl AppCtx for Stub {
    fn db(&self) -> &Db {
        unreachable!("speak 不读库")
    }
    fn middleware(&self) -> &Arc<MiddlewareEngine> {
        unreachable!("speak 不用中间件")
    }
    fn proxy_handle(&self) -> &ProxyHandle {
        &self.proxy
    }
    fn emit(&self, _: &str, _: serde_json::Value) {}
    fn ui_exists(&self) -> bool {
        self.ui
    }
    fn speak_via_ui(&self, _: &str) -> bool {
        self.via_ui.fetch_add(1, Ordering::SeqCst);
        true
    }
}

#[test]
fn web_speech_without_ui_falls_back_to_system_backend() {
    let want = if cfg!(target_os = "macos") {
        TtsBackend::MacSay
    } else {
        TtsBackend::CrossPlatform
    };
    assert_eq!(effective_backend(false, TtsBackend::WebSpeech), want);
}

#[test]
fn web_speech_with_ui_stays_web_speech() {
    assert_eq!(
        effective_backend(true, TtsBackend::WebSpeech),
        TtsBackend::WebSpeech
    );
}

#[test]
fn other_backends_untouched_whether_ui_or_not() {
    for ui in [true, false] {
        assert_eq!(effective_backend(ui, TtsBackend::MacSay), TtsBackend::MacSay);
        assert_eq!(
            effective_backend(ui, TtsBackend::CrossPlatform),
            TtsBackend::CrossPlatform
        );
    }
}

#[test]
fn speak_with_ui_emits_to_frontend_once() {
    let c = Stub { ui: true, via_ui: AtomicUsize::new(0), proxy: ProxyHandle::new() };
    speak(Some(&c), TtsBackend::WebSpeech, "hi");
    assert_eq!(c.via_ui.load(Ordering::SeqCst), 1);
}
