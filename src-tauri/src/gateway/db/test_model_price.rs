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
