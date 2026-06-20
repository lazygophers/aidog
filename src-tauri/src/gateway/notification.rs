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
use super::models::{default_template_for_event, NotifForm, NotifType, TtsBackend};

/// 前端事件名：WebSpeech 后端播报请求（payload = 文本，前端 webview SpeechSynthesis 朗读）。
pub const NOTIF_SPEAK: &str = "notif-speak";

/// 替换模板中的 `{key}` 占位为 vars 对应值；未知占位保留原文。
///
/// 不依赖正则：线性扫描，遇 `{` 找配对 `}`，取键查 vars。
/// 键含非占位字符（空格等）或缺失 → 整段 `{...}` 原样保留。
pub fn substitute_vars(template: &str, vars: &HashMap<String, String>) -> String {
    substitute_vars_impl(template, vars, false)
}

/// 同 `substitute_vars`，但缺失占位 → **替换为空串**（不保留 `{x}` 字面）。
///
/// 用于 event 路径：每事件默认模板用其专属入参（`{tool_name}` 等），但脚本通用透传时
/// 该事件实际 stdin 可能缺该可选字段；为避免残留裸 `{x}` 难看，event 路径采用「缺失→空串」。
/// type 路径仍用 `substitute_vars`（保留未知占位，与历史一致）。
pub fn substitute_vars_fill_empty(template: &str, vars: &HashMap<String, String>) -> String {
    substitute_vars_impl(template, vars, true)
}

/// 占位替换核心。`fill_empty=true` 时缺失/未知占位替换为空串，否则保留原文。
fn substitute_vars_impl(template: &str, vars: &HashMap<String, String>, fill_empty: bool) -> String {
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
                    } else if !fill_empty {
                        // 未知占位保留原文
                        out.push('{');
                        out.push_str(key);
                        out.push('}');
                    }
                    // fill_empty 且缺失 → 不输出任何内容（替换为空串）
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

/// 无 project 时，default_template 渲染用的品牌兜底名（避免 `{project}` 字面泄漏）。
/// render 为纯函数无 AppHandle，用常量最简且稳定（不取 product_name）。
const BRAND_FALLBACK: &str = "aidog";

/// 渲染 title/body：
/// - **title 字段语义 = 项目名**（取 `vars["project"]`，hook 脚本注入的 cwd basename）。
///   无 project（vars 未带或值空）→ 空字符串，前端 fallback 到类型 i18n 标签。
///   弹窗标题在 dispatch 内若 title 空则用 default_title 兜底。
/// - body 兜底链：setting.template > content > default_template > default_title（末位兜底）。
///   template+content 都空时无论有无 project 都渲染 default_template；无 project 时给 `{project}`
///   注入品牌兜底名（`aidog`），故得「aidog 完成」而非空串 / `{project}` 字面 / 英文退化。
fn render(
    notif_type: NotifType,
    template: &str,
    content: Option<&str>,
    vars: &HashMap<String, String>,
) -> (String, String) {
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
        if dt.is_empty() {
            // default_template 理论空时最末兜底，避免空串。
            substitute_vars(default_title(notif_type), vars)
        } else {
            // 无 project 时注入品牌兜底名，杜绝 `{project}` 字面残留。
            let has_project = vars
                .get("project")
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if has_project {
                substitute_vars(dt, vars)
            } else {
                let mut vars2 = vars.clone();
                vars2.insert("project".to_string(), BRAND_FALLBACK.to_string());
                substitute_vars(dt, &vars2)
            }
        }
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
    }
}

/// 核心分发入口。
///
/// `app` 为可选 AppHandle（无头测试 / 端点未带 app 时为 None，则跳过 popup/sound/emit，仅落库）。
///
/// `event`（N2 hook 事件通知 — 逐事件自含路径）：
/// - 存在且 `settings.per_event[event]` 命中且 `enabled` → **完全自含**，不经 notif_type/per_type/form：
///   - 通道：`do_tts = settings.tts_enabled && es.tts`；`do_popup = es.popup`；`do_sound = es.sound`
///     （独立 `play_beep`，不再跟随 popup）；inbox **恒落库**（历史）。
///   - body：`es.template` 非空用之，否则 `default_template_for_event(event)`，再否则类型 default 兜底（防空）。
///     event 路径占位用 `substitute_vars_fill_empty`（缺失专属字段 → 空串，不残留裸 `{x}`）。
///   - 标题：`vars["project"]` 非空用之，否则事件名，否则 "Notification"。
/// - 不存在 / 未命中 / 未启用 / 无 event（Codex / 裸 type）→ 维持现有按 `type_str` 类型路径
///   （向后兼容，不破坏 Codex；未知 type → TaskComplete）。
pub async fn dispatch(
    db: &Arc<Db>,
    app: Option<&tauri::AppHandle>,
    event: Option<&str>,
    type_str: &str,
    content: Option<&str>,
    vars: &HashMap<String, String>,
) -> DispatchResult {
    let settings = super::db::get_notification_settings(db).await;

    // event 路径：命中 per_event 且 enabled → 自含分发。
    if let Some(es) = event
        .and_then(|e| settings.event_setting(e))
        .filter(|es| es.enabled)
    {
        let event_name = event.unwrap_or_default();

        // body 模板：es.template 非空 > default_template_for_event > 类型 default（防空兜底）。
        let raw_body = if !es.template.trim().is_empty() {
            es.template.clone()
        } else {
            let dft = default_template_for_event(event_name);
            if !dft.is_empty() {
                dft.to_string()
            } else {
                NotifType::TaskComplete.default_template().to_string()
            }
        };
        // 缺失专属字段 → 空串（不残留裸占位）；无 project 注入品牌兜底。
        let mut bvars = vars.clone();
        if bvars.get("project").map(|s| s.trim().is_empty()).unwrap_or(true) {
            bvars.insert("project".to_string(), BRAND_FALLBACK.to_string());
        }
        let body = substitute_vars_fill_empty(&raw_body, &bvars);

        // 标题：vars.project 非空 > 事件名 > "Notification"。
        let title = vars
            .get("project")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| (!event_name.is_empty()).then(|| event_name.to_string()))
            .unwrap_or_else(|| "Notification".to_string());

        // 总开关 OFF → 旁路。
        if !settings.enabled {
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

        let do_tts = settings.tts_enabled && es.tts;
        let do_popup = es.popup;
        let do_sound = es.sound;

        // inbox 恒落库（历史）。event 路径 inbox 用事件名作 notif_type 列（便于回看来源）。
        let mut inbox_id = None;
        match super::db::insert_notification(db, event_name, &title, &body).await {
            Ok(id) => inbox_id = Some(id),
            Err(e) => tracing::warn!(error = %e, "notify: insert inbox failed"),
        }

        if do_popup {
            if let Some(app) = app {
                show_popup(app, &title, &body);
            }
        }
        if do_tts {
            let speak_text = if body.is_empty() { title.clone() } else { body.clone() };
            speak(app, settings.tts_backend, &speak_text);
        }
        // event 路径 sound 为独立开关（不再跟随 popup）。无头测试 app=None 时跳过实际播音。
        if do_sound && app.is_some() {
            play_beep();
        }

        return DispatchResult {
            dispatched: true,
            title,
            body,
            tts: do_tts,
            popup: do_popup,
            sound: do_sound,
            inbox: true,
            inbox_id,
        };
    }

    // CC hook 事件来源守卫：通用 hook 脚本 POST 时带 event、不带 type（见 hooks.rs「不传 type」），
    // 当 event 存在但未在 per_event 启用时，禁止回退类型路径（type_str="" → from_str_or_default → TaskComplete）
    // 误派通知。仅 event=None（Codex）或携带显式 type 的旧/Codex 路径才走下方类型路径。
    if let Some(ev) = event.filter(|e| !e.is_empty()) {
        if type_str.is_empty() {
            // 未启用事件：不入库、不触发任何通道。title/body 仅填充返回结构，不产生任何副作用。
            return DispatchResult {
                dispatched: false,
                title: ev.to_string(),
                body: default_template_for_event(ev).to_string(),
                tts: false,
                popup: false,
                sound: false,
                inbox: false,
                inbox_id: None,
            };
        }
    }

    // 类型路径（无 event / 未命中 / 未启用）：向后兼容 + Codex。
    let notif_type = NotifType::from_str_or_default(type_str);
    let setting = settings.type_setting(notif_type);
    let template = setting.template.as_str();

    // 总开关 OFF → 旁路
    if !settings.enabled {
        let (title, body) = render(notif_type, template, content, vars);
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

    let (title, body) = render(notif_type, template, content, vars);
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

/// 系统弹窗：全平台统一走 `tauri-plugin-notification`
/// （macOS UserNotifications / Windows WinRT toast / Linux freedesktop）。
///
/// 注意（macOS）：插件走 UserNotifications framework，要求 app **已签名 + 用户授权**，
/// dev / 未签名场景可能被系统静默吞掉（看不到弹窗）。打包发布版需正确签名才稳定弹出。
///
/// 治本：macOS 打包 **必须签名 + 公证**（codesign + notarytool），bundle 注册后系统
/// 才会默认授予通知权限并稳定投递；缺签名/公证时即使用户未显式拒绝，通知也可能被静默
/// 丢弃。运行期补救见前端通知设置页「打开系统通知设置」引导按钮（仅 macOS），以及
/// 启动时的 request_permission（lib.rs setup）。
///
/// 失败仅记日志，不阻塞调用方。
pub(crate) fn show_popup(app: &tauri::AppHandle, title: &str, body: &str) {
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
    fn render_template_priority() {
        let v = vars(&[("project", "aidog")]);
        // template 优先；title 现取 vars["project"]
        let (title, body) = render(NotifType::TaskComplete, "{project} 完成", Some("ignored"), &v);
        assert_eq!(title, "aidog");
        assert_eq!(body, "aidog 完成");
        // template 空 → content
        let (_, body2) = render(NotifType::Error, "", Some("构建失败 {project}"), &v);
        assert_eq!(body2, "构建失败 aidog");
        // 都空 + 有 project → default_template 兜底
        let (_, body3) = render(NotifType::WaitingInput, "", None, &v);
        assert_eq!(body3, "aidog 等待用户输入");
    }

    #[test]
    fn render_body_uses_default_template_when_project_present() {
        let v = vars(&[("project", "aidog")]);
        // 3 类型 default_template 兜底
        let (_, b1) = render(NotifType::TaskComplete, "", None, &v);
        assert_eq!(b1, "aidog 完成");
        let (_, b2) = render(NotifType::WaitingInput, "", None, &v);
        assert_eq!(b2, "aidog 等待用户输入");
        let (_, b3) = render(NotifType::Error, "", None, &v);
        assert_eq!(b3, "aidog 出错");
    }

    #[test]
    fn render_body_uses_default_template_with_brand_fallback_when_no_project() {
        // 无 project 空模板 → 仍渲染 default_template，`{project}` 用品牌兜底 "aidog"
        // 核心新行为：不再退化到英文 default_title，也不泄漏 `{project}` 字面。
        let v = HashMap::new();
        for (t, expect) in [
            (NotifType::TaskComplete, "aidog 完成"),
            (NotifType::WaitingInput, "aidog 等待用户输入"),
            (NotifType::Error, "aidog 出错"),
        ] {
            let body = render(t, "", None, &v).1;
            assert_eq!(body, expect);
            assert!(!body.contains("{project}"), "残留 {{project}} 字面: {body}");
            assert_ne!(body, default_title(t), "不应退化到英文 default_title");
        }

        // project 仅空白也按无 project 处理 → 品牌兜底
        let v_blank = vars(&[("project", "   ")]);
        assert_eq!(render(NotifType::Error, "", None, &v_blank).1, "aidog 出错");
    }

    #[test]
    fn render_title_is_project_name() {
        // 有 project：title = project（不是 default_title）
        let v = vars(&[("project", "aidog")]);
        let (title, _) = render(NotifType::Error, "", None, &v);
        assert_eq!(title, "aidog");

        // project 含周围空白 → trim
        let v2 = vars(&[("project", "  myproj  ")]);
        let (title2, _) = render(NotifType::WaitingInput, "", None, &v2);
        assert_eq!(title2, "myproj");
    }

    #[test]
    fn render_title_empty_when_no_project() {
        // vars 无 project → title 空字符串（前端 fallback typeLabel；弹窗 dispatch 内 fallback default_title）
        let v = HashMap::new();
        let (title, body) = render(NotifType::TaskComplete, "", None, &v);
        assert_eq!(title, "");
        assert_eq!(body, "aidog 完成"); // body 用 default_template + 品牌兜底

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
        let r = dispatch(&db, None, None, "task_complete", Some("done {project}"), &v).await;
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
        let r = dispatch(&db, None, None, "error", Some("oops"), &HashMap::new()).await;
        assert!(r.dispatched && r.inbox && !r.popup && !r.sound);
        let list = super::super::db::list_notifications(&db, 10).await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn dispatch_sound_only_no_inbox() {
        let db = mem_db().await;
        set_form(&db, "waiting_input", "sound_only", true).await;
        let r = dispatch(&db, None, None, "waiting_input", Some("?"), &HashMap::new()).await;
        assert!(r.dispatched && r.sound && !r.inbox && !r.popup);
        // 不落库
        assert!(super::super::db::list_notifications(&db, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn dispatch_master_switch_off_bypasses() {
        let db = mem_db().await;
        set_form(&db, "task_complete", "full", false).await; // enabled=false
        let r = dispatch(&db, None, None, "task_complete", Some("x"), &HashMap::new()).await;
        assert!(!r.dispatched);
        assert!(super::super::db::list_notifications(&db, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn dispatch_unknown_type_as_task_complete() {
        let db = mem_db().await;
        // 不配 per_type → 默认 Full + 全 true
        let r = dispatch(&db, None, None, "my_custom_type", Some("hi"), &HashMap::new()).await;
        assert!(r.dispatched && r.inbox);
        let list = super::super::db::list_notifications(&db, 10).await.unwrap();
        // 未知 type 兜底到 task_complete（通知不丢）
        assert_eq!(list[0].notif_type, "task_complete");
    }

    // ── N2 hook 事件解析（per_event）──
    async fn set_notif_settings(db: &Arc<Db>, value: serde_json::Value) {
        super::super::db::set_setting(db, super::super::models::SetSettingInput {
            scope: "notification".into(),
            key: "settings".into(),
            value,
        }).await.unwrap();
    }

    #[tokio::test]
    async fn dispatch_event_uses_custom_template_and_direct_channels() {
        let db = mem_db().await;
        // SubagentStop 启用 + 自定义文案 {agent_type}；tts/popup/sound 全关 → 仅 inbox。
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "tts_enabled": true,
            "tts_backend": "cross_platform",
            "per_event": {
                "SubagentStop": { "enabled": true, "tts": false, "popup": false, "sound": false, "template": "{project} 子代理 {agent_type} 结束" }
            }
        })).await;
        let v = vars(&[("project", "aidog"), ("agent_type", "reviewer")]);
        let r = dispatch(&db, None, Some("SubagentStop"), "ignored_type", None, &v).await;
        assert!(r.dispatched);
        // 通道直接取 EventSetting：tts/popup/sound 都关 → 仅 inbox（恒落库）。
        assert!(r.inbox && !r.popup && !r.sound && !r.tts);
        assert_eq!(r.body, "aidog 子代理 reviewer 结束");
        let list = super::super::db::list_notifications(&db, 10).await.unwrap();
        // inbox 列用事件名（来源标识）。
        assert_eq!(list[0].notif_type, "SubagentStop");
    }

    #[tokio::test]
    async fn dispatch_event_tts_popup_directly_controlled() {
        let db = mem_db().await;
        // popup 开、tts 开（全局 tts_enabled 开）→ tts/popup 直控生效，sound 默认 true（向后兼容）。
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "tts_enabled": true,
            "per_event": { "Stop": { "enabled": true, "tts": true, "popup": true } }
        })).await;
        let v = vars(&[("project", "aidog")]);
        // 无 app handle → popup/tts/sound 不实际触发，但 DispatchResult 标志反映决策。
        // per_event[Stop] 未给 sound → serde default_true → r.sound 仍为 true（旧配置向后兼容）。
        let r = dispatch(&db, None, Some("Stop"), "", None, &v).await;
        assert!(r.dispatched && r.tts && r.popup && r.sound && r.inbox);
        // 全局 tts_enabled 关 → do_tts 必关，即使 es.tts 开。
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "tts_enabled": false,
            "per_event": { "Stop": { "enabled": true, "tts": true, "popup": true } }
        })).await;
        let r2 = dispatch(&db, None, Some("Stop"), "", None, &v).await;
        assert!(!r2.tts && r2.popup);
    }

    #[tokio::test]
    async fn dispatch_event_sound_independent_of_popup() {
        let db = mem_db().await;
        let v = vars(&[("project", "aidog")]);
        // sound 开、popup 关 → sound 独立于 popup（不再跟随）。
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "per_event": { "Stop": { "enabled": true, "popup": false, "sound": true } }
        })).await;
        let r = dispatch(&db, None, Some("Stop"), "", None, &v).await;
        assert!(!r.popup && r.sound, "sound 应独立于 popup: {:?}", r);
        // sound 关、popup 开 → sound 关，不跟随 popup。
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "per_event": { "Stop": { "enabled": true, "popup": true, "sound": false } }
        })).await;
        let r2 = dispatch(&db, None, Some("Stop"), "", None, &v).await;
        assert!(r2.popup && !r2.sound, "sound 关时不应跟随 popup: {:?}", r2);
    }

    #[test]
    fn event_setting_sound_backward_compat_defaults_true() {
        // 旧 per_event 配置无 sound 字段 → 反序列化默认 true（向后兼容）。
        let es: crate::gateway::models::EventSetting =
            serde_json::from_value(serde_json::json!({ "enabled": true })).unwrap();
        assert!(es.sound, "旧配置无 sound 应默认 true");
        // roundtrip 保留 sound。
        let es2 = crate::gateway::models::EventSetting { sound: false, ..es };
        let json = serde_json::to_string(&es2).unwrap();
        let back: crate::gateway::models::EventSetting = serde_json::from_str(&json).unwrap();
        assert!(!back.sound, "roundtrip 应保留 sound=false");
    }

    #[tokio::test]
    async fn dispatch_event_empty_template_falls_back_to_event_default() {
        let db = mem_db().await;
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "per_event": { "PermissionRequest": { "enabled": true, "template": "" } }
        })).await;
        let v = vars(&[("project", "aidog"), ("tool_name", "Bash")]);
        let r = dispatch(&db, None, Some("PermissionRequest"), "ignored", None, &v).await;
        // template 空 → default_template_for_event(PermissionRequest)，用专属入参 {tool_name}。
        assert_eq!(r.body, "aidog 请求授权：Bash");
    }

    #[tokio::test]
    async fn dispatch_event_missing_field_filled_empty_no_residual_placeholder() {
        let db = mem_db().await;
        // PermissionRequest 默认模板含 {tool_name}，但本次 vars 未提供 → 缺失替换为空串，
        // 不残留裸 {tool_name}（event 路径 fill_empty 策略）。
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "per_event": { "PermissionRequest": { "enabled": true } }
        })).await;
        let v = vars(&[("project", "aidog")]);
        let r = dispatch(&db, None, Some("PermissionRequest"), "", None, &v).await;
        assert!(!r.body.contains("{tool_name}"), "残留裸占位: {}", r.body);
        assert_eq!(r.body, "aidog 请求授权：");
    }

    #[tokio::test]
    async fn dispatch_event_not_enabled_falls_to_type_path() {
        let db = mem_db().await;
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "per_type": { "task_complete": { "form": "inbox_only", "template": "{project} 完成" } },
            "per_event": { "Stop": { "enabled": false, "template": "x" } }
        })).await;
        let v = vars(&[("project", "aidog")]);
        // 事件未启用 → 走 type_str 类型路径（task_complete），向后兼容/Codex 不破坏。
        let r = dispatch(&db, None, Some("Stop"), "task_complete", None, &v).await;
        assert_eq!(r.body, "aidog 完成");
        let list = super::super::db::list_notifications(&db, 10).await.unwrap();
        assert_eq!(list[0].notif_type, "task_complete");
    }

    #[tokio::test]
    async fn dispatch_no_event_uses_type_path_codex_regression() {
        let db = mem_db().await;
        // Codex 路径：complete 脚本 POST type=task_complete，无 event → 走类型路径不受影响。
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "per_type": { "task_complete": { "form": "full", "template": "{project} 完成" } },
            "per_event": { "Stop": { "enabled": true, "template": "事件路径不应命中" } }
        })).await;
        let v = vars(&[("project", "aidog")]);
        // event=None（Codex 不传 event）→ 类型路径，per_event[Stop] 不命中。
        let r = dispatch(&db, None, None, "task_complete", None, &v).await;
        assert!(r.dispatched);
        assert_eq!(r.body, "aidog 完成");
        let list = super::super::db::list_notifications(&db, 10).await.unwrap();
        assert_eq!(list[0].notif_type, "task_complete");
    }

    #[tokio::test]
    async fn dispatch_event_present_empty_type_unenabled_suppressed() {
        let db = mem_db().await;
        // 通用 CC hook 脚本：带 event、不带 type（type_str=""）。仅启用 Stop，未启用 SubagentStop。
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "per_type": { "task_complete": { "form": "full", "template": "{project} 完成" } },
            "per_event": { "Stop": { "enabled": true } }
        })).await;
        let v = vars(&[("project", "aidog")]);
        // SubagentStop 未在 per_event 启用 + type_str="" → 守卫抑制，不回退类型路径误派。
        let r = dispatch(&db, None, Some("SubagentStop"), "", None, &v).await;
        assert!(!r.dispatched, "未启用事件不应派发");
        assert!(!r.inbox, "未启用事件不应入库");
        let list = super::super::db::list_notifications(&db, 10).await.unwrap();
        assert_eq!(list.len(), 0, "不应有任何入库记录");
    }

    #[tokio::test]
    async fn dispatch_event_explicit_disabled_empty_type_suppressed() {
        let db = mem_db().await;
        // SubagentStop 显式 enabled=false，同样被守卫抑制（type_str="" → 不回退类型路径）。
        set_notif_settings(&db, serde_json::json!({
            "enabled": true,
            "per_type": { "task_complete": { "form": "full", "template": "{project} 完成" } },
            "per_event": { "Stop": { "enabled": true }, "SubagentStop": { "enabled": false } }
        })).await;
        let v = vars(&[("project", "aidog")]);
        let r = dispatch(&db, None, Some("SubagentStop"), "", None, &v).await;
        assert!(!r.dispatched, "显式禁用事件不应派发");
        assert!(!r.inbox, "显式禁用事件不应入库");
        let list = super::super::db::list_notifications(&db, 10).await.unwrap();
        assert_eq!(list.len(), 0, "不应有任何入库记录");
    }
}
