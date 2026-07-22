//! 核心分发入口：event 自含路径 + 类型兼容路径 + 唯一 action key 解析。

use std::collections::HashMap;
use std::sync::Arc;

use super::super::db::Db;
use super::super::models::{default_template_for_event, NotifType};
use super::render::{channels_for_form, default_title, render, DispatchResult, BRAND_FALLBACK};
use super::tts::{play_beep, show_popup, speak};
use super::vars::substitute_vars_fill_empty;

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
    let settings = super::super::db::get_notification_settings(db).await;

    // 应用行为追踪 key：与日志 trace_id / 代理 request_id 同口径，标识「触发本次通知的那次操作」。
    // 来源优先级：① 调用方 vars 已带 request_id（如 /api/notify 脚本透传）> ② 当前活跃 span 的
    // trace_id / request_id（tauri command #[instrument] / proxy 请求 span / /api/notify 的 notify span
    // / 后台 span 自动继承）> ③ new_trace_id() 兜底（纯定时器等无操作 span 的来源，如备份失败通知）。
    // 禁固定值；保证每次 dispatch 都带唯一非空 key。注入 vars 供模板引用 + 写入日志便于串回触发来源。
    let action_key = vars
        .get("request_id")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(crate::logging::current_trace_id)
        .unwrap_or_else(crate::logging::new_trace_id);
    let vars_owned: HashMap<String, String> = {
        let mut v = vars.clone();
        v.insert("request_id".to_string(), action_key.clone());
        v
    };
    let vars = &vars_owned;
    tracing::info!(
        request_id = %action_key,
        event = ?event,
        notif_type = %type_str,
        "notify: dispatch",
    );

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
        match super::super::db::insert_notification(db, event_name, &title, &body).await {
            Ok(id) => inbox_id = Some(id),
            Err(e) => tracing::warn!(error = %e, "notify: insert inbox failed"),
        }

        if do_popup
            && let Some(app) = app {
                show_popup(app, &title, &body);
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
    if let Some(ev) = event.filter(|e| !e.is_empty())
        && type_str.is_empty() {
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
        match super::super::db::insert_notification(db, notif_type.as_str(), &title, &body).await {
            Ok(id) => {
                inbox_id = Some(id);
            }
            Err(e) => tracing::warn!(error = %e, "notify: insert inbox failed"),
        }
    }

    // 弹窗：title 空（无 project 注入）时退化到类型默认名，避免空标题弹窗
    if do_popup
        && let Some(app) = app {
            let popup_title = if title.is_empty() {
                default_title(notif_type)
            } else {
                title.as_str()
            };
            show_popup(app, popup_title, &body);
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
