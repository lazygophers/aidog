#![cfg(test)]
use super::test_support::*;
use super::*;
use rusqlite::params;

/// endpoints 反序列化容错：DB 中含未知 client_type（如旧数据 "anthropic"）的
/// endpoint 数组应仍能完整解析。ClientType = String 后未知值原值保留（arbitrary），
/// 仅空串 / null 归一化为 "default"（`deserialize_client_type_lenient`）。
#[tokio::test]
async fn endpoints_with_unknown_client_type_still_parse() {
    let json = r#"[{"protocol":"openai","base_url":"https://x/v1","client_type":"codex_tui","coding_plan":false},{"protocol":"anthropic","base_url":"https://x/anthropic","client_type":"anthropic","coding_plan":false}]"#;
    let parsed = parse_endpoints(json);
    assert_eq!(parsed.len(), 2, "未知 client_type 不应导致整个数组解析失败");
    // String arbitrary：未知值原值保留（不再回退 Default）
    assert_eq!(
        parsed[1].client_type, "anthropic",
        "未知 client_type 原值保留"
    );
    assert_eq!(parsed[1].protocol, Protocol::Anthropic);

    // 端到端：写入 DB 后 list_platforms 应带回 endpoints
    let db = test_db().await;
    let mut input = sample_platform("p1");
    input.endpoints = Some(vec![PlatformEndpoint {
        protocol: Protocol::OpenAI,
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
        client_type: "codex_tui".to_string(),
        coding_plan: true,
    }]);
    create_platform(&db, input).await.unwrap();
    let listed = list_platforms(&db).await.unwrap();
    assert_eq!(
        listed[0].endpoints.len(),
        1,
        "list_platforms 应返回 endpoints"
    );
}

// ── R3 platform_type 列（protocol 改名）往返 ──
#[tokio::test]
async fn r3_platform_type_roundtrip() {
    let db = test_db().await;
    let mut input = sample_platform("pt");
    input.platform_type = Protocol::Glm;
    let p = create_platform(&db, input).await.unwrap();
    let fetched = get_platform(&db, p.id).await.unwrap().unwrap();
    assert_eq!(fetched.platform_type, Protocol::Glm);
    // 列名为 platform_type（间接：能写入该列即证明列存在）
    let pid = p.id as i64;
    let stored: String = db
        .call_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn.query_row(
                "SELECT platform_type FROM platform WHERE id = ?1",
                params![pid],
                |r| r.get(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(stored, "\"glm\"");
}

/// 回归：DB 里存了**裸** wire 名（无 JSON 引号，外部工具 / 老 seed SQL 直写）时，
/// 读行不得 panic —— 旧实现 `serde_json::from_str(..).unwrap()` 会杀死 tokio-rusqlite
/// 后台线程，该连接此后永久返 ConnectionClosed，platform 读池逐条耗尽后
/// `get_group_platforms` 全失败 → 所有代理请求 400 route error（2026-08-28 现场）。
#[tokio::test]
async fn bare_platform_type_does_not_kill_db_thread() {
    let db = test_db().await;
    let mut input = sample_platform("bare");
    input.platform_type = Protocol::Glm;
    let p = create_platform(&db, input).await.unwrap();
    let pid = p.id as i64;
    // 绕过写路径，直接把列改成裸值（复现脏数据形态）
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "UPDATE platform SET platform_type = 'minimax_coding' WHERE id = ?1",
            params![pid],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    db.invalidate_hot_caches();

    let fetched = get_platform(&db, p.id).await.unwrap().unwrap();
    assert_eq!(
        fetched.platform_type,
        Protocol::MinimaxCoding,
        "裸 wire 名应被认出"
    );
    // 连接仍存活：后续查询不返 ConnectionClosed
    assert!(
        !list_platforms(&db).await.unwrap().is_empty(),
        "DB 线程应仍存活"
    );
}

// ── S1 async DB：增删改查全路径（内存库，验证 tokio-rusqlite 闭包往返）──
#[tokio::test]
async fn s1_async_platform_crud_roundtrip() {
    let db = test_db().await;
    // create
    let mut input = sample_platform("crud");
    input.base_url = "https://crud.example/v1".to_string();
    let created = create_platform(&db, input).await.unwrap();
    assert!(created.id >= 1);

    // read (list + get)
    assert_eq!(list_platforms(&db).await.unwrap().len(), 1);
    let got = get_platform(&db, created.id).await.unwrap().unwrap();
    assert_eq!(got.base_url, "https://crud.example/v1");

    // update
    let updated = update_platform(
        &db,
        UpdatePlatform {
            id: created.id,
            name: None,
            platform_type: None,
            base_url: Some("https://crud.example/v2".to_string()),
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
        quota_source: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.base_url, "https://crud.example/v2");
    assert_eq!(
        get_platform(&db, created.id)
            .await
            .unwrap()
            .unwrap()
            .base_url,
        "https://crud.example/v2"
    );

    // delete（软删）→ list 不含、get None
    delete_platform(&db, created.id).await.unwrap();
    assert_eq!(list_platforms(&db).await.unwrap().len(), 0);
    assert!(get_platform(&db, created.id).await.unwrap().is_none());
}

// ── S1 async DB：OptionalExtension 路径（query_row().optional() 在闭包内）──
#[tokio::test]
async fn s1_async_optional_extension_returns_none_for_missing() {
    let db = test_db().await;
    // 不存在的 id → get_platform 走 query_row().optional() 返回 None（非 Err）
    assert!(get_platform(&db, 99_999).await.unwrap().is_none());
    // 存在则返回 Some
    let p = create_platform(&db, sample_platform("opt")).await.unwrap();
    assert!(get_platform(&db, p.id).await.unwrap().is_some());
    // get_setting 同样走 optional()：缺键 None
    assert!(get_setting(&db, "nope", "nope").await.unwrap().is_none());
}

#[tokio::test]
async fn platform_breaker_roundtrips_via_extra() {
    let db = test_db().await;
    // create 时把 breaker 覆盖写进 extra.breaker，读回经 Platform::breaker() 解析一致。
    let mut input = sample_platform("brk");
    input.extra = crate::models::merge_breaker_into_extra(
        "{}",
        &crate::models::PlatformBreaker {
            failure_threshold: 7,
            open_secs: 120,
            half_open_max: 3,
        },
    );
    let p = create_platform(&db, input).await.unwrap();
    let got = get_platform(&db, p.id).await.unwrap().unwrap();
    let b = got.breaker();
    assert_eq!(b.failure_threshold, 7);
    assert_eq!(b.open_secs, 120);
    assert_eq!(b.half_open_max, 3);

    // 缺省（空 extra）→ 全 0（继承全局默认）。
    let p2 = create_platform(&db, sample_platform("brk-default"))
        .await
        .unwrap();
    let b2 = get_platform(&db, p2.id).await.unwrap().unwrap().breaker();
    assert_eq!(
        (b2.failure_threshold, b2.open_secs, b2.half_open_max),
        (0, 0, 0)
    );

    // update 改 extra → breaker 跟随更新。
    let cleared = crate::models::merge_breaker_into_extra(
        &got.extra,
        &crate::models::PlatformBreaker::default(),
    );
    update_platform(
        &db,
        UpdatePlatform {
            id: p.id,
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: Some(cleared),
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
        quota_source: None,
        },
    )
    .await
    .unwrap();
    let b3 = get_platform(&db, p.id).await.unwrap().unwrap().breaker();
    assert_eq!(
        (b3.failure_threshold, b3.open_secs, b3.half_open_max),
        (0, 0, 0),
        "clear breaker via extra"
    );
}

#[tokio::test]
async fn censorship_block_auto_disables_for_one_hour() {
    let db = test_db().await;
    let p = create_platform(&db, sample_platform("censorship")).await.unwrap();
    let before = now();

    disable_platform_for_censorship(&db, p.id).await.unwrap();

    let disabled = get_platform(&db, p.id).await.unwrap().unwrap();
    assert_eq!(disabled.status, PlatformStatus::AutoDisabled);
    assert!(!disabled.enabled);
    assert!(disabled.auto_disabled_until >= before + 60 * 60 * 1000);
}

/// R5（2026-09-28）：auto_disable 写入路径已删（原 set_platform_auto_disabled），本测试守
/// 读兼容的 recover 路径——存量 auto_disabled 行成功后恢复 enabled、清 strikes/until。
#[tokio::test]
async fn auto_disable_recover_clears_state() {
    let db = test_db().await;
    let p = create_platform(&db, sample_platform("ad")).await.unwrap();
    assert_eq!(p.status, PlatformStatus::Enabled);

    // 造存量行（原退避迭代语义随写入路径一起删除，不再测）
    set_legacy_auto_disabled(&db, p.id, 3, now() + 4 * 3_600_000).await;
    let g = get_platform(&db, p.id).await.unwrap().unwrap();
    assert_eq!(g.status, PlatformStatus::AutoDisabled);
    assert!(!g.enabled, "auto_disabled 平台 enabled 列同步为 false");
    assert_eq!(g.auto_disable_strikes, 3);

    // 2xx 恢复：清状态
    recover_platform_auto_disabled(&db, p.id).await.unwrap();
    let g4 = get_platform(&db, p.id).await.unwrap().unwrap();
    assert_eq!(g4.status, PlatformStatus::Enabled);
    assert!(g4.enabled);
    assert_eq!(g4.auto_disable_strikes, 0);
    assert_eq!(g4.auto_disabled_until, 0);
}

/// 改 api_key 自恢复：auto_disabled 平台改 api_key → 立即恢复 enabled 清退避。
#[tokio::test]
async fn api_key_change_recovers_auto_disabled() {
    let db = test_db().await;
    let p = create_platform(&db, sample_platform("rk")).await.unwrap();
    set_legacy_auto_disabled(&db, p.id, 1, now() + 3_600_000).await;
    assert_eq!(
        get_platform(&db, p.id).await.unwrap().unwrap().status,
        PlatformStatus::AutoDisabled
    );

    // 改 api_key（不显式传 status）
    let upd = update_platform(
        &db,
        UpdatePlatform {
            id: p.id,
            name: None,
            platform_type: None,
            base_url: None,
            api_key: Some("sk-new-key".to_string()),
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
        quota_source: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(upd.status, PlatformStatus::Enabled, "改 api_key 立即恢复");
    assert_eq!(upd.auto_disable_strikes, 0);
    assert_eq!(upd.auto_disabled_until, 0);
}

// ── quota-scripts T4：quota_script 物化列随 create/update 的写读规则 ──

#[tokio::test]
async fn create_materializes_first_variant() {
    let db = test_db().await;
    let mut input = sample_platform("quota-glm");
    input.platform_type = Protocol::Glm;
    input.extra = String::new();
    let created = create_platform(&db, input).await.unwrap();
    let first = registry::quota_scripts_in(registry::presets(), "glm")[0]
        .script
        .clone();
    assert_eq!(created.quota_script, first, "创建即物化首条变体（零配置）");
    assert_eq!(
        get_platform(&db, created.id)
            .await
            .unwrap()
            .unwrap()
            .quota_script,
        first,
        "列持久化"
    );
}

#[tokio::test]
async fn create_without_scripts_keeps_empty() {
    let db = test_db().await;
    let mut input = sample_platform("quota-none");
    input.platform_type = Protocol::OpenAI;
    let created = create_platform(&db, input).await.unwrap();
    assert_eq!(created.quota_script, "", "无脚本协议 → 空列");
}

#[tokio::test]
async fn update_pins_custom_and_rematrializes_on_type_change() {
    let db = test_db().await;
    let mut input = sample_platform("quota-upd");
    input.platform_type = Protocol::DeepSeek;
    input.extra = String::new();
    let created = create_platform(&db, input).await.unwrap();
    let ds = created.quota_script.clone();
    assert!(!ds.is_empty());

    // 无 id + 列非空 + 协议未变 → 保留（远程同步不自动换已物化脚本）
    let upd = update_platform(
        &db,
        UpdatePlatform {
            id: created.id,
            name: Some("n2".into()),
            ..empty_update()
        },
    )
    .await
    .unwrap();
    assert_eq!(upd.quota_script, ds);

    // extra.quota_custom_script → 物化用户手写脚本
    let upd2 = update_platform(
        &db,
        UpdatePlatform {
            id: created.id,
            extra: Some(r#"{"quota_custom_script":"return 1"}"#.into()),
            ..empty_update()
        },
    )
    .await
    .unwrap();
    assert_eq!(upd2.quota_script, "return 1");

    // id 失效（远程改名/删条）→ 回落首条重物化
    let upd3 = update_platform(
        &db,
        UpdatePlatform {
            id: created.id,
            extra: Some(r#"{"quota_script_id":"gone"}"#.into()),
            ..empty_update()
        },
    )
    .await
    .unwrap();
    assert_eq!(upd3.quota_script, ds, "deepseek 单变体，id 失效回落首条");

    // 协议变更 → 重物化（目标协议无脚本 → 清列）
    let upd4 = update_platform(
        &db,
        UpdatePlatform {
            id: created.id,
            platform_type: Some(Protocol::OpenAI),
            ..empty_update()
        },
    )
    .await
    .unwrap();
    assert_eq!(upd4.quota_script, "");
}

/// 最小 UpdatePlatform 骨架（全部 None = 不动）。
fn empty_update() -> UpdatePlatform {
    UpdatePlatform {
        id: 0,
        name: None,
        platform_type: None,
        base_url: None,
        api_key: None,
        extra: None,
        models: None,
        available_models: None,
        endpoints: None,
        enabled: None,
        status: None,
        manual_budgets: None,
        join_group_ids: None,
        expires_at: None,
        quota_source: None,
    }
}

// ── quota_source 互斥（quota-ia spec §1，票 01/09）──

/// 切 manual：extra 两键剥掉 + 物化列清空 + 预算保留；切回 auto：预算清空、脚本侧按现配置重物化。
#[tokio::test]
async fn quota_source_switch_clears_opposite_side() {
    let db = test_support::test_db().await;
    let budget = crate::models::ManualBudget {
        id: "b1".into(),
        amount: 5.0,
        consumed: 0.0,
        enabled: true,
        kind: "total".into(),
        unit: "usd".into(),
        window_hours: None,
        window_unit: Default::default(),
        window_start_at: Some(0),
    };
    // deepseek 协议带内置 quota 变体 → 显式传 manual 验证创建即互斥。
    let created = create_platform(
        &db,
        CreatePlatform {
            name: "m".into(),
            platform_type: Protocol::DeepSeek,
            base_url: "https://ex.com".into(),
            api_key: "k".into(),
            extra: r#"{"quota_script_id":"default","breaker":{"threshold":3}}"#.into(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: Some(vec![budget.clone()]),
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: Some("manual".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(created.quota_source, "manual");
    assert!(created.extra.contains("\"threshold\":3"), "非 quota 键保留");
    assert!(
        !created.extra.contains("quota_script_id"),
        "manual 创建即剥脚本侧 extra 键"
    );
    assert!(created.quota_script.is_empty(), "manual 物化列恒空");
    assert_eq!(created.manual_budgets.len(), 1, "manual 预算保留");

    // 切回 auto：预算清空
    let mut upd = empty_update();
    upd.id = created.id;
    upd.quota_source = Some("auto".into());
    let updated = update_platform(&db, upd).await.unwrap();
    assert_eq!(updated.quota_source, "auto");
    assert!(updated.manual_budgets.is_empty(), "切 auto 清预算");

    // None = 不动：source 与两侧数据保持
    let mut keep = empty_update();
    keep.id = created.id;
    keep.manual_budgets = Some(vec![budget]);
    let again = update_platform(&db, keep).await.unwrap();
    assert_eq!(again.quota_source, "auto", "None 不改 source");
    assert_eq!(again.manual_budgets.len(), 1);

    // 空串/未知值归一为 auto；存量空串行读侧当 auto
    assert_eq!(normalize_quota_source(""), "auto");
    assert_eq!(normalize_quota_source("auto"), "auto");
    assert_eq!(normalize_quota_source("manual"), "manual");
    assert_eq!(normalize_quota_source("garbage"), "auto");
    assert!(is_manual_quota_source("manual"));
    assert!(!is_manual_quota_source(""));
    // strip：无键 / 非 JSON 原样
    assert_eq!(strip_quota_script_extra_keys(r#"{"a":1}"#), r#"{"a":1}"#);
    assert_eq!(strip_quota_script_extra_keys("not-json"), "not-json");
}

/// 新建缺省 source：按协议能力推断（anthropic 有内置变体 → auto；无变体协议 → manual）。
#[tokio::test]
async fn quota_source_default_by_protocol_capability() {
    let db = test_support::test_db().await;
    let mk = |proto: Protocol| {
        CreatePlatform {
            name: "d".into(),
            platform_type: proto,
            base_url: "https://ex.com".into(),
            api_key: "k".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        }
    };
    // deepseek：registry 带内置 quota 变体 → auto（开箱即查保持）
    let a = create_platform(&db, mk(Protocol::DeepSeek)).await.unwrap();
    assert_eq!(a.quota_source, "auto", "有变体协议默认 auto");
    // openai_completions：无内置变体 → manual（票 01 新建默认）
    let o = create_platform(&db, mk(Protocol::OpenAICompletions)).await.unwrap();
    assert_eq!(o.quota_source, "manual", "无变体协议默认 manual");
}

// ── platform.name 唯一性（platform-name-unique，2026-09-28）──

#[tokio::test]
async fn create_duplicate_name_gets_suffix() {
    let db = test_db().await;
    let p1 = create_platform(&db, sample_platform("dup")).await.unwrap();
    let p2 = create_platform(&db, sample_platform("dup")).await.unwrap();
    assert_eq!(p1.name, "dup", "首个保留原名");
    assert!(
        p2.name.starts_with("dup-") && p2.name.len() == "dup".len() + 9,
        "第二个自动加 8 位随机后缀，实际 {}",
        p2.name
    );
    assert_ne!(p1.name, p2.name);
}

#[tokio::test]
async fn update_rename_to_existing_gets_suffix() {
    let db = test_db().await;
    let a = create_platform(&db, sample_platform("aaa")).await.unwrap();
    let b = create_platform(&db, sample_platform("bbb")).await.unwrap();
    // b 改名成 a 的名字 → 自动后缀
    let upd = UpdatePlatform {
        id: b.id,
        name: Some("aaa".to_string()),
        platform_type: None,
        base_url: None,
        api_key: None,
        extra: None,
        models: None,
        available_models: None,
        endpoints: None,
        enabled: None,
        status: None,
        manual_budgets: None,
        join_group_ids: None,
        expires_at: None,
        quota_source: None,
    };
    let r = update_platform(&db, upd).await.unwrap();
    assert!(r.name.starts_with("aaa-"), "撞名自动后缀，实际 {}", r.name);
    assert_eq!(
        get_platform(&db, a.id).await.unwrap().unwrap().name,
        "aaa",
        "原持有者不动"
    );
    // 平台 a 改回自己已有的名字（aaa → aaa）不触发后缀
    let upd2 = UpdatePlatform {
        id: a.id,
        name: Some("aaa".to_string()),
        platform_type: None,
        base_url: None,
        api_key: None,
        extra: None,
        models: None,
        available_models: None,
        endpoints: None,
        enabled: None,
        status: None,
        manual_budgets: None,
        join_group_ids: None,
        expires_at: None,
        quota_source: None,
    };
    let r2 = update_platform(&db, upd2).await.unwrap();
    assert_eq!(r2.name, "aaa", "改回已有自己的名字不受影响");
}

#[tokio::test]
async fn migration_dedupes_existing_names() {
    let db = test_db().await;
    // 造 3 个重名（绕过 create 的唯一性检查，直接走存量行 fixture 语义）
    let p1 = create_platform(&db, sample_platform("legacy")).await.unwrap();
    let _p2 = create_platform(&db, sample_platform("legacy-x")).await.unwrap();
    let _p3 = create_platform(&db, sample_platform("legacy-y")).await.unwrap();
    // 手工改回重名，模拟升级前存量
    db.call_platform_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute("UPDATE platform SET name = 'legacy' WHERE id IN (?1, ?2)", rusqlite::params![_p2.id as i64, _p3.id as i64])?;
        Ok(())
    }).await.unwrap();
    // 跑 migration（幂等可重放）
    db.call_platform_traced(None, std::panic::Location::caller(), move |conn| {
        crate::schema_late::run_migrations_platform_late(conn)?;
        Ok(())
    }).await.unwrap();
    let n1 = get_platform(&db, p1.id).await.unwrap().unwrap().name;
    let n2 = get_platform(&db, _p2.id).await.unwrap().unwrap().name;
    let n3 = get_platform(&db, _p3.id).await.unwrap().unwrap().name;
    let names = [n1.clone(), n2, n3];
    assert_eq!(n1, "legacy", "最小 id 保留原名");
    assert!(names.iter().all(|n| n == "legacy" || n.starts_with("legacy-")));
    assert_eq!(names.iter().collect::<std::collections::HashSet<_>>().len(), 3, "全部唯一");
}

// ── 粘贴 key 带首尾空白（如尾部换行）：create/update 入口剥净，
// 否则 reqwest 报 builder error: failed to parse header value（proxy_log 5ba9be8e 实证） ──
#[tokio::test]
async fn api_key_whitespace_trimmed_on_create_and_update() {
    let db = test_db().await;
    let mut input = sample_platform("trim-key");
    input.api_key = "  sk-live\tline\n".to_string();
    let p = create_platform(&db, input).await.unwrap();
    assert_eq!(p.api_key, "sk-live\tline");

    let updated = update_platform(&db, UpdatePlatform {
        id: p.id,
        api_key: Some("sk-new\n".to_string()),
        name: None,
        platform_type: None,
        base_url: None,
        extra: None,
        models: None,
        available_models: None,
        endpoints: None,
        enabled: None,
        status: None,
        manual_budgets: None,
        join_group_ids: None,
        expires_at: None,
        quota_source: None,
    }).await.unwrap();
    assert_eq!(updated.api_key, "sk-new");
}
