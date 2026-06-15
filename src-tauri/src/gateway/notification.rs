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

/// 渲染 title/body：
/// - **title 字段语义 = 项目名**（取 `vars["project"]`，hook 脚本注入的 cwd basename）。
///   无 project（vars 未带或值空）→ 空字符串，前端 fallback 到类型 i18n 标签。
///   弹窗标题在 dispatch 内若 title 空则用 default_title 兜底。
/// - body 兜底链：setting.template > content > default_template (vars 含 project 时) > default_title。
///   含 project 时优先用 default_template（如「aidog 完成」）；无 project 时退化为类型默认名
///   （如「Task Complete」），避免 `{project}` 字面残留。
fn render(
    notif_type: NotifType,
    template: &str,
    content: Option<&str>,
    vars: &HashMap<String, String>,
) -> (String, String) {
    let has_project = vars
        .get("project")
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let title = vars
        .get("project")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    let raw_body = if !template.is_empty() {
        template
    } else {
        content.unwrap_or_default()
    };
    let body = if raw_body.is_empty() {
        let dt = notif_type.default_template();
        let fallback = if !dt.is_empty() && has_project {
            dt
        } else {
            default_title(notif_type)
        };
        substitute_vars(fallback, vars)
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
            }
            Err(e) => tracing::warn!(error = %e, "notify: insert inbox failed"),
        }
    }

    // 弹窗：title 空（无 project 注入）时退化到类型默认名，避免空标题弹窗
    if do_popup {
        if let Some(app) = app {
            let popup_title = if title.is_empty() {
                default_title(notif_type)
            } else {
                title.as_str()
            };
            show_popup(app, popup_title, &body);
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

/// 系统弹窗：
/// - **macOS**: `osascript -e 'display notification "..." with title "..."'`。
///   优于 tauri-plugin-notification —— 后者要求 app 已签名 + 用户授权，
///   dev/未签名场景常被吞；osascript 走系统 AppleScript，0 签名要求、直进通知中心。
/// - **其他平台**: `tauri-plugin-notification`（Windows WinRT / Linux freedesktop）。
///
/// 失败仅记日志，不阻塞调用方。
pub(crate) fn show_popup(app: &tauri::AppHandle, title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = app; // 跨平台签名一致；mac 走 osascript 不用 AppHandle
        let title = title.to_string();
        let body = body.to_string();
        std::thread::spawn(move || {
            let script = format!(
                "display notification \"{}\" with title \"{}\"",
                osascript_escape(&body),
                osascript_escape(&title),
            );
            if let Err(e) = std::process::Command::new("osascript")
                .args(["-e", &script])
                .status()
            {
                tracing::warn!(error = %e, "notify: osascript display notification failed");
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        use tauri_plugin_notification::NotificationExt;
        if let Err(e) = app.notification().builder().title(title).body(body).show() {
            tracing::warn!(error = %e, "notify: popup show failed");
        }
    }
}

/// 转义 AppleScript 字符串字面量内的 `\` 和 `"`，防止 osascript 语法注入 / 解析错误。
/// AppleScript 字符串与 C 一致：`\\` → 反斜杠，`\"` → 双引号。其他控制字符按原样
/// （换行实际会被 AppleScript 当成内容；通知中心会折行展示）。
#[cfg(any(target_os = "macos", test))]
fn osascript_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}

/// TTS 播报：按后端分发。
/// - CrossPlatform：tts crate，在独立线程构造 + speak（Tts 内含 Rc，须单线程持有；
///   speak 为非阻塞触发，线程随即退出，平台 backend 自行排播）。
/// - MacSay：std::process `say`（macOS only；其他平台退化为 CrossPlatform）。
/// - WebSpeech：emit 事件给前端 webview，由 N3 webview SpeechSynthesis 朗读。
pub(crate) fn speak(app: Option<&tauri::AppHandle>, backend: TtsBackend, text: &str) {
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
                fallback_say(&text);
            } else {
                // 给平台 backend 时间起播（speak 非阻塞；过早 drop 可能中断短句）。
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "notify: tts backend init failed, falling back to `say`");
            fallback_say(&text);
        }
    });
}

/// tts crate 失败兜底：macOS 调系统 `say` 命令；其他平台无 say，no-op。
/// tts crate macOS 后端 (AVFoundation) 在后台线程构造常返回 "Operation failed"，
/// 此兜底保证 macOS 至少有语音播报。
#[cfg(target_os = "macos")]
fn fallback_say(text: &str) {
    if let Err(e) = std::process::Command::new("say").arg(text).status() {
        tracing::warn!(error = %e, "notify: fallback `say` command failed");
    }
}

#[cfg(not(target_os = "macos"))]
fn fallback_say(_text: &str) {}

/// 跨平台系统提示音（独立通道，绕过弹窗 / TTS）。
/// - macOS: `afplay /System/Library/Sounds/Pop.aiff`
/// - Windows: PowerShell `[console]::Beep(800, 200)`
/// - Linux: `paplay <freedesktop bell>`，缺则 stdout `\a`
///
/// spawn 后立即返回；失败仅记日志，不阻塞调用方。
pub(crate) fn play_beep() {
    std::thread::spawn(|| {
        #[cfg(target_os = "macos")]
        {
            const SOUND: &str = "/System/Library/Sounds/Pop.aiff";
            if let Err(e) = std::process::Command::new("afplay").arg(SOUND).status() {
                tracing::warn!(error = %e, "notify: afplay beep failed");
            }
        }
        #[cfg(target_os = "windows")]
        {
            if let Err(e) = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", "[console]::Beep(800, 200)"])
                .status()
            {
                tracing::warn!(error = %e, "notify: powershell beep failed");
            }
        }
        #[cfg(target_os = "linux")]
        {
            const BELL: &str = "/usr/share/sounds/freedesktop/stereo/bell.oga";
            let bell_exists = std::path::Path::new(BELL).exists();
            if bell_exists {
                if let Err(e) = std::process::Command::new("paplay").arg(BELL).status() {
                    tracing::warn!(error = %e, "notify: paplay beep failed");
                }
            } else {
                // 兜底: 终端响铃
                print!("\x07");
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        }
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
    fn osascript_escape_backslash_and_quote() {
        assert_eq!(osascript_escape(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(osascript_escape("plain"), "plain");
        // 中文 + 多字节 → 原样
        assert_eq!(osascript_escape("项目 完成"), "项目 完成");
        // 嵌套引号: \"a\\b\" → 转义后 \\\"a\\\\b\\\"
        assert_eq!(osascript_escape(r#""a\b""#), r#"\"a\\b\""#);
    }

    #[test]
    fn render_template_priority() {
        let v = vars(&[("project", "aidog")]);
        // template 优先；title 现取 vars["project"]
        let (title, body) = render(NotifType::TaskComplete, "{project} 完成", Some("ignored"), &v);
        assert_eq!(title, "aidog");
        assert_eq!(body, "aidog 完成");
        // template 空 → content
        let (_, body2) = render(NotifType::Error, "", Some("构建失败 {project}"), &v);
        assert_eq!(body2, "构建失败 aidog");
        // 都空 + 有 project → default_template 兜底（custom = "{project} 通知"）
        let (_, body3) = render(NotifType::Custom, "", None, &v);
        assert_eq!(body3, "aidog 通知");
    }

    #[test]
    fn render_body_uses_default_template_when_project_present() {
        let v = vars(&[("project", "aidog")]);
        // 4 类型 default_template 兜底
        let (_, b1) = render(NotifType::TaskComplete, "", None, &v);
        assert_eq!(b1, "aidog 完成");
        let (_, b2) = render(NotifType::WaitingInput, "", None, &v);
        assert_eq!(b2, "aidog 等待用户输入");
        let (_, b3) = render(NotifType::Error, "", None, &v);
        assert_eq!(b3, "aidog 出错");
        let (_, b4) = render(NotifType::Custom, "", None, &v);
        assert_eq!(b4, "aidog 通知");
    }

    #[test]
    fn render_body_falls_back_to_default_title_when_no_project() {
        // 无 project → 退化 default_title（避免字面 `{project}`）
        let v = HashMap::new();
        assert_eq!(render(NotifType::TaskComplete, "", None, &v).1, "Task Complete");
        assert_eq!(render(NotifType::WaitingInput, "", None, &v).1, "Waiting for Input");
        assert_eq!(render(NotifType::Error, "", None, &v).1, "Error");
        assert_eq!(render(NotifType::Custom, "", None, &v).1, "Notification");

        // project 仅空白也按无 project 处理
        let v_blank = vars(&[("project", "   ")]);
        assert_eq!(render(NotifType::Error, "", None, &v_blank).1, "Error");
    }

    #[test]
    fn render_title_is_project_name() {
        // 有 project：title = project（不是 default_title）
        let v = vars(&[("project", "aidog")]);
        let (title, _) = render(NotifType::Error, "", None, &v);
        assert_eq!(title, "aidog");

        // project 含周围空白 → trim
        let v2 = vars(&[("project", "  myproj  ")]);
        let (title2, _) = render(NotifType::Custom, "", None, &v2);
        assert_eq!(title2, "myproj");
    }

    #[test]
    fn render_title_empty_when_no_project() {
        // vars 无 project → title 空字符串（前端 fallback typeLabel；弹窗 dispatch 内 fallback default_title）
        let v = HashMap::new();
        let (title, body) = render(NotifType::TaskComplete, "", None, &v);
        assert_eq!(title, "");
        assert_eq!(body, "Task Complete"); // body 兜底 default_title

        // project 值为空字符串 → 视同无
        let v2 = vars(&[("project", "")]);
        let (title2, _) = render(NotifType::Error, "", None, &v2);
        assert_eq!(title2, "");

        // project 仅空白 → trim 后空 → 视同无
        let v3 = vars(&[("project", "   ")]);
        let (title3, _) = render(NotifType::Error, "", None, &v3);
        assert_eq!(title3, "");
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
        let list = super::super::db::list_notifications(&db, 10).await.unwrap();
        assert_eq!(list.len(), 1);
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
