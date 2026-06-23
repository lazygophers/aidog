//! 导入应用器端到端：collect → encrypt → preview → apply 全链路（HOME 隔离）。
//! 覆盖 apply_files / apply_db / db_rows upsert / conflicts 检测 / resolve_name 决策分支。
//! 本模块经 #[path] 挂在 apply/mod.rs，故 `super` = apply 模块，可直调 preview/apply。
use super::{apply, preview};
use crate::gateway::db::test_support::{sample_group, sample_platform, test_db, HomeGuard};
use crate::gateway::import_export::{
    collect, container, ConflictDecision, Decision, Manifest, NamedText, Payload, SCOPE_CLAUDE_CODE,
    SCOPE_CODEX, SCOPE_GROUP, SCOPE_GROUP_PLATFORM, SCOPE_PLATFORM, SCOPE_SETTING,
};
use crate::gateway::models::GroupPlatformInput;

fn blank_manifest(scopes: Vec<String>) -> Manifest {
    Manifest {
        format_version: 1,
        aidog_version: "test".into(),
        created_at: "now".into(),
        source_machine: "m".into(),
        scopes,
        checksum: String::new(),
    }
}

fn empty_payload(scopes: Vec<String>) -> Payload {
    Payload {
        manifest: blank_manifest(scopes),
        platform: vec![],
        group: vec![],
        group_platform: vec![],
        setting: vec![],
        codex_global: None,
        codex_profiles: vec![],
        claude_code_global: None,
        claude_code_group_settings: vec![],
        skills: vec![],
    }
}

async fn seed_source(db: &crate::gateway::db::Db) {
    let plat = crate::gateway::db::create_platform(db, sample_platform("psrc"))
        .await
        .unwrap();
    let grp = crate::gateway::db::create_group(db, sample_group("gsrc", vec![]))
        .await
        .unwrap();
    crate::gateway::db::set_group_platforms(
        db,
        grp.id,
        &[GroupPlatformInput {
            platform_id: plat.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();
    crate::gateway::db::set_setting(
        db,
        crate::gateway::models::SetSettingInput {
            scope: "app".into(),
            key: "locale".into(),
            value: serde_json::json!({"locale": "zh-CN"}),
        },
    )
    .await
    .unwrap();
}

fn scopes() -> Vec<String> {
    [
        SCOPE_PLATFORM,
        SCOPE_GROUP,
        SCOPE_GROUP_PLATFORM,
        SCOPE_SETTING,
        SCOPE_CLAUDE_CODE,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[tokio::test]
async fn full_collect_apply_into_fresh_db() {
    let h = HomeGuard::new();
    // claude_code 全局文件存在 → 走 apply_files
    let claude_dir = h.home().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("settings.json"), r#"{"model":"opus"}"#).unwrap();

    let src = test_db().await;
    seed_source(&src).await;
    let payload = collect::collect(&src, &scopes()).await.unwrap();

    let target = test_db().await;
    let report = apply(payload, &[], &target).await.unwrap();
    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
    assert_eq!(*report.applied.get(SCOPE_PLATFORM).unwrap(), 1);
    assert_eq!(*report.applied.get(SCOPE_GROUP).unwrap(), 1);

    assert_eq!(
        crate::gateway::db::list_platforms(&target)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        crate::gateway::db::list_groups(&target).await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn preview_roundtrip_via_encrypt() {
    let _h = HomeGuard::new();
    let src = test_db().await;
    seed_source(&src).await;
    let mut payload = collect::collect(&src, &scopes()).await.unwrap();
    let plain = payload.serialize_with_checksum().unwrap();
    let cipher = container::encrypt(&plain).unwrap();

    let target = test_db().await;
    let pv = preview(&cipher, &target).await.unwrap();
    assert!(pv.counts.get(SCOPE_PLATFORM).copied().unwrap_or(0) >= 1);
    // 空目标库 → 无冲突
    assert!(pv.conflicts.is_empty());
}

#[tokio::test]
async fn apply_with_skip_and_rename_decisions() {
    let _h = HomeGuard::new();
    let src = test_db().await;
    seed_source(&src).await;
    let payload = collect::collect(&src, &scopes()).await.unwrap();

    let target = test_db().await;
    let decisions = vec![
        ConflictDecision {
            scope: SCOPE_GROUP.into(),
            key: "gsrc".into(),
            decision: Decision::Skip,
        },
        ConflictDecision {
            scope: SCOPE_PLATFORM.into(),
            key: "psrc".into(),
            decision: Decision::Rename {
                new_key: "prenamed".into(),
            },
        },
    ];
    let report = apply(payload, &decisions, &target).await.unwrap();
    assert_eq!(*report.skipped.get(SCOPE_GROUP).unwrap(), 1);
    let plats = crate::gateway::db::list_platforms(&target).await.unwrap();
    assert!(plats.iter().any(|p| p.name == "prenamed"));
}

#[tokio::test]
async fn overwrite_existing_group_updates_cols() {
    let _h = HomeGuard::new();
    let src = test_db().await;
    seed_source(&src).await;
    let payload = collect::collect(&src, &scopes()).await.unwrap();

    // 目标库已有同 group_key 的分组 → Overwrite 决策走 update_group_cols 分支
    let target = test_db().await;
    seed_source(&target).await;
    let before = crate::gateway::db::list_groups(&target).await.unwrap();
    let before_id = before.iter().find(|g| g.group_key == "gsrc").unwrap().id;

    let decisions = vec![ConflictDecision {
        scope: SCOPE_GROUP.into(),
        key: "gsrc".into(),
        decision: Decision::Overwrite,
    }];
    let report = apply(payload, &decisions, &target).await.unwrap();
    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);

    // 同 group_key 行被原地更新（id 不变，未新增分组）
    let after = crate::gateway::db::list_groups(&target).await.unwrap();
    assert_eq!(after.iter().filter(|g| g.group_key == "gsrc").count(), 1);
    assert!(after.iter().any(|g| g.id == before_id));
}

/// 文件类 scope（codex_global + codex_profiles + claude_code_group_settings）端到端 +
/// 已存在文件触发 backup_and_write 备份分支 + setting 应用 + setting-skip 决策。
#[tokio::test]
async fn apply_file_scopes_and_setting_skip() {
    let h = HomeGuard::new();
    // 预置已存在的 codex config.toml → 触发 backup 分支
    let codex_dir = h.home().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(codex_dir.join("config.toml"), "old = 1\n").unwrap();

    let mut payload = empty_payload(vec![SCOPE_CODEX.into(), SCOPE_SETTING.into()]);
    payload.codex_global = Some("model = \"o3\"\n".into());
    payload.codex_profiles = vec![NamedText {
        name: "myprofile".into(),
        text: "x = 1\n".into(),
    }];
    payload.claude_code_group_settings = vec![NamedText {
        name: "team".into(),
        text: r#"{"model":"opus"}"#.into(),
    }];
    payload.setting = vec![
        ["app".into(), "locale".into(), "{\"locale\":\"en\"}".into()],
        ["app".into(), "theme".into(), "{\"t\":\"dark\"}".into()],
    ];

    // setting app:theme 决策 Skip
    let decisions = vec![ConflictDecision {
        scope: SCOPE_SETTING.into(),
        key: "app:theme".into(),
        decision: Decision::Skip,
    }];

    let target = test_db().await;
    let report = apply(payload, &decisions, &target).await.unwrap();
    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
    assert_eq!(*report.skipped.get(SCOPE_SETTING).unwrap(), 1);
    assert_eq!(*report.applied.get(SCOPE_SETTING).unwrap(), 1);
    assert!(*report.applied.get(SCOPE_CODEX).unwrap() >= 2);

    // 文件已落地
    assert!(codex_dir.join("config.toml").exists());
    // backup 已生成
    assert!(codex_dir.join("config.toml.aidogbak").exists());
    assert!(h
        .home()
        .join(".aidog/settings.team.json")
        .exists());
    // setting locale 入库，theme 被跳过
    let locale = crate::gateway::db::get_setting(&target, "app", "locale")
        .await
        .unwrap();
    assert!(locale.is_some());
    let theme = crate::gateway::db::get_setting(&target, "app", "theme")
        .await
        .unwrap();
    assert!(theme.is_none());
}

/// db_rows::snapshot_platform_ids + ensure_group_and_attach（create-new 分支）+ relink。
#[tokio::test]
async fn ensure_group_attach_creates_and_links() {
    use super::db_rows;
    let _h = HomeGuard::new();
    let db = test_db().await;

    // before 快照（空库）
    let before = db_rows::snapshot_platform_ids(&db).await.unwrap();
    assert!(before.is_empty());

    // 新建一个平台（在快照之后）→ 进入 new_ids 差集
    let plat = crate::gateway::db::create_platform(&db, sample_platform("attachp"))
        .await
        .unwrap();

    // ensure group by name（不存在 → create 生成 gk_）+ attach 新平台
    db_rows::ensure_group_and_attach(&db, "autogrp", &before)
        .await
        .unwrap();

    let groups = crate::gateway::db::list_groups(&db).await.unwrap();
    let g = groups.iter().find(|g| g.name == "autogrp").unwrap();
    assert!(g.group_key.starts_with("gk_"));
    // 关联已建立
    let detail = crate::gateway::db::get_group_detail(&db, g.id)
        .await
        .unwrap()
        .unwrap();
    assert!(detail.platforms.iter().any(|p| p.platform.id == plat.id));
}

/// ensure_group_and_attach 第二次同名 → 命中复用分支（不重复建组）。
#[tokio::test]
async fn ensure_group_attach_reuses_existing_name() {
    use super::db_rows;
    let _h = HomeGuard::new();
    let db = test_db().await;
    let before = db_rows::snapshot_platform_ids(&db).await.unwrap();
    crate::gateway::db::create_platform(&db, sample_platform("p1"))
        .await
        .unwrap();
    db_rows::ensure_group_and_attach(&db, "dup", &before).await.unwrap();
    db_rows::ensure_group_and_attach(&db, "dup", &before).await.unwrap();
    let groups = crate::gateway::db::list_groups(&db).await.unwrap();
    assert_eq!(groups.iter().filter(|g| g.name == "dup").count(), 1);
}

/// relink_group_platform：成功（组+平台都在）+ 缺失报错两分支。
#[tokio::test]
async fn relink_success_and_missing() {
    use super::db_rows;
    let _h = HomeGuard::new();
    let db = test_db().await;
    let plat = crate::gateway::db::create_platform(&db, sample_platform("rp"))
        .await
        .unwrap();
    let grp = crate::gateway::db::create_group(&db, sample_group("rg", vec![]))
        .await
        .unwrap();
    // relink 按 name 查 group（参数名 group_key 实为 name）
    db_rows::relink_group_platform(&db, "rg", "rp").await.unwrap();
    let detail = crate::gateway::db::get_group_detail(&db, grp.id)
        .await
        .unwrap()
        .unwrap();
    assert!(detail.platforms.iter().any(|p| p.platform.id == plat.id));

    // 缺失 → Err
    let err = db_rows::relink_group_platform(&db, "nope", "rp").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn conflict_detected_on_existing_group() {
    let _h = HomeGuard::new();
    let src = test_db().await;
    seed_source(&src).await;
    let mut payload = collect::collect(&src, &scopes()).await.unwrap();
    let plain = payload.serialize_with_checksum().unwrap();
    let cipher = container::encrypt(&plain).unwrap();

    // 目标库预置同名 group/platform → preview 应报冲突
    let target = test_db().await;
    seed_source(&target).await;
    let pv = preview(&cipher, &target).await.unwrap();
    assert!(!pv.conflicts.is_empty());
}
