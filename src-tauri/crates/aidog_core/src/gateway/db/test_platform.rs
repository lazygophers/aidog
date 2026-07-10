#![cfg(test)]
use super::*;
use super::test_support::*;
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
        assert_eq!(parsed[1].client_type, "anthropic", "未知 client_type 原值保留");
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
        assert_eq!(listed[0].endpoints.len(), 1, "list_platforms 应返回 endpoints");
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
        let stored: String = db.call_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn.query_row("SELECT platform_type FROM platform WHERE id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(stored, "\"glm\"");
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
        let updated = update_platform(&db, UpdatePlatform {
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
        }).await.unwrap();
        assert_eq!(updated.base_url, "https://crud.example/v2");
        assert_eq!(get_platform(&db, created.id).await.unwrap().unwrap().base_url, "https://crud.example/v2");

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
        input.extra = crate::gateway::models::merge_breaker_into_extra(
            "{}",
            &crate::gateway::models::PlatformBreaker {
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
        let p2 = create_platform(&db, sample_platform("brk-default")).await.unwrap();
        let b2 = get_platform(&db, p2.id).await.unwrap().unwrap().breaker();
        assert_eq!((b2.failure_threshold, b2.open_secs, b2.half_open_max), (0, 0, 0));

        // update 改 extra → breaker 跟随更新。
        let cleared = crate::gateway::models::merge_breaker_into_extra(&got.extra, &crate::gateway::models::PlatformBreaker::default());
        update_platform(&db, UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
            extra: Some(cleared), models: None, available_models: None, endpoints: None,
            enabled: None, status: None, manual_budgets: None, join_group_ids: None, expires_at: None,
        }).await.unwrap();
        let b3 = get_platform(&db, p.id).await.unwrap().unwrap().breaker();
        assert_eq!((b3.failure_threshold, b3.open_secs, b3.half_open_max), (0, 0, 0), "clear breaker via extra");
    }



    /// 401/403 自动禁用：状态变 auto_disabled，strikes 递增，退避指数 1h/2h/4h。
    #[tokio::test]
    async fn auto_disable_exponential_backoff() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("ad")).await.unwrap();
        assert_eq!(p.status, PlatformStatus::Enabled);

        let base = 60 * 60 * 1000i64;
        // 第 1 次：strikes=1, 退避 1h
        let t0 = now();
        let until1 = set_platform_auto_disabled(&db, p.id).await.unwrap();
        let g1 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g1.status, PlatformStatus::AutoDisabled);
        assert!(!g1.enabled, "auto_disabled 平台 enabled 列同步为 false");
        assert_eq!(g1.auto_disable_strikes, 1);
        assert!(until1 >= t0 + base && until1 <= now() + base + 1000, "first backoff ~1h");

        // 第 2 次：strikes=2, 退避 2h
        set_platform_auto_disabled(&db, p.id).await.unwrap();
        let g2 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g2.auto_disable_strikes, 2);
        assert!(g2.auto_disabled_until - now() >= 2 * base - 2000, "second backoff ~2h");

        // 第 3 次：strikes=3, 退避 4h
        set_platform_auto_disabled(&db, p.id).await.unwrap();
        let g3 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g3.auto_disable_strikes, 3);
        assert!(g3.auto_disabled_until - now() >= 4 * base - 2000, "third backoff ~4h");

        // 2xx 恢复：清状态
        recover_platform_auto_disabled(&db, p.id).await.unwrap();
        let g4 = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g4.status, PlatformStatus::Enabled);
        assert!(g4.enabled);
        assert_eq!(g4.auto_disable_strikes, 0);
        assert_eq!(g4.auto_disabled_until, 0);
    }



    /// 用户手动 disabled 平台不受 401/403 自动禁用影响（区分手动 vs 自动）。
    #[tokio::test]
    async fn auto_disable_skips_user_disabled() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("ud")).await.unwrap();
        // 用户手动禁用
        let upd = update_platform(&db, UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
            extra: None, models: None, available_models: None, endpoints: None,
            enabled: None, status: Some(PlatformStatus::Disabled), manual_budgets: None,
            join_group_ids: None, expires_at: None,
        }).await.unwrap();
        assert_eq!(upd.status, PlatformStatus::Disabled);
        assert!(!upd.enabled);

        // 401/403 触发不应改成 auto_disabled
        let until = set_platform_auto_disabled(&db, p.id).await.unwrap();
        assert_eq!(until, 0, "user-disabled 平台不进入退避");
        let g = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g.status, PlatformStatus::Disabled, "保持用户手动禁用");
    }



    /// 改 api_key 自恢复：auto_disabled 平台改 api_key → 立即恢复 enabled 清退避。
    #[tokio::test]
    async fn api_key_change_recovers_auto_disabled() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("rk")).await.unwrap();
        set_platform_auto_disabled(&db, p.id).await.unwrap();
        assert_eq!(get_platform(&db, p.id).await.unwrap().unwrap().status, PlatformStatus::AutoDisabled);

        // 改 api_key（不显式传 status）
        let upd = update_platform(&db, UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None,
            api_key: Some("sk-new-key".to_string()),
            extra: None, models: None, available_models: None, endpoints: None,
            enabled: None, status: None, manual_budgets: None,
            join_group_ids: None, expires_at: None,
        }).await.unwrap();
        assert_eq!(upd.status, PlatformStatus::Enabled, "改 api_key 立即恢复");
        assert_eq!(upd.auto_disable_strikes, 0);
        assert_eq!(upd.auto_disabled_until, 0);
    }
