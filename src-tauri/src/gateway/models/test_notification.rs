//! notification.rs 单测（原 models.rs `notif_event_model_tests` + `middleware_model_tests`
//! 中的 NotifType / NotifForm / TtsBackend / NotificationSettings 相关用例）。

use super::*;

#[test]
fn default_template_per_event_independent() {
    // 每事件模板各自独立、用其专属入参（抽样核对）。
    assert_eq!(default_template_for_event("Stop"), "{project} 任务完成");
    assert_eq!(default_template_for_event("SubagentStop"), "{project} 子代理 {agent_type} 完成");
    assert_eq!(default_template_for_event("Notification"), "{project}：{message}");
    assert_eq!(default_template_for_event("PostToolUseFailure"), "{project} {tool_name} 失败：{error}");
    assert_eq!(default_template_for_event("SessionEnd"), "{project} 会话结束（{end_reason}）");
    // 全量目录每事件都有非空专属默认模板。
    for e in CC_HOOK_EVENTS {
        assert!(!default_template_for_event(e).is_empty(), "event {e} missing default template");
    }
    // 各事件模板互不相同（无统一模板）。
    let mut seen = std::collections::HashSet::new();
    for e in CC_HOOK_EVENTS {
        let t = default_template_for_event(e);
        assert!(seen.insert(t), "duplicate default template across events: {t}");
    }
    // 未命中事件 → 空串（dispatch 兜底类型 default_template）。
    assert_eq!(default_template_for_event("UnknownEvent"), "");
}

#[test]
fn default_on_set_subset_of_catalog() {
    for e in DEFAULT_ON_EVENTS {
        assert!(CC_HOOK_EVENTS.contains(e), "default-on event {e} not in catalog");
    }
    // 默认 ON 仅 Stop + PermissionRequest（用户指示精简）。
    assert_eq!(DEFAULT_ON_EVENTS, &["Stop", "PermissionRequest"]);
    // 这些事件在目录但默认 off（可手动开）。
    for e in [
        "SessionStart",
        "SubagentStop",
        "Notification",
        "SessionEnd",
        "PreCompact",
    ] {
        assert!(CC_HOOK_EVENTS.contains(&e), "event {e} should be in catalog");
        assert!(!DEFAULT_ON_EVENTS.contains(&e), "event {e} should default off");
    }
}

#[test]
fn settings_backward_compat_without_per_event() {
    // 旧 JSON 无 per_event → 反序列化为空 map，不报错。
    let json = serde_json::json!({
        "enabled": true,
        "tts_enabled": true,
        "tts_backend": "cross_platform",
        "per_type": {}
    });
    let s: NotificationSettings = serde_json::from_value(json).unwrap();
    assert!(s.per_event.is_empty());
    assert!(s.event_setting("Stop").is_none());
}

#[test]
fn event_setting_roundtrip() {
    let json = serde_json::json!({
        "per_event": {
            "Stop": { "enabled": true, "tts": false, "popup": true, "template": "{project} done" },
            "PostToolUse": { "enabled": false }
        }
    });
    let s: NotificationSettings = serde_json::from_value(json).unwrap();
    let stop = s.event_setting("Stop").unwrap();
    assert!(stop.enabled);
    assert!(!stop.tts);
    assert!(stop.popup);
    assert_eq!(stop.template, "{project} done");
    // tts/popup/template 缺省 → tts/popup default true、template 空串。
    let pt = s.event_setting("PostToolUse").unwrap();
    assert!(!pt.enabled);
    assert!(pt.tts);
    assert!(pt.popup);
    assert_eq!(pt.template, "");
}

#[test]
fn event_setting_backward_compat_ignores_legacy_notif_type() {
    // 旧 DB per_event 含 notif_type（已删字段）→ serde 无 deny_unknown 忽略多余字段，
    // 旧缺 tts/popup → serde default true。不报错。
    let json = serde_json::json!({
        "per_event": {
            "SubagentStop": { "enabled": true, "notif_type": "error", "template": "x" }
        }
    });
    let s: NotificationSettings = serde_json::from_value(json).unwrap();
    let es = s.event_setting("SubagentStop").unwrap();
    assert!(es.enabled);
    assert!(es.tts); // 旧无 → default true
    assert!(es.popup); // 旧无 → default true
    assert_eq!(es.template, "x");
}

#[test]
fn notif_type_serde_snake_case_roundtrip() {
    for (variant, lit) in [
        (NotifType::TaskComplete, "\"task_complete\""),
        (NotifType::WaitingInput, "\"waiting_input\""),
        (NotifType::Error, "\"error\""),
    ] {
        assert_eq!(serde_json::to_string(&variant).unwrap(), lit);
        let back: NotifType = serde_json::from_str(lit).unwrap();
        assert_eq!(back, variant);
        assert_eq!(format!("\"{}\"", variant.as_str()), lit);
    }
    assert_eq!(NotifType::from_str_or_default("waiting_input"), NotifType::WaitingInput);
    assert_eq!(NotifType::from_str_or_default("unknown_xyz"), NotifType::TaskComplete);
}

#[test]
fn notif_form_and_backend_serde() {
    assert_eq!(serde_json::to_string(&NotifForm::PopupOnly).unwrap(), "\"popup_only\"");
    assert_eq!(serde_json::to_string(&NotifForm::InboxOnly).unwrap(), "\"inbox_only\"");
    assert_eq!(serde_json::to_string(&NotifForm::SoundOnly).unwrap(), "\"sound_only\"");
    assert_eq!(serde_json::to_string(&NotifForm::Full).unwrap(), "\"full\"");
    assert_eq!(NotifForm::default(), NotifForm::Full);
    assert_eq!(serde_json::to_string(&TtsBackend::CrossPlatform).unwrap(), "\"cross_platform\"");
    assert_eq!(serde_json::to_string(&TtsBackend::MacSay).unwrap(), "\"mac_say\"");
    assert_eq!(serde_json::to_string(&TtsBackend::WebSpeech).unwrap(), "\"web_speech\"");
    assert_eq!(TtsBackend::default(), TtsBackend::CrossPlatform);
}

#[test]
fn notification_settings_default_and_partial() {
    let s = NotificationSettings::default();
    assert!(s.enabled);
    assert!(s.tts_enabled);
    assert_eq!(s.tts_backend, TtsBackend::CrossPlatform);
    assert_eq!(s.inbox_retention_days, 7);
    assert!(s.per_type.is_empty());
    // 缺省类型 → 全 true + Full
    let ts = s.type_setting(NotifType::Error);
    assert!(ts.tts && ts.popup);
    assert_eq!(ts.form, NotifForm::Full);

    // 部分 JSON 填默认
    let p: NotificationSettings = serde_json::from_str("{\"enabled\":false}").unwrap();
    assert!(!p.enabled);
    assert!(p.tts_enabled);
    assert_eq!(p.tts_backend, TtsBackend::CrossPlatform);
    // 旧配置无 inbox_retention_days → serde default 回退 7
    assert_eq!(p.inbox_retention_days, 7);

    // per_type 显式覆盖往返
    let mut s2 = NotificationSettings::default();
    s2.per_type.insert(
        NotifType::TaskComplete.as_str().into(),
        TypeSetting { tts: false, popup: true, form: NotifForm::InboxOnly, template: "{project} done".into() },
    );
    let json = serde_json::to_string(&s2).unwrap();
    let back: NotificationSettings = serde_json::from_str(&json).unwrap();
    let got = back.type_setting(NotifType::TaskComplete);
    assert!(!got.tts);
    assert_eq!(got.form, NotifForm::InboxOnly);
    assert_eq!(got.template, "{project} done");
}
