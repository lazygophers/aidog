#![cfg(test)]
use super::*;
use super::test_support::*;

    // ── R2 单数表名 + "group" 转义：init_tables 成功间接验证 DDL ──
    #[tokio::test]
    async fn r2_singular_table_names_and_group_escaped() {
        // init_tables() 已在 test_db 中执行；进一步断言单数表名存在、复数不存在
        let db = test_db().await;
        let names: Vec<String> = db.call_traced(None, std::panic::Location::caller(), |conn| {
            Ok(conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?
                .query_map([], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect())
        }).await.unwrap();
        assert!(names.contains(&"platform".to_string()));
        assert!(names.contains(&"group".to_string()));
        assert!(names.contains(&"group_platform".to_string()));
        assert!(names.contains(&"setting".to_string()));
        assert!(names.contains(&"proxy_log".to_string()));
        // 复数旧表名禁止存在
        assert!(!names.contains(&"platforms".to_string()));
        assert!(!names.contains(&"groups".to_string()));
        assert!(!names.contains(&"model_mappings".to_string()));
    }



    // ── R7 / D1 主键自增 uint64 ──
    #[tokio::test]
    async fn r7_platform_pk_autoincrement_u64() {
        let db = test_db().await;
        let p1 = create_platform(&db, sample_platform("p1")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("p2")).await.unwrap();
        assert!(p1.id >= 1, "first id should be >= 1, got {}", p1.id);
        assert_eq!(p2.id, p1.id + 1, "id should auto-increment");
        // 类型为 u64（编译期保证），运行期断言 >0
        let _: u64 = p2.id;
        assert!(p2.id > 0);
    }



    // ── R1 / R9 毫秒级时间戳 ──
    #[tokio::test]
    async fn r1_timestamps_are_millis() {
        let db = test_db().await;
        let before = chrono::Utc::now().timestamp_millis();
        let p = create_platform(&db, sample_platform("ts")).await.unwrap();
        let after = chrono::Utc::now().timestamp_millis();
        // 毫秒级：> 1e12（2001 年之后），且落在 before..=after 区间
        assert!(p.created_at > 1_000_000_000_000, "created_at not ms-level: {}", p.created_at);
        assert!(p.updated_at > 1_000_000_000_000, "updated_at not ms-level: {}", p.updated_at);
        assert!(p.created_at >= before && p.created_at <= after);
        assert_eq!(p.created_at, p.updated_at);
    }



    // ── R10 禁 NULL：未提供 extra 时为空串而非 NULL ──
    #[tokio::test]
    async fn r10_no_null_defaults() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("nn")).await.unwrap();
        assert_eq!(p.extra, "");

        let g = create_group(&db, sample_group("g", vec![])).await.unwrap();
        assert_eq!(g.auto_from_platform, "");
        assert_eq!(g.model_mappings.len(), 0);

        // 直接断言列值非 NULL
        let (null_count, g_null): (i64, i64) = db.call_traced(None, std::panic::Location::caller(), |conn| {
            let null_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM platform WHERE extra IS NULL OR base_url IS NULL OR api_key IS NULL",
                [],
                |r| r.get(0),
            )?;
            let g_null: i64 = conn.query_row(
                "SELECT COUNT(*) FROM \"group\" WHERE auto_from_platform IS NULL OR model_mappings IS NULL OR source_protocol IS NULL",
                [],
                |r| r.get(0),
            )?;
            Ok((null_count, g_null))
        }).await.unwrap();
        assert_eq!(null_count, 0, "no platform column should be NULL");
        assert_eq!(g_null, 0, "no group column should be NULL");
    }



    // ── R8 proxy_log 主键 TEXT hex32（无连字符），软删 + retention ──
    #[tokio::test]
    async fn r8_proxy_log_uuid_no_hyphen_and_retention() {
        let db = test_db().await;
        // hex32 无连字符 id（模拟生产生成方式 uuid simple）
        let new_id = uuid::Uuid::new_v4().simple().to_string();
        assert_eq!(new_id.len(), 32, "simple uuid should be 32 hex chars");
        assert!(!new_id.contains('-'), "uuid must not contain hyphen");

        let now_ms = chrono::Utc::now().timestamp_millis();
        // 一条最近日志
        upsert_proxy_log(&db, sample_log(&new_id, "g", now_ms)).await.unwrap();
        // 一条很旧的日志（100 天前）
        let old_id = uuid::Uuid::new_v4().simple().to_string();
        let old_ms = now_ms - 100 * 86_400_000;
        upsert_proxy_log(&db, sample_log(&old_id, "g", old_ms)).await.unwrap();

        assert_eq!(count_proxy_logs(&db).await.unwrap(), 2);

        // retention 30 天：旧日志被软删
        cleanup_proxy_logs(&db, 30).await.unwrap();
        assert_eq!(count_proxy_logs(&db).await.unwrap(), 1);
        assert!(get_proxy_log(&db, &old_id).await.unwrap().is_none());
        assert!(get_proxy_log(&db, &new_id).await.unwrap().is_some());

        // proxy_log 主键存储原样 TEXT
        let fetched = get_proxy_log(&db, &new_id).await.unwrap().unwrap();
        assert_eq!(fetched.id, new_id);
        assert!(fetched.created_at > 1_000_000_000_000);
    }



    /// 全新 db 首启即 seed 全部内置规则，is_builtin=1 且默认 enabled。
    #[tokio::test]
    async fn c4_fresh_db_seeds_builtin_rules() {
        let db = test_db().await;
        let rules = list_middleware_rules(&db).await.unwrap();
        let builtin: Vec<_> = rules.iter().filter(|r| r.is_builtin).collect();
        assert_eq!(
            builtin.len(),
            builtin_rule_specs().len(),
            "首启应 seed 全部内置规则"
        );
        for r in &builtin {
            assert!(r.enabled, "内置规则 {} 默认应 enabled", r.name);
            assert_eq!(r.scope, RuleScope::Global);
        }
        // 三条脱敏 + 默认 error_rules
        assert!(builtin.iter().any(|r| r.name == "内置·密钥脱敏"));
        assert!(builtin.iter().any(|r| r.name == "内置·邮箱脱敏"));
        assert!(builtin.iter().any(|r| r.name == "内置·手机号脱敏"));
        assert!(builtin
            .iter()
            .any(|r| r.rule_type == RuleType::ErrorRule));
    }



    /// 密钥/邮箱脱敏走 content_filter 空 pattern（复用 C2 内置检测器），手机走显式 regex。
    #[tokio::test]
    async fn c4_secret_email_reuse_c2_detector_phone_explicit() {
        let db = test_db().await;
        let rules = list_middleware_rules(&db).await.unwrap();
        let secret = rules.iter().find(|r| r.name == "内置·密钥脱敏").unwrap();
        let email = rules.iter().find(|r| r.name == "内置·邮箱脱敏").unwrap();
        let phone = rules.iter().find(|r| r.name == "内置·手机号脱敏").unwrap();
        // 密钥/邮箱 pattern 留空 → C2 BUILTIN_SECRET/EMAIL 检测器接管，避免重复定义正则
        assert!(secret.pattern.is_empty(), "密钥规则 pattern 应留空复用 C2 检测器");
        assert!(email.pattern.is_empty(), "邮箱规则 pattern 应留空复用 C2 检测器");
        assert_eq!(secret.rule_type, RuleType::ContentFilter);
        assert_eq!(secret.action, RuleAction::Mask);
        // 手机 C2 无内置检测器 → 显式 regex
        assert!(!phone.pattern.is_empty(), "手机规则用显式 regex");
        assert_eq!(phone.match_type, MatchType::Regex);
    }



    /// 内置手机号正则命中中国大陆 11 位 + E.164；不误伤普通数字。
    #[test]
    fn c4_builtin_phone_pattern_matches_samples() {
        let re = regex::Regex::new(BUILTIN_PHONE_PATTERN).unwrap();
        assert!(re.is_match("联系我 13812345678 谢谢"), "中国大陆手机号");
        assert!(re.is_match("call +14155552671 now"), "E.164 国际号");
        assert!(!re.is_match("订单号 12345"), "短数字不应命中");
    }



    /// 内置默认 error_rules 正则命中各 category 的样例上游错误消息。
    #[test]
    fn c4_builtin_error_rules_match_samples() {
        // (category, 样例错误消息)
        let samples: &[(&str, &str)] = &[
            ("prompt_limit", "This model's maximum context length is 128000 tokens"),
            ("content_filter", "The response was flagged by our content filter"),
            ("pdf_limit", "PDF has too many pages, maximum is 100"),
            ("thinking_error", "thinking is not supported for this model"),
            ("parameter_error", "Unsupported parameter: 'temperature' is not allowed"),
            ("invalid_request", "invalid_request_error: missing field"),
            ("cache_limit", "prompt cache: too many cache_control blocks"),
        ];
        for (category, msg) in samples {
            let spec = builtin_rule_specs()
                .iter()
                .find(|s| s.rule_type == "error_rule" && s.config.contains(&format!("\"category\":\"{category}\"")))
                .unwrap_or_else(|| panic!("缺 category={category} 的 error_rule"));
            let re = regex::Regex::new(spec.pattern).unwrap();
            assert!(
                re.is_match(msg),
                "category={category} 正则 {} 应命中样例: {msg}",
                spec.pattern
            );
        }
    }



    /// seed 幂等：重复调用（模拟重启）不重复插入。
    #[tokio::test]
    async fn c4_seed_is_idempotent_on_restart() {
        let db = test_db().await;
        let before = list_middleware_rules(&db).await.unwrap().len();
        // 再次跑一遍 init_tables（含 seed），模拟重启
        db.init_tables().await.unwrap();
        let after = list_middleware_rules(&db).await.unwrap().len();
        assert_eq!(before, after, "重启不应重复 seed");
    }



    /// 用户禁用内置规则后重启不被重新启用（尊重用户禁用状态，内置可禁不可硬删）。
    #[tokio::test]
    async fn c4_seed_respects_user_disabled_state() {
        let db = test_db().await;
        let rules = list_middleware_rules(&db).await.unwrap();
        let secret = rules.iter().find(|r| r.name == "内置·密钥脱敏").unwrap().clone();
        // 用户禁用该内置规则
        update_middleware_rule(
            &db,
            UpdateMiddlewareRule {
                id: secret.id,
                name: secret.name.clone(),
                description: secret.description.clone(),
                rule_type: secret.rule_type,
                scope: secret.scope,
                scope_ref: secret.scope_ref.clone(),
                match_type: secret.match_type,
                pattern: secret.pattern.clone(),
                action: secret.action,
                config: secret.config.clone(),
                priority: secret.priority,
                enabled: false,
                is_builtin: true,
            },
        )
        .await
        .unwrap();
        // 重启
        db.init_tables().await.unwrap();
        let after = list_middleware_rules(&db).await.unwrap();
        let secret_after = after.iter().find(|r| r.name == "内置·密钥脱敏").unwrap();
        assert!(!secret_after.enabled, "用户禁用的内置规则重启后不应被重新启用");
        // 仍只有一条（未重复插入）
        let count = after.iter().filter(|r| r.name == "内置·密钥脱敏").count();
        assert_eq!(count, 1, "禁用的内置规则不应被重复 seed");
    }
