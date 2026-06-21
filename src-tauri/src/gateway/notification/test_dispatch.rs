use super::*;
use super::super::db::Db;
use std::collections::HashMap;
use std::sync::Arc;

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
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

// ── 应用行为追踪 key（request_id）注入 ──
fn is_trace_key(s: &str) -> bool {
    // new_trace_id() = 8 位十六进制
    s.len() == 8 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[tokio::test]
async fn dispatch_injects_nonempty_unique_action_key() {
    let db = mem_db().await;
    set_form(&db, "task_complete", "full", true).await;
    // 模板引用 {request_id} → body 即为注入的 key（无 span 时走 new_trace_id 兜底）。
    super::super::db::set_setting(&db, super::super::models::SetSettingInput {
        scope: "notification".into(),
        key: "settings".into(),
        value: serde_json::json!({
            "enabled": true, "tts_enabled": false, "tts_backend": "cross_platform",
            "per_type": { "task_complete": { "tts": false, "popup": false, "form": "inbox_only", "template": "{request_id}" } }
        }),
    }).await.unwrap();
    let r = dispatch(&db, None, None, "task_complete", None, &HashMap::new()).await;
    assert!(r.dispatched);
    assert!(!r.body.is_empty(), "action key must be non-empty");
    assert!(is_trace_key(&r.body), "fallback key must be 8-hex trace id, got {:?}", r.body);
}

#[tokio::test]
async fn dispatch_different_triggers_get_different_keys() {
    let db = mem_db().await;
    super::super::db::set_setting(&db, super::super::models::SetSettingInput {
        scope: "notification".into(),
        key: "settings".into(),
        value: serde_json::json!({
            "enabled": true, "tts_enabled": false, "tts_backend": "cross_platform",
            "per_type": { "task_complete": { "tts": false, "popup": false, "form": "inbox_only", "template": "{request_id}" } }
        }),
    }).await.unwrap();
    let r1 = dispatch(&db, None, None, "task_complete", None, &HashMap::new()).await;
    let r2 = dispatch(&db, None, None, "task_complete", None, &HashMap::new()).await;
    assert!(is_trace_key(&r1.body) && is_trace_key(&r2.body));
    assert_ne!(r1.body, r2.body, "两次独立触发须得不同 key");
}

#[tokio::test]
async fn dispatch_prefers_caller_request_id_in_vars() {
    let db = mem_db().await;
    super::super::db::set_setting(&db, super::super::models::SetSettingInput {
        scope: "notification".into(),
        key: "settings".into(),
        value: serde_json::json!({
            "enabled": true, "tts_enabled": false, "tts_backend": "cross_platform",
            "per_type": { "task_complete": { "tts": false, "popup": false, "form": "inbox_only", "template": "{request_id}" } }
        }),
    }).await.unwrap();
    let v = vars(&[("request_id", "deadbeefcafe0001")]);
    let r = dispatch(&db, None, None, "task_complete", None, &v).await;
    // 调用方已带 request_id → 原样沿用，不再兜底生成。
    assert_eq!(r.body, "deadbeefcafe0001");
}

#[tokio::test]
async fn dispatch_captures_env_span_trace_id() {
    // 模拟 tauri command #[instrument] / proxy 请求 span：dispatch 在带 trace_id 的活跃 span 内
    // 运行时，应捕获该 span 的 id 作为 action key（与日志同口径），而非另造新 id。
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::registry().with(crate::logging::trace_id_layer_for_test());
    let _guard = tracing::subscriber::set_default(subscriber);

    let db = mem_db().await;
    super::super::db::set_setting(&db, super::super::models::SetSettingInput {
        scope: "notification".into(),
        key: "settings".into(),
        value: serde_json::json!({
            "enabled": true, "tts_enabled": false, "tts_backend": "cross_platform",
            "per_type": { "task_complete": { "tts": false, "popup": false, "form": "inbox_only", "template": "{request_id}" } }
        }),
    }).await.unwrap();

    let tid = crate::logging::new_trace_id();
    let span = tracing::info_span!("notify", trace_id = %tid);
    let r = {
        use tracing::Instrument;
        dispatch(&db, None, None, "task_complete", None, &HashMap::new())
            .instrument(span)
            .await
    };
    assert_eq!(r.body, tid, "dispatch 应沿用活跃 span 的 trace_id 作为 action key");
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
