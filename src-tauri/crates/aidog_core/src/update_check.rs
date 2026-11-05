//! 每日检查更新（内核侧）：24h 节流状态存 settings，检查本身由外壳注入（updater 插件只在桌面壳）。
//! 有新版本：界面在 → emit `EVENT`（前端弹现有更新提示）；界面不在 → 系统通知，版本号记 settings，
//! 界面下次就绪时 `flush_pending` 补发。
use aidog_ctx::AppCtx;
use aidog_i18n::{ErrorKey, t};
use std::future::Future;

pub const EVENT: &str = "aidog-update-available";
pub const THROTTLE_MS: i64 = 24 * 60 * 60 * 1000;
const PENDING_KEY: &str = "pending_update_version";

pub fn throttled(last_ms: Option<i64>, now_ms: i64) -> bool {
    // last 在未来（时钟回拨）按未节流，否则会卡到时钟追上为止。
    last_ms.is_some_and(|l| l <= now_ms && now_ms - l < THROTTLE_MS)
}

async fn put(ctx: &dyn AppCtx, key: &str, value: serde_json::Value) {
    let r = aidog_db::set_setting(
        ctx.db(),
        aidog_db::models::SetSettingInput { scope: "app".into(), key: key.into(), value },
    )
    .await;
    if let Err(e) = r {
        tracing::warn!(error = %e, key, "persist update-check setting failed");
    }
}

/// 开关开 + 未节流才调 `check`；无论成败都先写时间戳（失败不反复重试）。
/// `check` 返回 `Ok(Some(version))` = 有新版本。
pub async fn daily_check<F, Fut>(ctx: &dyn AppCtx, now_ms: i64, check: F)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Option<String>, String>>,
{
    let db = ctx.db();
    if !crate::system_cmd::auto_update::load_auto_update_enabled(db).await {
        return;
    }
    let last = match aidog_db::get_setting(db, "app", "last_update_check").await {
        Ok(Some(v)) => v.as_i64(),
        _ => None,
    };
    if throttled(last, now_ms) {
        return;
    }
    put(ctx, "last_update_check", serde_json::json!(now_ms)).await;
    match check().await {
        Ok(Some(version)) => {
            if ctx.ui_exists() {
                ctx.emit(EVENT, serde_json::json!({ "version": version }));
            } else {
                put(ctx, PENDING_KEY, serde_json::json!(version)).await;
                let lang = crate::gateway::proxy::get_lang(db).await;
                ctx.show_popup("AiDog", t(lang, ErrorKey::UpdateAvailable));
            }
        }
        Ok(None) => {}
        Err(e) => tracing::debug!(error = %e, "daily update check failed (silent)"),
    }
}

/// 界面就绪时调：有挂起的新版本就补发一次并清除。
pub async fn flush_pending(ctx: &dyn AppCtx) {
    if let Ok(Some(serde_json::Value::String(version))) =
        aidog_db::get_setting(ctx.db(), "app", PENDING_KEY).await
    {
        put(ctx, PENDING_KEY, serde_json::Value::Null).await;
        ctx.emit(EVENT, serde_json::json!({ "version": version }));
    }
}
