#![cfg(test)]
use super::*;

    #[test]
    fn apply_context_tier_selects_long_tier() {
        // OpenAI gpt-5.5: short in=5e-6/out=3e-5/cache=5e-7, long@272000 in=1e-5/out=4.5e-5/cache=1e-6
        let pd = serde_json::json!({
            "input_cost_per_token": 5e-6,
            "output_cost_per_token": 3e-5,
            "cache_read_input_token_cost": 5e-7,
            "context_tiers": [{
                "min_tokens": 272000,
                "input_cost_per_token": 1e-5,
                "output_cost_per_token": 4.5e-5,
                "cache_read_input_token_cost": 1e-6
            }]
        });
        let base = crate::gateway::models::ResolvedPrice {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 3e-5,
            cache_read_input_token_cost: 5e-7,
            source: "top_level".to_string(),
        };
        // 短档: input < 272000 → base 不变 (无 +tier 后缀)
        let short = apply_context_tier(base.clone(), &pd, 100_000);
        assert_eq!(short.input_cost_per_token, 5e-6);
        assert_eq!(short.output_cost_per_token, 3e-5);
        assert_eq!(short.source, "top_level");
        // 长档: input >= 272000 → tier 覆盖
        let long = apply_context_tier(base.clone(), &pd, 300_000);
        assert_eq!(long.input_cost_per_token, 1e-5);
        assert_eq!(long.output_cost_per_token, 4.5e-5);
        assert_eq!(long.cache_read_input_token_cost, 1e-6);
        assert_eq!(long.source, "top_level+tier");
        // 边界: 恰好等于阈值 → long
        let edge = apply_context_tier(base.clone(), &pd, 272_000);
        assert_eq!(edge.input_cost_per_token, 1e-5);
    }



    #[test]
    fn apply_context_tier_no_tier_passthrough() {
        // 无 context_tiers 字段 → base 不变 (向后兼容旧 price_data)
        let pd = serde_json::json!({"input_cost_per_token": 2.5e-6});
        let base = crate::gateway::models::ResolvedPrice {
            input_cost_per_token: 2.5e-6,
            output_cost_per_token: 1.5e-5,
            cache_read_input_token_cost: 2.5e-7,
            source: "top_level".to_string(),
        };
        let r = apply_context_tier(base.clone(), &pd, 999_999_999);
        assert_eq!(r.input_cost_per_token, 2.5e-6);
        assert_eq!(r.source, "top_level");
        // tiers 为空数组 → 同样不变
        let pd2 = serde_json::json!({"context_tiers": []});
        let r2 = apply_context_tier(base, &pd2, 999_999_999);
        assert_eq!(r2.source, "top_level");
    }



    #[test]
    fn apply_context_tier_partial_override() {
        // 长档仅覆盖部分字段 (如某些模型长档无 cache 价 → 继承 base cache)
        let pd = serde_json::json!({
            "input_cost_per_token": 3e-5,
            "output_cost_per_token": 1.8e-4,
            "cache_read_input_token_cost": 0.0,
            "context_tiers": [{
                "min_tokens": 272000,
                "input_cost_per_token": 6e-5,
                "output_cost_per_token": 2.7e-4
                // cache_read_input_token_cost 缺失 → 继承 base
            }]
        });
        let base = crate::gateway::models::ResolvedPrice {
            input_cost_per_token: 3e-5,
            output_cost_per_token: 1.8e-4,
            cache_read_input_token_cost: 0.0,
            source: "top_level".to_string(),
        };
        let r = apply_context_tier(base, &pd, 300_000);
        assert_eq!(r.input_cost_per_token, 6e-5);
        assert_eq!(r.output_cost_per_token, 2.7e-4);
        assert_eq!(r.cache_read_input_token_cost, 0.0); // 继承 base
    }

    use super::test_support::test_db;

    fn pd_basic() -> String {
        serde_json::json!({
            "input_cost_per_token": 3e-6,
            "output_cost_per_token": 1.5e-5,
            "cache_read_input_token_cost": 3e-7,
            "max_output_tokens": 8192
        })
        .to_string()
    }

    #[tokio::test]
    async fn upsert_get_list_count_search() {
        let db = test_db().await;
        upsert_model_price(&db, "claude-sonnet-4", "github", &pd_basic(), Some(200000), Some(8192), Some(200000))
            .await
            .unwrap();
        // upsert again → update path
        upsert_model_price(&db, "claude-sonnet-4", "github", &pd_basic(), Some(200000), Some(8192), Some(200000))
            .await
            .unwrap();
        upsert_model_price(&db, "gpt-4o", "github", &pd_basic(), None, None, None)
            .await
            .unwrap();

        assert_eq!(count_model_prices(&db).await.unwrap(), 2);

        let list = list_model_prices(&db, 10, 0).await.unwrap();
        assert_eq!(list.len(), 2);
        // input_price converted to $/M
        let claude = list.iter().find(|m| m.model_name == "claude-sonnet-4").unwrap();
        assert!((claude.input_price.unwrap() - 3.0).abs() < 1e-9);

        let got = get_model_price(&db, "gpt-4o").await.unwrap();
        assert!(got.is_some());
        assert!(get_model_price(&db, "missing").await.unwrap().is_none());

        let found = search_model_prices(&db, "claude", 10).await.unwrap();
        assert_eq!(found.len(), 1);
    }

    #[tokio::test]
    async fn get_model_price_prefers_manual_over_github() {
        let db = test_db().await;
        upsert_model_price(&db, "m", "github", &pd_basic(), None, None, None).await.unwrap();
        let manual_pd = serde_json::json!({"input_cost_per_token": 9e-6}).to_string();
        upsert_model_price(&db, "m", "manual", &manual_pd, None, None, None).await.unwrap();
        let got = get_model_price(&db, "m").await.unwrap().unwrap();
        assert_eq!(got.source, "manual");
    }

    #[tokio::test]
    async fn max_output_tokens_column_and_json_fallback() {
        let db = test_db().await;
        // column set
        upsert_model_price(&db, "a", "github", "{}", None, Some(4096), None).await.unwrap();
        assert_eq!(get_model_max_output_tokens(&db, "a").await.unwrap(), Some(4096));
        // column NULL, JSON fallback
        let pd = serde_json::json!({"max_output_tokens": 1234}).to_string();
        upsert_model_price(&db, "b", "github", &pd, None, None, None).await.unwrap();
        assert_eq!(get_model_max_output_tokens(&db, "b").await.unwrap(), Some(1234));
        // missing model
        assert_eq!(get_model_max_output_tokens(&db, "none").await.unwrap(), None);
    }

    #[tokio::test]
    async fn resolve_price_priority_chain() {
        let db = test_db().await;
        // platform_override path
        let pd_platform = serde_json::json!({
            "pricing": {"openai": {"input_cost_per_token": 1e-6, "output_cost_per_token": 2e-6, "cache_read_input_token_cost": 1e-7}}
        }).to_string();
        upsert_model_price(&db, "p1", "github", &pd_platform, None, None, None).await.unwrap();
        let r = resolve_price(&db, "p1", "openai", 99.0, 99.0, 0).await.unwrap();
        assert_eq!(r.source, "platform_override");
        assert_eq!(r.input_cost_per_token, 1e-6);

        // top_level path
        let pd_top = serde_json::json!({"input_cost_per_token": 5e-6, "output_cost_per_token": 6e-6}).to_string();
        upsert_model_price(&db, "p2", "github", &pd_top, None, None, None).await.unwrap();
        let r2 = resolve_price(&db, "p2", "anthropic", 99.0, 99.0, 0).await.unwrap();
        assert_eq!(r2.source, "top_level");

        // default_platform path
        let pd_dp = serde_json::json!({
            "default_platform": "glm",
            "pricing": {"glm": {"input_cost_per_token": 7e-6, "output_cost_per_token": 8e-6}}
        }).to_string();
        upsert_model_price(&db, "p3", "github", &pd_dp, None, None, None).await.unwrap();
        let r3 = resolve_price(&db, "p3", "no-match", 99.0, 99.0, 0).await.unwrap();
        assert_eq!(r3.source, "default_platform");

        // fallback path (no record)
        let r4 = resolve_price(&db, "unknown", "openai", 3.0, 6.0, 0).await.unwrap();
        assert_eq!(r4.source, "fallback");
        assert!((r4.input_cost_per_token - 3.0 / 1_000_000.0).abs() < 1e-12);
    }

    #[tokio::test]
    async fn filtered_list_and_count() {
        let db = test_db().await;
        upsert_model_price(&db, "alpha", "github", &pd_basic(), None, None, None).await.unwrap();
        upsert_model_price(&db, "beta", "manual", &pd_basic(), None, None, None).await.unwrap();
        upsert_model_price(&db, "alphabeta", "github", &pd_basic(), None, None, None).await.unwrap();

        // no filter
        assert_eq!(filtered_count_model_prices(&db, None, None).await.unwrap(), 3);
        // query filter
        let q = filtered_list_model_prices(&db, Some("alpha"), None, 10, 0).await.unwrap();
        assert_eq!(q.len(), 2);
        assert_eq!(filtered_count_model_prices(&db, Some("alpha"), None).await.unwrap(), 2);
        // source filter
        let s = filtered_list_model_prices(&db, None, Some("manual"), 10, 0).await.unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(filtered_count_model_prices(&db, None, Some("manual")).await.unwrap(), 1);
        // both + empty strings ignored ("alpha"/"alphabeta" both github)
        let b = filtered_list_model_prices(&db, Some("alpha"), Some("github"), 10, 0).await.unwrap();
        assert_eq!(b.len(), 2);
        let empty = filtered_list_model_prices(&db, Some(""), Some(""), 10, 0).await.unwrap();
        assert_eq!(empty.len(), 3);
        assert_eq!(filtered_count_model_prices(&db, Some(""), Some("")).await.unwrap(), 3);
    }
