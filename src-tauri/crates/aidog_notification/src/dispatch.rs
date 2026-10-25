//! 核心分发入口（票 04 重写：规则驱动）。
//!
//! 流程：action_key 解析 → 语义类型/级别判定（event 优先；透传 raw 事件名自成类型）→
//! 规则引擎 evaluate（settings.rules 空 = 未迁移/未配置 → 预置 6 条）→ 命中规则经
//! DedupGate 频控后**并集**渠道：本地 popup/sound/tts/inbox + 外部渠道（无配置旁路，
//! fire-and-forget）。免打扰总闸压掉打断性渠道（规则 bypass_dnd 穿透）；总开关 OFF 整体旁路。
//!
//! per_event / per_type 不再控制通道（票 04 起），仅存**模板**（正文渲染沿用两级模板选择）；
//! per_event 仍控制 hook 注入面（aidog_core::hooks，不在本 crate）。

use std::collections::HashMap;
use std::sync::Arc;

use super::channels::{self, ExternalChannel};
use super::render::{BRAND_FALLBACK, DispatchResult, default_title, render};
use super::rules::{Channel, MatchInput, dedup_gate, rule_matches};
use super::semantic::severity_for_type;
use super::tts::{play_beep, show_popup, speak};
use super::vars::substitute_vars_fill_empty;
use aidog_ctx::AppCtx;
use aidog_db::Db;
use aidog_db::models::{NotifType, PROXY_SEMANTIC_EVENTS, default_template_for_event};

/// 核心分发入口。
///
/// `ctx` 为可选 [`AppCtx`]（无头测试 / 端点未带 ctx 时为 None，则跳过 popup/sound/emit，仅落库）。
pub async fn dispatch(
    db: &Arc<Db>,
    ctx: Option<&dyn AppCtx>,
    event: Option<&str>,
    type_str: &str,
    content: Option<&str>,
    vars: &HashMap<String, String>,
) -> DispatchResult {
    let settings = aidog_db::get_notification_settings(db).await;

    // 应用行为追踪 key：与日志 trace_id / 代理 request_id 同口径，标识「触发本次通知的那次操作」。
    // 来源优先级：① 调用方 vars 已带 request_id（如 /api/notify 脚本透传）> ② 当前活跃 span 的
    // trace_id / request_id > ③ new_trace_id() 兜底。注入 vars 供模板引用 + 写入日志便于串回触发来源。
    let action_key = vars
        .get("request_id")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(aidog_db::logging::current_trace_id)
        .unwrap_or_else(aidog_db::logging::new_trace_id);
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

    // 语义类型：event 非空优先（CC 事件名 / proxy 语义事件 / 透传 raw 名），否则 type_str（Codex）。
    let typ = event
        .filter(|e| !e.is_empty())
        .unwrap_or(type_str)
        .to_string();
    let severity = severity_for_type(&typ);
    // 来源：proxy 语义事件 / CC hook 事件 / 外部脚本（api）。
    let source = if PROXY_SEMANTIC_EVENTS.contains(&typ.as_str()) {
        "proxy"
    } else if event.is_some_and(|e| !e.is_empty()) {
        "hook"
    } else {
        "api"
    };
    // 渲染兜底类型（未知/透传名 → TaskComplete 的默认模板），通道不再看它。
    let notif_type = NotifType::from_str_or_default(&typ);

    // 标题/正文：event 路径沿用 per_event 模板 > 事件默认模板 > 类型默认（fill_empty 防裸 {x}）；
    // type 路径沿用 render（per_type 模板 / content / 类型默认）。
    let (title, body) = if let Some(ev) = event.filter(|e| !e.is_empty()) {
        let raw_body = settings
            .event_setting(ev)
            .map(|es| es.template.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| {
                let dft = default_template_for_event(ev);
                if !dft.is_empty() {
                    dft.to_string()
                } else {
                    notif_type.default_template().to_string()
                }
            });
        let mut bvars = vars.clone();
        if bvars.get("project").is_none_or(|s| s.trim().is_empty()) {
            bvars.insert("project".to_string(), BRAND_FALLBACK.to_string());
        }
        let body = substitute_vars_fill_empty(&raw_body, &bvars);
        let title = vars
            .get("project")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ev.to_string());
        (title, body)
    } else {
        let setting = settings.type_setting(notif_type);
        render(notif_type, setting.template.as_str(), content, vars)
    };

    // 总开关 OFF → 整体旁路。
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

    // 规则匹配 + 频控 + 渠道并集（免打扰压打断性渠道；bypass_dnd 穿透）。
    let input = MatchInput {
        typ: &typ,
        severity: severity.as_str(),
        vars,
    };
    let rules = settings.effective_rules();
    let now_ms = aidog_db::now();
    let dedup_key = format!(
        "{}:{}",
        typ,
        vars.get("platform_id").map(String::as_str).unwrap_or("")
    );
    let gate = dedup_gate();
    let mut do_popup = false;
    let mut do_sound = false;
    let mut do_tts = false;
    let mut inbox = false;
    let mut ext: Vec<Channel> = Vec::new();
    for r in rules.iter().filter(|r| rule_matches(r, &input)) {
        if !gate.pass(&r.id, &dedup_key, now_ms, r.action.dedup_window_secs) {
            continue;
        }
        let interruptible = r.action.bypass_dnd || !settings.dnd_enabled;
        for ch in &r.action.channels {
            match ch {
                Channel::Popup => do_popup |= interruptible,
                Channel::Sound => do_sound |= interruptible,
                // TTS 仍与全局 tts_enabled 取与（沿用旧语义）。
                Channel::Tts => do_tts |= interruptible && settings.tts_enabled,
                Channel::Inbox => inbox = true,
                external => {
                    if interruptible {
                        ext.push(*external);
                    }
                }
            }
        }
    }

    if !(do_popup || do_sound || do_tts || inbox || !ext.is_empty()) {
        // 无命中规则（未启用事件 / 无匹配透传类型）→ 不派发，与旧「未启用不派发」一致。
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

    // 收件箱落库（带 source/severity/vars 快照 + action_key：筛选与来源跳转用）。
    let mut inbox_id = None;
    if inbox {
        match aidog_db::insert_notification_full(
            db,
            &typ,
            &title,
            &body,
            source,
            severity.as_str(),
            vars,
            &action_key,
        )
        .await
        {
            Ok(id) => inbox_id = Some(id),
            Err(e) => tracing::warn!(error = %e, "notify: insert inbox failed"),
        }
    }

    // 弹窗（title 空时退化类型默认名，防空标题）。
    if do_popup && let Some(ctx) = ctx {
        let popup_title = if title.is_empty() {
            default_title(notif_type)
        } else {
            title.as_str()
        };
        show_popup(ctx, popup_title, &body);
    }
    // TTS（body 空则播 title）。
    if do_tts {
        let speak_text = if body.is_empty() {
            title.clone()
        } else {
            body.clone()
        };
        speak(ctx, settings.tts_backend, &speak_text);
    }
    // 提示音（无头测试 ctx=None 跳过实际播音）。
    if do_sound && ctx.is_some() {
        play_beep();
    }

    // 外部渠道：fire-and-forget（不阻塞分发；终态各自落 notification_delivery）。
    // 无 tokio runtime（纯同步测试直调）→ 跳过（与 scheduling emit_breaker_event 同 idiom）。
    if !ext.is_empty() && tokio::runtime::Handle::try_current().is_ok() {
        let db2 = db.clone();
        let chans = settings.channels.clone();
        let t = title.clone();
        let b = body.clone();
        let ak = action_key.clone();
        tokio::spawn(async move {
            for ch in ext {
                let Some(kind) = map_external(ch) else { continue };
                if channels::configured(kind, &chans) {
                    channels::deliver_external(&db2, kind, &chans, &t, &b, &ak).await;
                }
            }
        });
    }

    DispatchResult {
        dispatched: true,
        title,
        body,
        tts: do_tts,
        popup: do_popup,
        sound: do_sound,
        inbox,
        inbox_id,
    }
}

/// 规则渠道 → 外部渠道种类（本地 4 形态返回 None）。
fn map_external(ch: Channel) -> Option<ExternalChannel> {
    match ch {
        Channel::Webhook => Some(ExternalChannel::Webhook),
        Channel::Telegram => Some(ExternalChannel::Telegram),
        Channel::Bark => Some(ExternalChannel::Bark),
        Channel::Discord => Some(ExternalChannel::Discord),
        Channel::Slack => Some(ExternalChannel::Slack),
        Channel::Email => Some(ExternalChannel::Email),
        Channel::Feishu => Some(ExternalChannel::Feishu),
        Channel::Popup | Channel::Inbox | Channel::Sound | Channel::Tts => None,
    }
}

#[cfg(test)]
#[path = "test_dispatch.rs"]
mod test_dispatch;
