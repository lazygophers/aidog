#![cfg(test)]
use super::*;
use super::test_support::*;



    // ── 平台 breaker 配置经 extra 往返 + 手动组成员全量同步（auto 组不动）──

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
            enabled: None, status: None, manual_budgets: None, join_group_ids: None,
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
            join_group_ids: None,
        }).await.unwrap();
        assert_eq!(upd.status, PlatformStatus::Disabled);
        assert!(!upd.enabled);

        // 401/403 触发不应改成 auto_disabled
        let until = set_platform_auto_disabled(&db, p.id).await.unwrap();
        assert_eq!(until, 0, "user-disabled 平台不进入退避");
        let g = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g.status, PlatformStatus::Disabled, "保持用户手动禁用");
    }



    /// 404/405 死端点：连续累计达阈值才禁用；未达阈值保持 enabled。
    #[tokio::test]
    async fn dead_endpoint_strikes_accumulate_then_disable() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("de")).await.unwrap();
        let th = DEAD_ENDPOINT_STRIKE_THRESHOLD; // 3
        assert!(th >= 2, "阈值须 ≥2 才能体现累计语义");

        // 前 th-1 次：仅累计计数，保持 enabled、不退避
        for i in 1..th {
            let (strikes, until) = record_dead_endpoint_strike(&db, p.id, th).await.unwrap();
            assert_eq!(strikes, i, "第 {i} 次 strikes 递增");
            assert_eq!(until, 0, "未达阈值不禁用");
            let g = get_platform(&db, p.id).await.unwrap().unwrap();
            assert_eq!(g.status, PlatformStatus::Enabled, "未达阈值仍 enabled，继续参与调度");
            assert!(g.enabled);
        }

        // 第 th 次：达阈值 → auto_disabled + 退避
        let (strikes, until) = record_dead_endpoint_strike(&db, p.id, th).await.unwrap();
        assert_eq!(strikes, th);
        assert!(until > now(), "达阈值后进入退避");
        let g = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g.status, PlatformStatus::AutoDisabled, "达阈值后临时禁用");
        assert!(!g.enabled);
    }



    /// 偶发 404/405：未达阈值 + 一次 2xx 成功 → 计数清零，平台不被误禁。
    #[tokio::test]
    async fn dead_endpoint_transient_reset_on_success() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("dt")).await.unwrap();
        let th = DEAD_ENDPOINT_STRIKE_THRESHOLD;

        // 累计 th-1 次（差一次就禁用）
        for _ in 1..th {
            record_dead_endpoint_strike(&db, p.id, th).await.unwrap();
        }
        assert_eq!(get_platform(&db, p.id).await.unwrap().unwrap().auto_disable_strikes, th - 1);
        assert_eq!(get_platform(&db, p.id).await.unwrap().unwrap().status, PlatformStatus::Enabled);

        // 一次成功 → 清零计数
        reset_dead_endpoint_strikes(&db, p.id).await.unwrap();
        let g = get_platform(&db, p.id).await.unwrap().unwrap();
        assert_eq!(g.auto_disable_strikes, 0, "成功后计数清零");
        assert_eq!(g.status, PlatformStatus::Enabled);

        // 之后再来一次 404 → 重新从 1 数起，不会因历史累计被立即禁
        let (strikes, until) = record_dead_endpoint_strike(&db, p.id, th).await.unwrap();
        assert_eq!(strikes, 1, "清零后重新从 1 累计");
        assert_eq!(until, 0);
    }



    /// 死端点累计跳过用户手动禁用平台（区分手动 vs 自动）。
    #[tokio::test]
    async fn dead_endpoint_skips_user_disabled() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("du")).await.unwrap();
        update_platform(&db, UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
            extra: None, models: None, available_models: None, endpoints: None,
            enabled: None, status: Some(PlatformStatus::Disabled), manual_budgets: None,
            join_group_ids: None,
        }).await.unwrap();

        let (strikes, until) = record_dead_endpoint_strike(&db, p.id, DEAD_ENDPOINT_STRIKE_THRESHOLD).await.unwrap();
        assert_eq!((strikes, until), (0, 0), "user-disabled 平台死端点信号不动它");
        assert_eq!(get_platform(&db, p.id).await.unwrap().unwrap().status, PlatformStatus::Disabled);
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
            join_group_ids: None,
        }).await.unwrap();
        assert_eq!(upd.status, PlatformStatus::Enabled, "改 api_key 立即恢复");
        assert_eq!(upd.auto_disable_strikes, 0);
        assert_eq!(upd.auto_disabled_until, 0);
    }
