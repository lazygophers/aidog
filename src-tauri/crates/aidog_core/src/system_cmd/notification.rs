use crate::gateway;
use gateway::models::*;

crate::tauri_command! {
    pub async fn notification_settings_get() -> Result<NotificationSettings, String> {
    let db = aidog_ctx::db();
        let mut s = aidog_db::get_notification_settings(db).await;
        // 渠道凭据脱敏回显（票 03）：非空凭据 → 哨兵 ••••；写入侧见 notification_settings_set。
        if let Some(t) = s.channels.telegram.as_mut()
            && !t.bot_token.is_empty()
        {
            t.bot_token = MASK.to_string();
        }
        if let Some(e) = s.channels.email.as_mut()
            && !e.password.is_empty()
        {
            e.password = MASK.to_string();
        }
        Ok(s)
    }
}

/// 凭据脱敏哨兵：settings_get 回显用；settings_set 收到哨兵 = 保留旧值。
const MASK: &str = "••••";

crate::tauri_command! {
    pub async fn notification_settings_set(
        settings: NotificationSettings) -> Result<(), String> {
    let db = aidog_ctx::db();
        // 哨兵合并：凭据字段回传 •••• = 用户未改 → 保留库里旧值（脱敏回显的写侧对偶）。
        let mut settings = settings;
        let old = aidog_db::get_notification_settings(db).await;
        if let (Some(new_t), Some(old_t)) = (settings.channels.telegram.as_mut(), old.channels.telegram.as_ref())
            && new_t.bot_token == MASK
        {
            new_t.bot_token = old_t.bot_token.clone();
        }
        if let (Some(new_e), Some(old_e)) = (settings.channels.email.as_mut(), old.channels.email.as_ref())
            && new_e.password == MASK
        {
            new_e.password = old_e.password.clone();
        }
        let retention_days = settings.inbox_retention_days;
        aidog_db::set_setting(db, SetSettingInput {
            scope: "notification".to_string(),
            key: "settings".to_string(),
            value: serde_json::to_value(&settings).map_err(|e| format!("serialize notification settings: {e}"))?,
        }).await
            .map_err(|e| { tracing::error!(command = "notification_settings_set", error = %e, "persist notification settings failed"); e })?;
        // 改保留天数即时清理一次过期收件箱（非关键路径，失败仅 warn 不阻塞保存）。
        if let Err(e) = aidog_db::cleanup_notifications(db, retention_days).await {
            tracing::warn!(command = "notification_settings_set", error = %e, "cleanup notifications failed");
        }
        Ok(())
    }
}

crate::tauri_command! {
    pub async fn notification_inbox_list( limit: Option<i64>) -> Result<Vec<Notification>, String> {
    let db = aidog_ctx::db();
        aidog_db::list_notifications(db, limit.unwrap_or(100)).await
    }
}

crate::tauri_command! {
    pub async fn notification_clear() -> Result<(), String> {
    let db = aidog_ctx::db();
        aidog_db::clear_notifications(db).await
    }
}

crate::tauri_command! {
    pub async fn notification_test(
        notif_type: String,
        content: Option<String>) -> Result<aidog_notification::DispatchResult, String> {
    let db = aidog_ctx::db();
        // 应用行为 key 由 dispatch 内部统一解析（取本命令 #[instrument] span 的 trace_id，
        // 与日志同口径），无需在此手动注入；vars 仅提供模板渲染所需的展示字段。
        tracing::debug!(command = "notification_test", notif_type = %notif_type, "command invoked");
        let mut vars = std::collections::HashMap::new();
        vars.insert("project".to_string(), "aidog".to_string());
        vars.insert("status".to_string(), "test".to_string());
        vars.insert("time".to_string(), chrono::Local::now().format("%H:%M:%S").to_string());
        vars.insert("session".to_string(), "test-session".to_string());
        vars.insert("group".to_string(), "test".to_string());
        let db_arc = std::sync::Arc::new(db.clone());
        Ok(aidog_notification::dispatch(&db_arc, aidog_ctx::try_ctx(), None, &notif_type, content.as_deref(), &vars).await)
    }
}

crate::tauri_command! {
    /// 仅测 TTS 通道（绕过 dispatch，按当前 settings.tts_backend 播报 text）。
    pub async fn notification_test_tts(
        text: String) -> Result<(), String> {
    let db = aidog_ctx::db();
        let db_arc = std::sync::Arc::new(db.clone());
        let settings = aidog_db::get_notification_settings(&db_arc).await;
        aidog_notification::speak(aidog_ctx::try_ctx(), settings.tts_backend, &text);
        Ok(())
    }
}

crate::tauri_command! {
    /// 仅测系统弹窗通道（绕过 dispatch，直接调 tauri-plugin-notification）。
    pub async fn notification_test_popup(
        title: String,
        body: String,
    ) -> Result<(), String> {
        aidog_notification::show_popup(aidog_ctx::ctx(), &title, &body);
        Ok(())
    }
}

crate::tauri_command! {
    /// 仅测系统提示音通道（跨平台 spawn system beep）。
    pub async fn notification_test_beep() -> Result<(), String> {
        aidog_notification::play_beep();
        Ok(())
    }
}

#[cfg(test)]
#[path = "test_notification.rs"]
mod test_notification;

crate::tauri_command! {
    pub async fn notification_test_channel(
        channel: String) -> Result<aidog_notification::channels::DeliverySummary, String> {
    let db = aidog_ctx::db();
        let settings = aidog_db::get_notification_settings(db).await;
        let db_arc = std::sync::Arc::new(db.clone());
        aidog_notification::channels::test_channel(&db_arc, &settings.channels, &channel).await
    }
}

crate::tauri_command! {
    pub async fn notification_delivery_list(
        limit: Option<i64>) -> Result<Vec<aidog_db::models::NotificationDelivery>, String> {
    let db = aidog_ctx::db();
        aidog_db::list_notification_deliveries(db, None, limit.unwrap_or(200)).await
    }
}
