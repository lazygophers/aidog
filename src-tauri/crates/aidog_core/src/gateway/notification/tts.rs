//! 输出后端：系统弹窗、TTS 三后端、系统提示音。

use super::super::models::TtsBackend;

/// 前端事件名：WebSpeech 后端播报请求（payload = 文本，前端 webview SpeechSynthesis 朗读）。
pub const NOTIF_SPEAK: &str = "notif-speak";

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
pub fn show_popup(app: &tauri::AppHandle, title: &str, body: &str) {
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
pub fn speak(app: Option<&tauri::AppHandle>, backend: TtsBackend, text: &str) {
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
pub fn play_beep() {
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
