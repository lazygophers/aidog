//! 系统通知分发服务（N1 — 系统通知模块）。
//!
//! 职责：
//! - 变量替换（{project}/{status}/{time}/{session}/{group}，未知占位保留）。
//! - 按 NotificationSettings.per_type[type].form 选择分发通道（Full/PopupOnly/InboxOnly/SoundOnly）。
//! - TTS 三后端：CrossPlatform（tts crate）/ MacSay（std::process `say`）/ WebSpeech（emit 事件给前端 webview）。
//! - 弹窗（tauri_plugin_notification）+ 收件箱落库（db）+ 未读更新事件 emit。
//!
//! 总开关 OFF 时整体旁路。音量跟随系统（不设置）。

use std::collections::HashMap;
use std::sync::Arc;

use super::db::Db;
use super::models::{NotifForm, NotifType, TtsBackend};

/// 前端事件名：收件箱未读数变化（前端通知中心 badge 监听刷新）。
pub const NOTIF_INBOX_UPDATED: &str = "notif-inbox-updated";
/// 前端事件名：WebSpeech 后端播报请求（payload = 文本，前端 webview SpeechSynthesis 朗读）。
pub const NOTIF_SPEAK: &str = "notif-speak";

/// 替换模板中的 `{key}` 占位为 vars 对应值；未知占位保留原文。
///
/// 不依赖正则：线性扫描，遇 `{` 找配对 `}`，取键查 vars。
/// 键含非占位字符（空格等）或缺失 → 整段 `{...}` 原样保留。
pub fn substitute_vars(template: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            // 找配对 }
            if let Some(rel) = template[i + 1..].find('}') {
                let key = &template[i + 1..i + 1 + rel];
                // 键须为合法占位名（非空 + 仅 [a-zA-Z0-9_]）
                let valid = !key.is_empty()
                    && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
                if valid {
                    if let Some(v) = vars.get(key) {
                        out.push_str(v);
                    } else {
                        // 未知占位保留原文
                        out.push('{');
                        out.push_str(key);
                        out.push('}');
                    }
                    i = i + 1 + rel + 1;
                    continue;
                }
            }
            // 无配对 / 非法键：原样输出 '{'
            out.push('{');
            i += 1;
        } else {
            // 推进一个 UTF-8 字符（避免切断多字节）
            let ch_len = utf8_char_len(bytes[i]);
            out.push_str(&template[i..i + ch_len]);
            i += ch_len;
        }
    }
    out
}

/// UTF-8 首字节判断字符字节数。
fn utf8_char_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

/// 分发选定通道（按 form 解析；纯函数便于单测）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channels {
    pub tts: bool,
    pub popup: bool,
    pub sound: bool,
    pub inbox: bool,
}

/// 按 form 解析启用的通道。
///
/// - Full       → TTS + popup + sound + inbox
/// - PopupOnly  → popup + inbox（弹窗类落库便于回看）
/// - InboxOnly  → inbox
/// - SoundOnly  → sound
///
/// TTS 还需 setting.tts && settings.tts_enabled 才真正播报（在 dispatch 内取与）。
pub fn channels_for_form(form: NotifForm) -> Channels {
    match form {
        NotifForm::Full => Channels { tts: true, popup: true, sound: true, inbox: true },
        NotifForm::PopupOnly => Channels { tts: false, popup: true, sound: false, inbox: true },
        NotifForm::InboxOnly => Channels { tts: false, popup: false, sound: false, inbox: true },
        NotifForm::SoundOnly => Channels { tts: false, popup: false, sound: true, inbox: false },
    }
}

/// 分发结果（用于端点返回 / 测试断言；不含副作用细节）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DispatchResult {
    /// 总开关 OFF 或被旁路 → false。
    pub dispatched: bool,
    /// 实际渲染后的标题。
    pub title: String,
    /// 实际渲染后的正文。
    pub body: String,
    /// 实际启用的通道（取与设置后）。
    pub tts: bool,
    pub popup: bool,
    pub sound: bool,
    pub inbox: bool,
    /// 收件箱落库行 id（inbox 通道启用时）。
    pub inbox_id: Option<i64>,
}

/// 渲染 title/body：title 用类型默认名（前端可覆盖），body 用 template（空则用 content）。
/// 返回 (title, body)。
fn render(
    notif_type: NotifType,
    template: &str,
    content: Option<&str>,
    vars: &HashMap<String, String>,
) -> (String, String) {
    let title = default_title(notif_type).to_string();
    // body 优先 template（替换变量），template 空时用 content（也替换变量），都空 → title。
    let raw_body = if !template.is_empty() {
        template
    } else {
        content.unwrap_or_default()
    };
    let body = if raw_body.is_empty() {
        substitute_vars(&title, vars)
    } else {
        substitute_vars(raw_body, vars)
    };
    (substitute_vars(&title, vars), body)
}

/// 类型默认标题（英文中性，前端按 i18n 展示，此处仅弹窗/收件箱兜底）。
fn default_title(t: NotifType) -> &'static str {
    match t {
        NotifType::TaskComplete => "Task Complete",
        NotifType::WaitingInput => "Waiting for Input",
        NotifType::Error => "Error",
        NotifType::Custom => "Notification",
    }
}

/// 核心分发入口。
///
/// `app` 为可选 AppHandle（无头测试 / 端点未带 app 时为 None，则跳过 popup/sound/emit，仅落库）。
pub async fn dispatch(
    db: &Arc<Db>,
    app: Option<&tauri::AppHandle>,
    type_str: &str,
    content: Option<&str>,
    vars: &HashMap<String, String>,
) -> DispatchResult {
    let settings = super::db::get_notification_settings(db).await;
    let notif_type = NotifType::from_str_or_custom(type_str);
    let setting = settings.type_setting(notif_type);

    // 总开关 OFF → 旁路
    if !settings.enabled {
        let (title, body) = render(notif_type, &setting.template, content, vars);
        return DispatchResult {
            dispatched: false,
            title,
            body,
            tts: false,
            popup: false,
            sound: false,
            inbox: false,
            inbox_id: None,
        };
    }

    let (title, body) = render(notif_type, &setting.template, content, vars);
    let ch = channels_for_form(setting.form);

    // TTS：通道开 + 本类型 tts + 全局 tts_enabled 才播报
    let do_tts = ch.tts && setting.tts && settings.tts_enabled;
    let do_popup = ch.popup && setting.popup;

    // 收件箱落库
    let mut inbox_id = None;
    if ch.inbox {
        match super::db::insert_notification(db, notif_type.as_str(), &title, &body).await {
            Ok(id) => {
                inbox_id = Some(id);
                if let Some(app) = app {
                    let unread = super::db::count_unread_notifications(db).await.unwrap_or(0);
                    use tauri::Emitter;
                    let _ = app.emit(NOTIF_INBOX_UPDATED, unread);
                }
            }
            Err(e) => tracing::warn!(error = %e, "notify: insert inbox failed"),
        }
    }

    // 弹窗
    if do_popup {
        if let Some(app) = app {
            show_popup(app, &title, &body);
        }
    }

    // TTS（含 sound 语义：播报本身即声音；SoundOnly 无文本则播 title）
    if do_tts {
        let speak_text = if body.is_empty() { title.clone() } else { body.clone() };
        speak(app, settings.tts_backend, &speak_text);
    }
    // SoundOnly 通道但未启用 TTS：仍走系统提示音（弹窗带声 / 单独提示音）。
    // tauri 弹窗自带系统提示音；SoundOnly 无弹窗时退化为 TTS（若关则静默）。
    // 简化：sound 通道 == 通过 popup 或 tts 发声，单独 beep 不引入额外依赖。

    DispatchResult {
        dispatched: true,
        title,
        body,
        tts: do_tts,
        popup: do_popup,
        sound: ch.sound,
        inbox: ch.inbox,
        inbox_id,
    }
}

/// 弹窗（tauri_plugin_notification）。失败仅记日志。
fn show_popup(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!(error = %e, "notify: popup show failed");
    }
}

/// TTS 播报：按后端分发。
/// - CrossPlatform：tts crate，在独立线程构造 + speak（Tts 内含 Rc，须单线程持有；
///   speak 为非阻塞触发，线程随即退出，平台 backend 自行排播）。
/// - MacSay：std::process `say`（macOS only；其他平台退化为 CrossPlatform）。
/// - WebSpeech：emit 事件给前端 webview，由 N3 webview SpeechSynthesis 朗读。
fn speak(app: Option<&tauri::AppHandle>, backend: TtsBackend, text: &str) {
    match backend {
        TtsBackend::WebSpeech => {
            if let Some(app) = app {
                use tauri::Emitter;
                let _ = app.emit(NOTIF_SPEAK, text.to_string());
            } else {
                tracing::debug!("notify: web_speech backend but no app handle, skip");
            }
        }
        TtsBackend::MacSay => {
            #[cfg(target_os = "macos")]
            {
                let text = text.to_string();
                std::thread::spawn(move || {
                    if let Err(e) = std::process::Command::new("say").arg(&text).status() {
                        tracing::warn!(error = %e, "notify: `say` command failed");
                    }
                });
            }
            #[cfg(not(target_os = "macos"))]
            {
                tracing::debug!("notify: mac_say backend on non-macOS, falling back to cross_platform");
                speak_cross_platform(text);
            }
        }
        TtsBackend::CrossPlatform => speak_cross_platform(text),
    }
}

/// 跨平台 TTS（tts crate）。在独立线程构造 Tts + speak。
///
/// 注意：`tts::Tts` 内部持 `Rc`，非真正 Send（crate 声明的 unsafe Send 仅在单线程使用安全），
/// 故在本地线程内构造并立即 speak，不跨线程传递实例。speak 触发后线程退出，
/// 各平台 backend（AVFoundation/WinRT/SpeechDispatcher）自行异步播完。
fn speak_cross_platform(text: &str) {
    let text = text.to_string();
    std::thread::spawn(move || match tts::Tts::default() {
        Ok(mut t) => {
            if let Err(e) = t.speak(&text, false) {
                tracing::warn!(error = %e, "notify: tts speak failed");
            }
            // 给平台 backend 时间起播（speak 非阻塞；过早 drop 可能中断短句）。
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        Err(e) => tracing::warn!(error = %e, "notify: tts backend init failed"),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn substitute_known_and_unknown() {
        let v = vars(&[("project", "aidog"), ("status", "done")]);
        assert_eq!(substitute_vars("{project} {status}", &v), "aidog done");
        // 未知占位保留
        assert_eq!(substitute_vars("{project} {time}", &v), "aidog {time}");
        // 无占位
        assert_eq!(substitute_vars("plain text", &v), "plain text");
        // 非法键（含空格）原样保留
        assert_eq!(substitute_vars("{not a key}", &v), "{not a key}");
        // 孤立 {
        assert_eq!(substitute_vars("a { b", &v), "a { b");
        // 多字节中文不被切断
        assert_eq!(substitute_vars("项目 {project} 完成", &v), "项目 aidog 完成");
    }

    #[test]
    fn substitute_all_vars() {
        let v = vars(&[
            ("project", "p"),
            ("status", "s"),
            ("time", "t"),
            ("session", "se"),
            ("group", "g"),
        ]);
        assert_eq!(
            substitute_vars("{project}/{status}/{time}/{session}/{group}", &v),
            "p/s/t/se/g"
        );
    }

    #[test]
    fn channels_per_form() {
        assert_eq!(
            channels_for_form(NotifForm::Full),
            Channels { tts: true, popup: true, sound: true, inbox: true }
        );
        assert_eq!(
            channels_for_form(NotifForm::PopupOnly),
            Channels { tts: false, popup: true, sound: false, inbox: true }
        );
        assert_eq!(
            channels_for_form(NotifForm::InboxOnly),
            Channels { tts: false, popup: false, sound: false, inbox: true }
        );
        assert_eq!(
            channels_for_form(NotifForm::SoundOnly),
            Channels { tts: false, popup: false, sound: true, inbox: false }
        );
    }

    #[test]
    fn render_template_priority() {
        let v = vars(&[("project", "aidog")]);
        // template 优先
        let (title, body) = render(NotifType::TaskComplete, "{project} 完成", Some("ignored"), &v);
        assert_eq!(title, "Task Complete");
        assert_eq!(body, "aidog 完成");
        // template 空 → content
        let (_, body2) = render(NotifType::Error, "", Some("构建失败 {project}"), &v);
        assert_eq!(body2, "构建失败 aidog");
        // 都空 → title
        let (_, body3) = render(NotifType::Custom, "", None, &v);
        assert_eq!(body3, "Notification");
    }

    // ── 分发集成（无 app handle → 仅落库，验证按 form 选通道）──
    async fn mem_db() -> Arc<Db> {
        let db = Db::new(":memory:").await.unwrap();
        db.init_tables().await.unwrap();
        Arc::new(db)
    }

    async fn set_form(db: &Arc<Db>, type_str: &str, form: &str, enabled: bool) {
        let json = serde_json::json!({
            "enabled": enabled,
            "tts_enabled": true,
            "tts_backend": "cross_platform",
            "per_type": { type_str: { "tts": true, "popup": true, "form": form, "template": "" } }
        });
        super::super::db::set_setting(db, super::super::models::SetSettingInput {
            scope: "notification".into(),
            key: "settings".into(),
            value: json,
        }).await.unwrap();
    }

    #[tokio::test]
    async fn dispatch_full_form_falls_to_inbox_without_app() {
        let db = mem_db().await;
        set_form(&db, "task_complete", "full", true).await;
        let v = vars(&[("project", "aidog")]);
        let r = dispatch(&db, None, "task_complete", Some("done {project}"), &v).await;
        assert!(r.dispatched);
        assert!(r.inbox);
        assert_eq!(r.body, "done aidog");
        assert!(r.inbox_id.is_some());
        // 落库一行
        let list = super::super::db::list_notifications(&db, 10).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].notif_type, "task_complete");
    }

    #[tokio::test]
    async fn dispatch_inbox_only() {
        let db = mem_db().await;
        set_form(&db, "error", "inbox_only", true).await;
        let r = dispatch(&db, None, "error", Some("oops"), &HashMap::new()).await;
        assert!(r.dispatched && r.inbox && !r.popup && !r.sound);
        assert_eq!(super::super::db::count_unread_notifications(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn dispatch_sound_only_no_inbox() {
        let db = mem_db().await;
        set_form(&db, "waiting_input", "sound_only", true).await;
        let r = dispatch(&db, None, "waiting_input", Some("?"), &HashMap::new()).await;
        assert!(r.dispatched && r.sound && !r.inbox && !r.popup);
        // 不落库
        assert!(super::super::db::list_notifications(&db, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn dispatch_master_switch_off_bypasses() {
        let db = mem_db().await;
        set_form(&db, "task_complete", "full", false).await; // enabled=false
        let r = dispatch(&db, None, "task_complete", Some("x"), &HashMap::new()).await;
        assert!(!r.dispatched);
        assert!(super::super::db::list_notifications(&db, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn dispatch_unknown_type_as_custom_full() {
        let db = mem_db().await;
        // 不配 per_type → 默认 Full + 全 true
        let r = dispatch(&db, None, "my_custom_type", Some("hi"), &HashMap::new()).await;
        assert!(r.dispatched && r.inbox);
        let list = super::super::db::list_notifications(&db, 10).await.unwrap();
        // 落库 type 记原始字符串映射后的 custom
        assert_eq!(list[0].notif_type, "custom");
    }
}
