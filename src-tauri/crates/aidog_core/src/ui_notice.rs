//! 界面不在场时由内核直接发的系统通知：代理启动失败、首次进入托盘常驻。
//! 文案取 `aidog_i18n`；发送走 `AppCtx::show_popup`（测试注入桩，禁真发）。

use aidog_ctx::AppCtx;
use aidog_i18n::{ErrorKey, Lang, t};

use crate::proxy_cmd::proxy::{ProxyStartError, ProxyStartErrorKind};

const TITLE: &str = "AiDog";

pub fn proxy_start_failed_body(lang: Lang, kind: ProxyStartErrorKind, port: u16) -> String {
    let key = match kind {
        ProxyStartErrorKind::AddrInUse => ErrorKey::ProxyStartPortInUse,
        ProxyStartErrorKind::Other => ErrorKey::ProxyStartOther,
    };
    t(lang, key).replace("{{port}}", &port.to_string())
}

/// 代理启动失败：内核直接发系统通知（前端不再监听转发，界面开着也只此一条）。
pub async fn notify_proxy_start_failed(ctx: &dyn AppCtx, e: &ProxyStartError) {
    let lang = crate::gateway::proxy::get_lang(ctx.db()).await;
    ctx.show_popup(TITLE, &proxy_start_failed_body(lang, e.kind, e.port));
}

/// 首次进入托盘常驻发一次「仍在托盘运行」，标记持久化在 settings（app/tray_close_notified）。
pub async fn notify_first_tray_close(ctx: &dyn AppCtx) {
    let db = ctx.db();
    if let Ok(Some(serde_json::Value::Bool(true))) =
        aidog_db::get_setting(db, "app", "tray_close_notified").await
    {
        return;
    }
    // 写标记失败仍发通知（宁可下次再提示一次，也不漏掉这条）。
    if let Err(e) = aidog_db::set_setting(
        db,
        aidog_db::models::SetSettingInput {
            scope: "app".into(),
            key: "tray_close_notified".into(),
            value: serde_json::json!(true),
        },
    )
    .await
    {
        tracing::warn!(error = %e, "persist tray_close_notified failed");
    }
    let lang = crate::gateway::proxy::get_lang(db).await;
    ctx.show_popup(TITLE, t(lang, ErrorKey::TrayStillRunning));
}
