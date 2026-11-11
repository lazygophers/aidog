#![cfg(test)]
use super::test_support::*;
use super::*;
use rusqlite::params;

// ── Notification 收件箱 CRUD（N1）──
#[tokio::test]
async fn notification_inbox_crud() {
    let db = test_db().await;
    // 空库
    assert!(list_notifications(&db, 50).await.unwrap().is_empty());

    let id1 = insert_notification(&db, "task_complete", "任务完成", "项目 X 完成")
        .await
        .unwrap();
    let id2 = insert_notification(&db, "error", "出错", "构建失败")
        .await
        .unwrap();
    assert!(id2 > id1);

    let list = list_notifications(&db, 50).await.unwrap();
    assert_eq!(list.len(), 2);
    // 倒序：最新在前
    assert_eq!(list[0].id, id2);
    assert_eq!(list[0].notif_type, "error");
    assert_eq!(list[1].title, "任务完成");

    // limit 生效
    for i in 0..5 {
        insert_notification(&db, "task_complete", &format!("t{i}"), "b")
            .await
            .unwrap();
    }
    assert_eq!(list_notifications(&db, 3).await.unwrap().len(), 3);

    // 清空
    clear_notifications(&db).await.unwrap();
    assert!(list_notifications(&db, 50).await.unwrap().is_empty());
}

// ── Notification retention 硬删（默认 7 天 + 不清理）──
#[tokio::test]
async fn cleanup_notifications_hard_deletes_old_rows() {
    let db = test_db().await;
    let now = now();
    let old = now - 100 * 24 * 3600 * 1000; // 100 天前
    let recent = now - 24 * 3600 * 1000; // 1 天前
    for (ts, title) in [(old, "old"), (recent, "recent")] {
        db
                .call_traced(None, std::panic::Location::caller(), move |conn| {
                    conn.execute(
                        "INSERT INTO notification (notif_type, title, body, created_at) VALUES ('error', ?1, '', ?2)",
                        params![title, ts],
                    )?;
                    Ok(())
                })
                .await
                .unwrap();
    }
    assert_eq!(list_notifications(&db, 50).await.unwrap().len(), 2);

    // retention=7 → 删 100 天前，留 1 天前
    cleanup_notifications(&db, 7).await.unwrap();
    let list = list_notifications(&db, 50).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].title, "recent");

    // retention=0 → 跳过清理（永不清理）
    cleanup_notifications(&db, 0).await.unwrap();
    assert_eq!(list_notifications(&db, 50).await.unwrap().len(), 1);
}

#[tokio::test]
async fn notification_settings_default_when_absent() {
    let db = test_db().await;
    let s = get_notification_settings(&db).await;
    assert!(s.enabled && s.tts_enabled);
    // 写入后读回
    set_setting(
        &db,
        SetSettingInput {
            scope: "notification".into(),
            key: "settings".into(),
            // 票 04 起：无 rules 字段视为旧结构会被迁移重置——新结构写入须带 rules 标记。
            value: serde_json::json!({"enabled": false, "tts_enabled": false, "rules": []}),
        },
    )
    .await
    .unwrap();
    let s2 = get_notification_settings(&db).await;
    assert!(!s2.enabled && !s2.tts_enabled);
    assert!(s2.rules.is_empty(), "存储空 rules → effective_rules 回落预置");
}

#[tokio::test]
async fn notification_settings_old_shape_migrates_to_presets() {
    let db = test_db().await;
    // 旧结构（无 rules，含 per_event）：检测 → 重置默认（规则=预置 6 条）并持久化
    set_setting(
        &db,
        SetSettingInput {
            scope: "notification".into(),
            key: "settings".into(),
            value: serde_json::json!({"enabled": false, "per_event": {"Stop": {"enabled": true}}}),
        },
    )
    .await
    .unwrap();
    let s = get_notification_settings(&db).await;
    assert!(s.enabled, "旧结构重置为默认");
    assert_eq!(s.rules.len(), 6, "规则=预置 6 条");
    assert_eq!(s.effective_rules().len(), 6);
    // 第二次读：已持久化为新结构（不再触发迁移路径，flags 保得住）
    set_setting(
        &db,
        SetSettingInput {
            scope: "notification".into(),
            key: "settings".into(),
            value: serde_json::to_value(&crate::models::NotificationSettings {
                enabled: false,
                ..Default::default()
            })
            .unwrap(),
        },
    )
    .await
    .unwrap();
    let s2 = get_notification_settings(&db).await;
    assert!(!s2.enabled);
}

// ── 内置规则删除保护（failed 内置残留可删）──
#[tokio::test]
async fn failed_builtin_rule_can_be_deleted() {
    let db = test_db().await;
    let rules = list_middleware_rules(&db).await.unwrap();
    let builtin = rules
        .iter()
        .find(|r| r.is_builtin)
        .expect("builtin rules seeded");
    // 正常内置：拒删
    let err = delete_middleware_rule(&db, builtin.id).await.unwrap_err();
    assert!(err.contains("cannot be deleted"), "{err}");
    let id = builtin.id;
    // failed 内置残留：放行删（seed 层自动清 + 用户可手删兜底）
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "UPDATE middleware_rule SET failed = 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    delete_middleware_rule(&db, builtin.id).await.unwrap();
    let left = list_middleware_rules(&db).await.unwrap();
    assert!(
        left.iter().all(|r| r.id != builtin.id),
        "failed builtin deleted"
    );
}

// ── 内置 spec conditions JSON 合法性（回归：正则反斜杠未 JSON 转义曾致全部内置规则失效）──
#[test]
fn builtin_spec_conditions_all_parse() {
    for spec in builtin_rule_specs() {
        let parsed: Result<crate::models::ConditionNode, _> = serde_json::from_str(spec.conditions);
        assert!(
            parsed.is_ok(),
            "builtin spec '{}' conditions invalid JSON: {:#?}",
            spec.name,
            parsed.err()
        );
    }
}

#[tokio::test]
async fn notification_delivery_table_exists_in_memory_db() {
    let db = Db::new(":memory:").await.unwrap();
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
    let id = crate::insert_notification_delivery(&db, "k", "webhook", "success", "", 12)
        .await
        .unwrap();
    assert!(id > 0);
    let rows = crate::list_notification_deliveries(&db, Some("k"), 5).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].channel, "webhook");
    assert_eq!(rows[0].duration_ms, 12);
}

// ── 票 03：内置脱敏规则升级为 reversible + all ──
/// (规则名, 占位符 TYPE, 默认启用)。
const REVERSIBLE_BUILTINS: &[(&str, &str, bool)] = &[
    ("内置·密钥脱敏", "SECRET", true),
    ("内置·邮箱脱敏", "EMAIL", true),
    ("内置·私钥脱敏", "PRIVATE_KEY", true),
    ("内置·Bearer token 脱敏", "BEARER_TOKEN", true),
    ("内置·云厂商密钥脱敏", "CLOUD_SECRET", false),
    ("内置·IP/MAC 地址脱敏", "IP_MAC", false),
    ("内置·信用卡号脱敏", "BANK_CARD", false),
    ("内置·IBAN 脱敏", "IBAN", false),
    ("内置·中国身份证号脱敏", "ID_NUMBER", false),
    ("内置·美国社保号脱敏", "US_SSN", false),
];

fn assert_reversible_all(r: &crate::models::MiddlewareRule, ty: &str) {
    assert_eq!(r.actions.len(), 1, "{}", r.name);
    let p = &r.actions[0].params;
    assert_eq!(r.actions[0].kind, crate::models::ActionKind::Mask, "{}", r.name);
    assert_eq!(p.mode, crate::models::MaskMode::Reversible, "{}", r.name);
    assert_eq!(p.fields, vec!["all".to_string()], "{}", r.name);
    assert_eq!(p.placeholder_type, ty, "{}", r.name);
}

#[tokio::test]
async fn fresh_install_builtin_privacy_rules_are_reversible_all() {
    let (db, dir) = test_support::three_file_db().await;
    drop(db);
    // 重开库再断言（落盘值，而非内存态）
    let db = Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap();
    let rules = list_middleware_rules(&db).await.unwrap();
    for (name, ty, enabled) in REVERSIBLE_BUILTINS {
        let r = rules
            .iter()
            .find(|r| r.is_builtin && r.name == *name)
            .unwrap_or_else(|| panic!("缺内置规则 {name}"));
        assert_reversible_all(r, ty);
        assert_eq!(r.enabled, *enabled, "{name} 启用默认值");
    }
}

#[tokio::test]
async fn upgrade_rewrites_old_builtin_actions_keeps_enabled() {
    let (db, dir) = test_support::three_file_db().await;
    // 模拟旧版本：密钥脱敏是 fixed + messages/system，且用户已手动关闭；删掉新规则
    db.call_traced(None, std::panic::Location::caller(), |conn| {
        conn.execute(
            "UPDATE middleware_rule SET enabled = 0, actions = ?1 WHERE name = '内置·密钥脱敏' AND is_builtin = 1",
            params![r#"[{"kind":"mask","params":{"replacement":"****","fields":["messages","system"]}}]"#],
        )?;
        conn.execute(
            "DELETE FROM middleware_rule WHERE name IN ('内置·私钥脱敏','内置·Bearer token 脱敏')",
            [],
        )?;
        crate::schema::seed_builtin_middleware_rules(conn)?;
        Ok(())
    })
    .await
    .unwrap();
    drop(db);
    let db = Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap();
    let rules = list_middleware_rules(&db).await.unwrap();
    let secret = rules.iter().find(|r| r.name == "内置·密钥脱敏").unwrap();
    assert_reversible_all(secret, "SECRET");
    assert!(!secret.enabled, "升级不覆盖用户的启停态");
    assert!(rules.iter().any(|r| r.name == "内置·私钥脱敏" && r.enabled));
    assert!(rules.iter().any(|r| r.name == "内置·Bearer token 脱敏" && r.enabled));
}

/// 内置规则内容可编辑（20261111-01 用户可配置内置规则）：内容编辑置 user_modified，
/// 纯启停不置；user_modified=1 后 seed 跳过覆盖（用户版本优先）。
#[tokio::test]
async fn builtin_edit_marks_user_modified_and_seed_respects_it() {
    let (db, _dir) = test_support::three_file_db().await;
    let rules = list_middleware_rules(&db).await.unwrap();
    let r = rules.iter().find(|r| r.name == "内置·密钥脱敏").unwrap();

    // 纯启停：不置标记
    let toggled = update_middleware_rule(
        &db,
        crate::models::UpdateMiddlewareRule {
            id: r.id,
            name: r.name.clone(),
            description: r.description.clone(),
            conditions: r.conditions.clone(),
            actions: r.actions.clone(),
            applies_to: r.applies_to.clone(),
            priority: r.priority,
            enabled: !r.enabled,
        },
    )
    .await
    .unwrap();
    assert!(!toggled.user_modified, "纯启停不置 user_modified");

    // 内容编辑：置标记 + 内容生效
    let mut fixed = r.actions.clone();
    fixed[0].params.mode = crate::models::MaskMode::Fixed;
    fixed[0].params.fields = vec!["messages".into()];
    let out = update_middleware_rule(
        &db,
        crate::models::UpdateMiddlewareRule {
            id: r.id,
            name: r.name.clone(),
            description: r.description.clone(),
            conditions: r.conditions.clone(),
            actions: fixed,
            applies_to: r.applies_to.clone(),
            priority: r.priority,
            enabled: r.enabled,
        },
    )
    .await
    .unwrap();
    assert!(out.user_modified, "内容编辑置 user_modified");
    assert_eq!(out.actions[0].params.mode, crate::models::MaskMode::Fixed);

    // seed 再跑：用户版本保留
    db
        .call_traced(None, std::panic::Location::caller(), |conn| {
            crate::schema::seed_builtin_middleware_rules(conn)?;
            Ok(())
        })
        .await
        .unwrap();
    let rules2 = list_middleware_rules(&db).await.unwrap();
    let r2 = rules2.iter().find(|r| r.name == "内置·密钥脱敏").unwrap();
    assert!(r2.user_modified, "seed 不清用户修改标记");
    assert_eq!(
        r2.actions[0].params.mode,
        crate::models::MaskMode::Fixed,
        "seed 跳过 user_modified=1 的内置规则"
    );
}

/// 未修改的内置规则：seed 仍按规格覆盖内容（默认值随版本升级自动生效的回归护栏）。
#[tokio::test]
async fn unmodified_builtin_content_overwritten_by_seed() {
    let (db, _dir) = test_support::three_file_db().await;
    // SQL 直改内容（不经过 update 命令 → user_modified 保持 0），模拟旧版默认内容
    db
        .call_traced(None, std::panic::Location::caller(), |conn| {
            conn.execute(
                "UPDATE middleware_rule SET actions = ?1 WHERE name = '内置·密钥脱敏' AND is_builtin = 1",
                params![r#"[{"kind":"mask","params":{"replacement":"****","fields":["messages","system"]}}]"#],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    db
        .call_traced(None, std::panic::Location::caller(), |conn| {
            crate::schema::seed_builtin_middleware_rules(conn)?;
            Ok(())
        })
        .await
        .unwrap();
    let rules = list_middleware_rules(&db).await.unwrap();
    let r = rules.iter().find(|r| r.name == "内置·密钥脱敏").unwrap();
    assert_reversible_all(r, "SECRET");
    assert!(!r.user_modified);
}

/// 「还原默认」：内容 + enabled 回规格值、user_modified 清零；之后 seed 恢复覆盖。
#[tokio::test]
async fn reset_middleware_rule_to_builtin_restores_spec() {
    let (db, _dir) = test_support::three_file_db().await;
    let rules = list_middleware_rules(&db).await.unwrap();
    let r = rules.iter().find(|r| r.name == "内置·密钥脱敏").unwrap();
    let mut fixed = r.actions.clone();
    fixed[0].params.mode = crate::models::MaskMode::Fixed;
    fixed[0].params.fields = vec!["messages".into()];
    update_middleware_rule(
        &db,
        crate::models::UpdateMiddlewareRule {
            id: r.id,
            name: r.name.clone(),
            description: r.description.clone(),
            conditions: r.conditions.clone(),
            actions: fixed,
            applies_to: r.applies_to.clone(),
            priority: r.priority,
            enabled: false,
        },
    )
    .await
    .unwrap();

    let out = reset_middleware_rule_to_builtin(&db, r.id).await.unwrap();
    assert_reversible_all(&out, "SECRET");
    assert!(out.enabled, "enabled 回规格 default_enabled");
    assert!(!out.user_modified, "还原清除修改标记");

    // 还原后再启停 + seed：内容仍是规格值（user_modified=0 → 恢复随升级覆盖）
    update_middleware_rule(
        &db,
        crate::models::UpdateMiddlewareRule {
            id: r.id,
            name: out.name.clone(),
            description: out.description.clone(),
            conditions: out.conditions.clone(),
            actions: out.actions.clone(),
            applies_to: out.applies_to.clone(),
            priority: out.priority,
            enabled: false,
        },
    )
    .await
    .unwrap();
    db
        .call_traced(None, std::panic::Location::caller(), |conn| {
            crate::schema::seed_builtin_middleware_rules(conn)?;
            Ok(())
        })
        .await
        .unwrap();
    let rules2 = list_middleware_rules(&db).await.unwrap();
    let r2 = rules2.iter().find(|r| r.name == "内置·密钥脱敏").unwrap();
    assert_reversible_all(r2, "SECRET");
    assert!(!r2.enabled, "seed 保留用户启停态");
}

/// 非内置规则不可还原（「还原默认」只对内置规则有意义）。
#[tokio::test]
async fn reset_rejects_non_builtin_rule() {
    let (db, _dir) = test_support::three_file_db().await;
    let rules = list_middleware_rules(&db).await.unwrap();
    let user_rule = rules.iter().find(|r| !r.is_builtin);
    let Some(r) = user_rule else {
        // 无用户规则时现建一条
        let created = create_middleware_rule(
            &db,
            crate::models::CreateMiddlewareRule {
                name: "user-x".into(),
                description: String::new(),
                conditions: crate::models::ConditionNode::Leaf(crate::models::ConditionLeaf {
                    target: crate::models::Target::RequestBody,
                    field: String::new(),
                    match_type: crate::models::MatchType::Contains,
                    pattern: "x".into(),
                    validator: String::new(),
                }),
                actions: vec![],
                applies_to: Default::default(),
                priority: 0,
                enabled: true,
                is_builtin: false,
            },
        )
        .await
        .unwrap();
        let err = reset_middleware_rule_to_builtin(&db, created.id)
            .await
            .unwrap_err();
        assert!(err.contains("not a builtin"), "{err}");
        return;
    };
    let err = reset_middleware_rule_to_builtin(&db, r.id).await.unwrap_err();
    assert!(err.contains("not a builtin"), "{err}");
}

/// 内置规则禁改名：update 改 name → Err（seed 按 name 幂等、还原默认按 name 定位规格，
/// 改名会让 seed 重复 INSERT、还原 None）；seed 再跑不产生重复内置行。
#[tokio::test]
async fn builtin_rule_rename_rejected() {
    let (db, _dir) = test_support::three_file_db().await;
    let rules = list_middleware_rules(&db).await.unwrap();
    let r = rules.iter().find(|r| r.is_builtin).unwrap();
    let err = update_middleware_rule(
        &db,
        crate::models::UpdateMiddlewareRule {
            id: r.id,
            name: "renamed-x".into(),
            description: r.description.clone(),
            conditions: r.conditions.clone(),
            actions: r.actions.clone(),
            applies_to: r.applies_to.clone(),
            priority: r.priority,
            enabled: r.enabled,
        },
    )
    .await
    .unwrap_err();
    assert!(err.contains("cannot be renamed"), "{err}");
    // seed 再跑：不因「按名查不到」重复 INSERT
    db
        .call_traced(None, std::panic::Location::caller(), |conn| {
            crate::schema::seed_builtin_middleware_rules(conn)?;
            Ok(())
        })
        .await
        .unwrap();
    let rules2 = list_middleware_rules(&db).await.unwrap();
    assert_eq!(
        rules2.iter().filter(|r| r.is_builtin).count(),
        rules.iter().filter(|r| r.is_builtin).count(),
        "seed 不重复 INSERT 内置行"
    );
}

/// 还原默认：applies_to 一并回 '{}'（INSERT 硬编码初值）。
#[tokio::test]
async fn reset_restores_applies_to_to_default() {
    let (db, _dir) = test_support::three_file_db().await;
    let rules = list_middleware_rules(&db).await.unwrap();
    let r = rules.iter().find(|r| r.is_builtin).unwrap().clone();
    let mut applies = r.applies_to.clone();
    applies.platforms = vec![999];
    update_middleware_rule(
        &db,
        crate::models::UpdateMiddlewareRule {
            id: r.id,
            name: r.name.clone(),
            description: r.description.clone(),
            conditions: r.conditions.clone(),
            actions: r.actions.clone(),
            applies_to: applies,
            priority: r.priority,
            enabled: r.enabled,
        },
    )
    .await
    .unwrap();
    let out = reset_middleware_rule_to_builtin(&db, r.id).await.unwrap();
    assert_eq!(
        serde_json::to_value(&out.applies_to).unwrap(),
        serde_json::json!({"platforms": [], "groups": [], "models": []}),
        "还原后 applies_to 回默认"
    );
    assert!(!out.user_modified);
}

/// 用户把内容改回与内置规格一致 → user_modified 清零（恢复随升级被默认覆盖）。
#[tokio::test]
async fn builtin_edit_back_to_spec_clears_user_modified() {
    let (db, _dir) = test_support::three_file_db().await;
    let rules = list_middleware_rules(&db).await.unwrap();
    let r = rules.iter().find(|r| r.name == "内置·密钥脱敏").unwrap().clone();
    let orig = r.clone();
    // 改内容 → 置标记
    let mut fixed = r.actions.clone();
    fixed[0].params.mode = crate::models::MaskMode::Fixed;
    fixed[0].params.fields = vec!["messages".into()];
    let out = update_middleware_rule(
        &db,
        crate::models::UpdateMiddlewareRule {
            id: r.id,
            name: r.name.clone(),
            description: r.description.clone(),
            conditions: r.conditions.clone(),
            actions: fixed,
            applies_to: r.applies_to.clone(),
            priority: r.priority,
            enabled: r.enabled,
        },
    )
    .await
    .unwrap();
    assert!(out.user_modified, "内容编辑置 user_modified");
    // 改回规格内容 → 清标记
    let back = update_middleware_rule(
        &db,
        crate::models::UpdateMiddlewareRule {
            id: r.id,
            name: orig.name.clone(),
            description: orig.description.clone(),
            conditions: orig.conditions.clone(),
            actions: orig.actions.clone(),
            applies_to: orig.applies_to.clone(),
            priority: orig.priority,
            enabled: r.enabled,
        },
    )
    .await
    .unwrap();
    assert!(!back.user_modified, "改回规格内容清 user_modified");
}

// ── 加分项：fixed + all 在创建/更新边界被拒 ──
fn mask_step(mode: crate::models::MaskMode, fields: &[&str]) -> crate::models::ActionStep {
    let p = crate::models::ActionParams {
        mode,
        fields: fields.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    crate::models::ActionStep { kind: crate::models::ActionKind::Mask, params: p }
}

#[tokio::test]
async fn create_and_update_reject_fixed_mask_with_all() {
    use crate::models::MaskMode::{Fixed, Reversible};
    let (db, _dir) = test_support::three_file_db().await;
    let conds: crate::models::ConditionNode = serde_json::from_str(
        r#"{"kind":"any","children":[{"kind":"leaf","target":"request_body","field":"","match_type":"regex","pattern":"x+"}]}"#,
    )
    .unwrap();
    let mk = |actions| crate::models::CreateMiddlewareRule {
        name: "u".into(),
        description: String::new(),
        conditions: conds.clone(),
        actions,
        applies_to: Default::default(),
        priority: 1,
        enabled: true,
        is_builtin: false,
    };
    for fields in [&["all"][..], &["messages", "all"][..]] {
        let err = create_middleware_rule(&db, mk(vec![mask_step(Fixed, fields)])).await.unwrap_err();
        assert!(err.contains("reversible"), "{err}");
    }
    assert!(list_middleware_rules(&db).await.unwrap().iter().all(|r| r.name != "u"));
    let mut bad = mask_step(Reversible, &["all"]);
    bad.params.placeholder_type = "lower".into();
    assert!(create_middleware_rule(&db, mk(vec![bad])).await.unwrap_err().contains("placeholder_type"));
    // 合法：fixed + messages；reversible + all
    let ok1 = create_middleware_rule(&db, mk(vec![mask_step(Fixed, &["messages"])])).await.unwrap();
    create_middleware_rule(&db, mk(vec![mask_step(Reversible, &["all"])])).await.unwrap();
    // 更新为 fixed + all 被拒
    let upd = crate::models::UpdateMiddlewareRule {
        id: ok1.id,
        name: "u".into(),
        description: String::new(),
        conditions: conds.clone(),
        actions: vec![mask_step(Fixed, &["all"])],
        applies_to: Default::default(),
        priority: 1,
        enabled: true,
    };
    assert!(update_middleware_rule(&db, upd).await.unwrap_err().contains("reversible"));
}
