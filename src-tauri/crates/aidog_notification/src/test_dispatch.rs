// dispatch 规则驱动接线测试（票 04 重写：通道由规则控制，per_event/per_type 只存模板）。
// 文案断言：t 不引入，模板 fallback 为 zh 硬编码（与 models 同源），文本确定性等价字面量。
use std::collections::HashMap;
use std::sync::Arc;

use aidog_db::models::{NotificationSettings, Rule, RuleAction, Channel};
use aidog_db::Db;

use super::dispatch;

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

async fn mem_db() -> Arc<Db> {
    let db = Db::new(":memory:").await.unwrap();
    aidog_db::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
    Arc::new(db)
}

async fn set_rules(db: &Arc<Db>, f: impl FnOnce(&mut NotificationSettings)) {
    let mut s = NotificationSettings::default();
    f(&mut s);
    if s.rules.is_empty() {
        s.rules = aidog_db::models::preset_rules();
    }
    aidog_db::set_setting(db, aidog_db::models::SetSettingInput {
        scope: "notification".to_string(),
        key: "settings".to_string(),
        value: serde_json::to_value(&s).unwrap(),
    })
    .await
    .unwrap();
}

fn user_rule(id: &str, typ: &str, channels: Vec<Channel>, dedup: u32, bypass_dnd: bool) -> Rule {
    Rule {
        id: id.to_string(),
        name: id.to_string(),
        enabled: true,
        preset: false,
        match_type: Some(typ.to_string()),
        condition: None,
        action: RuleAction { channels, bypass_dnd, dedup_window_secs: dedup },
    }
}

#[tokio::test]
async fn presets_deliver_task_complete_all_local_channels() {
    let db = mem_db().await;
    set_rules(&db, |_| {}).await; // rules 空 → 预置
    let r = dispatch(&db, None, None, "task_complete", None, &vars(&[("project", "P1")])).await;
    assert!(r.dispatched);
    assert!(r.inbox);
    assert!(r.popup);
    assert!(r.sound);
    assert!(r.tts);
    assert!(r.inbox_id.is_some());
    // 收件箱行带 source=api（无 event、非 proxy 语义）+ severity=info
    let rows = aidog_db::list_notifications(&db, 10).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source, "api");
    assert_eq!(rows[0].severity, "info");
    assert_eq!(rows[0].notif_type, "task_complete");
    assert!(!rows[0].action_key.is_empty());
}

#[tokio::test]
async fn presets_match_cc_hook_event_by_semantic_type() {
    let db = mem_db().await;
    set_rules(&db, |_| {}).await;
    let r = dispatch(&db, None, Some("Stop"), "", None, &vars(&[("project", "P1")])).await;
    assert!(r.dispatched);
    assert!(r.popup);
    assert!(r.inbox);
    assert!(r.sound);
    assert!(r.tts);
}

#[tokio::test]
async fn breaker_event_source_proxy_and_severity_error() {
    let db = mem_db().await;
    set_rules(&db, |_| {}).await;
    let r = dispatch(
        &db,
        None,
        Some("breaker_on"),
        "",
        None,
        &vars(&[("platform", "glm_coding"), ("platform_id", "7")]),
    )
    .await;
    assert!(r.dispatched && r.inbox);
    let rows = aidog_db::list_notifications(&db, 10).await.unwrap();
    assert_eq!(rows[0].source, "proxy");
    assert_eq!(rows[0].severity, "error");
    assert_eq!(rows[0].vars.get("platform").map(String::as_str), Some("glm_coding"));
}

#[tokio::test]
async fn no_matching_rule_suppressed() {
    let db = mem_db().await;
    set_rules(&db, |s| {
        s.rules = vec![user_rule("only-task", "task_complete", vec![Channel::Inbox], 0, false)];
    })
    .await;
    // raw 透传类型无规则命中 → 不派发不落库
    let r = dispatch(&db, None, Some("WhateverRaw"), "", None, &vars(&[])).await;
    assert!(!r.dispatched);
    let rows = aidog_db::list_notifications(&db, 10).await.unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn master_switch_off_bypasses() {
    let db = mem_db().await;
    set_rules(&db, |s| {
        s.enabled = false;
    })
    .await;
    let r = dispatch(&db, None, None, "task_complete", None, &vars(&[("project", "P")])).await;
    assert!(!r.dispatched);
    assert!(aidog_db::list_notifications(&db, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn dnd_suppresses_interruptible_inbox_persists_and_bypass_pierces() {
    let db = mem_db().await;
    set_rules(&db, |s| {
        s.dnd_enabled = true;
        s.rules = vec![
            user_rule("r-quiet", "error", vec![Channel::Popup, Channel::Inbox], 0, false),
            user_rule("r-pierce", "error", vec![Channel::Sound], 0, true),
        ];
    })
    .await;
    let r = dispatch(&db, None, None, "error", None, &vars(&[])).await;
    // popup 被免打扰压掉（第一条规则），sound 由 bypass_dnd 规则穿透，inbox 恒可落
    assert!(r.dispatched);
    assert!(!r.popup);
    assert!(r.sound);
    assert!(r.inbox);
}

#[tokio::test]
async fn dedup_window_suppresses_second_dispatch() {
    let db = mem_db().await;
    set_rules(&db, |s| {
        s.rules = vec![user_rule("r-dedup", "quota_exhaust_forecast", vec![Channel::Inbox], 3600, false)];
    })
    .await;
    let v = vars(&[("project", "P"), ("platform_id", "42")]);
    let r1 = dispatch(&db, None, Some("quota_exhaust_forecast"), "", None, &v).await;
    assert!(r1.dispatched);
    let r2 = dispatch(&db, None, Some("quota_exhaust_forecast"), "", None, &v).await;
    assert!(!r2.dispatched, "同 dedup_key 窗口内第二次应被频控");
    assert_eq!(aidog_db::list_notifications(&db, 10).await.unwrap().len(), 1, "窗口内只落一条");
}

#[tokio::test]
async fn action_key_unique_and_prefers_caller_request_id() {
    let db = mem_db().await;
    set_rules(&db, |_| {}).await;
    // 调用方 request_id 优先
    let r = dispatch(&db, None, None, "task_complete", None, &vars(&[("request_id", "abc")])).await;
    assert!(r.dispatched);
    let rows = aidog_db::list_notifications(&db, 10).await.unwrap();
    assert_eq!(rows[0].action_key, "abc");
    // 无 request_id → 非空且两次不同
    let _ = dispatch(&db, None, None, "waiting_input", None, &vars(&[])).await;
    let _ = dispatch(&db, None, None, "waiting_input", None, &vars(&[])).await;
    let rows = aidog_db::list_notifications(&db, 10).await.unwrap();
    let keys: Vec<&String> = rows.iter().map(|n| &n.action_key).collect();
    assert!(keys.iter().all(|k| !k.is_empty()));
    assert_ne!(keys[1], keys[2], "两次触发 key 应不同");
}

#[tokio::test]
async fn event_path_template_fallback_and_fill_empty() {
    let db = mem_db().await;
    set_rules(&db, |s| {
        s.rules = vec![user_rule("r-stop", "Stop", vec![Channel::Inbox], 0, false)];
    })
    .await;
    // Stop 事件默认模板「{project} 任务完成」；缺 project 注入品牌兜底；专属字段缺失 → 空串
    let r = dispatch(&db, None, Some("Stop"), "", None, &vars(&[])).await;
    assert!(r.dispatched);
    assert!(r.body.ends_with("任务完成"));
    assert!(!r.body.contains("{"), "不残留裸占位：{}", r.body);
    // 事件名做标题（无 project）
    assert_eq!(r.title, "Stop");
}

#[tokio::test]
async fn external_channel_rule_triggers_delivery_record() {
    // 隔离本机代理（fake endpoint 直连）
    unsafe {
        std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
        std::env::set_var("no_proxy", "127.0.0.1,localhost");
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for _ in 0..4 {
            if let Ok((mut sock, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 8192];
                let _ = sock.read(&mut buf);
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
                let _ = sock.write_all(resp.as_bytes());
            }
        }
    });
    let db = mem_db().await;
    set_rules(&db, |s| {
        s.channels.webhook = Some(aidog_db::models::WebhookCfg {
            url: format!("http://127.0.0.1:{port}/hook"),
            template: String::new(),
        });
        s.rules = vec![user_rule(
            "r-ext",
            "task_complete",
            vec![Channel::Webhook, Channel::Inbox],
            0,
            false,
        )];
    })
    .await;
    let r = dispatch(&db, None, None, "task_complete", None, &vars(&[("project", "P")])).await;
    assert!(r.dispatched && r.inbox);
    // 外部投递是 spawn 的：轮询投递记录（上限 3s）
    let mut found = false;
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let rows = aidog_db::list_notification_deliveries(&db, None, 10).await.unwrap();
        if rows.iter().any(|d| d.channel == "webhook" && d.status == "success") {
            found = true;
            break;
        }
    }
    assert!(found, "webhook 投递记录应出现");
}
