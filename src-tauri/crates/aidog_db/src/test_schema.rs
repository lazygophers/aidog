#![cfg(test)]
use super::test_support::*;
use super::*;
use crate::schema::BUILTIN_PHONE_PATTERN;
use crate::schema_early::run_migrations_platform_early;
use crate::schema_late::run_migrations_platform_late;
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









    /// 内置手机号正则命中中国大陆 11 位 + E.164；不误伤普通数字。
    #[test]
    fn c4_builtin_phone_pattern_matches_samples() {
        let re = regex::Regex::new(BUILTIN_PHONE_PATTERN).unwrap();
        assert!(re.is_match("联系我 13812345678 谢谢"), "中国大陆手机号");
        assert!(re.is_match("call +14155552671 now"), "E.164 国际号");
        assert!(!re.is_match("订单号 12345"), "短数字不应命中");
    }





    /// Migration 20260727-01 (原 012): kimi 平台 codex_tui → claude_code 修正。
    /// 构造一个 legacy 连接（已有 platform 表 + kimi 平台 codex_tui endpoint），
    /// 再运行 run_migrations_early，验证 endpoint client_type 被改为 claude_code。
    #[test]
    fn migration_012_kimi_codex_tui_to_claude_code() {
        // 裸 rusqlite 连接（不走 Db async wrapper），直接测迁移逻辑
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // 建 platform 表（精简版，含迁移 012 所需列）
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS platform (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                name             TEXT NOT NULL DEFAULT '',
                platform_type    TEXT NOT NULL DEFAULT '',
                base_url         TEXT NOT NULL DEFAULT '',
                api_key          TEXT NOT NULL DEFAULT '',
                extra            TEXT NOT NULL DEFAULT '',
                models           TEXT NOT NULL DEFAULT '{}',
                available_models TEXT NOT NULL DEFAULT '[]',
                endpoints        TEXT NOT NULL DEFAULT '[]',
                enabled          INTEGER NOT NULL DEFAULT 1,
                est_balance_remaining REAL NOT NULL DEFAULT 0,
                est_coding_plan       TEXT NOT NULL DEFAULT '',
                last_real_query_at    INTEGER NOT NULL DEFAULT 0,
                estimate_count        INTEGER NOT NULL DEFAULT 0,
                show_in_tray          INTEGER NOT NULL DEFAULT 0,
                tray_display          TEXT NOT NULL DEFAULT 'balance',
                created_at       INTEGER NOT NULL DEFAULT 0,
                updated_at       INTEGER NOT NULL DEFAULT 0,
                deleted_at       INTEGER NOT NULL DEFAULT 0
            );",
        ).unwrap();
        // 插入一个 kimi 平台，endpoints 含需修正的 codex_tui coding endpoint
        let endpoints_json = r#"[
            {"protocol":"openai","base_url":"https://api.moonshot.cn/v1","client_type":"codex_tui","coding_plan":true},
            {"protocol":"anthropic","base_url":"https://api.moonshot.cn/anthropic/v1","client_type":"claude_code","coding_plan":false}
        ]"#;
        conn.execute(
            "INSERT INTO platform (name, platform_type, endpoints, created_at, updated_at, deleted_at)
             VALUES ('kimi-test', 'kimi', ?1, 0, 0, 0)",
            rusqlite::params![endpoints_json],
        ).unwrap();
        // 也插入一个非 kimi 平台（应不受影响）
        conn.execute(
            "INSERT INTO platform (name, platform_type, endpoints, created_at, updated_at, deleted_at)
             VALUES ('other', 'anthropic', '[{\"protocol\":\"openai\",\"base_url\":\"\",\"client_type\":\"codex_tui\",\"coding_plan\":true}]', 0, 0, 0)",
            [],
        ).unwrap();
        // 运行 platform migrations（原 012 已搬至 run_migrations_platform_late，重编 20260727-01；platform_early 建
        // "group" / group_platform 兜底 012 不踩「group 表缺」坑，CREATE IF NOT EXISTS 幂等）。
        run_migrations_platform_early(&conn).expect("run_migrations_platform_early should succeed");
        run_migrations_platform_late(&conn).expect("run_migrations_platform_late should succeed");
        // 验证 kimi 平台的 endpoint 被修正
        let kimi_eps: String = conn.query_row(
            "SELECT endpoints FROM platform WHERE name = 'kimi-test'",
            [],
            |r| r.get(0),
        ).unwrap();
        let eps: Vec<serde_json::Value> = serde_json::from_str(&kimi_eps).unwrap();
        let openai_ep = eps.iter().find(|e| e.get("protocol").and_then(|v| v.as_str()) == Some("openai")).unwrap();
        assert_eq!(
            openai_ep.get("client_type").and_then(|v| v.as_str()),
            Some("claude_code"),
            "migration 20260727-01 (原 012) should fix codex_tui→claude_code for kimi openai coding endpoint"
        );
        // anthropic endpoint 不受影响
        let anthropic_ep = eps.iter().find(|e| e.get("protocol").and_then(|v| v.as_str()) == Some("anthropic")).unwrap();
        assert_eq!(
            anthropic_ep.get("client_type").and_then(|v| v.as_str()),
            Some("claude_code"),
            "anthropic endpoint client_type should remain unchanged"
        );
        // 非 kimi 平台的 codex_tui 不被修正
        let other_eps: String = conn.query_row(
            "SELECT endpoints FROM platform WHERE name = 'other'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert!(other_eps.contains("codex_tui"), "non-kimi platform endpoint should not be changed");
    }

    /// Migration 20260729-01: platform.extra.peak_hours 中命中 W2 三条件指纹
    /// （start_at==1790784000 && multiplier==2.0 && start_hour==0 && end_hour==24）的窗口应被删除。
    #[test]
    fn migration_20260729_01_strips_w2_peak_window() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS platform (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                name             TEXT NOT NULL DEFAULT '',
                platform_type    TEXT NOT NULL DEFAULT '',
                base_url         TEXT NOT NULL DEFAULT '',
                api_key          TEXT NOT NULL DEFAULT '',
                extra            TEXT NOT NULL DEFAULT '',
                models           TEXT NOT NULL DEFAULT '{}',
                available_models TEXT NOT NULL DEFAULT '[]',
                endpoints        TEXT NOT NULL DEFAULT '[]',
                enabled          INTEGER NOT NULL DEFAULT 1,
                est_balance_remaining REAL NOT NULL DEFAULT 0,
                est_coding_plan       TEXT NOT NULL DEFAULT '',
                last_real_query_at    INTEGER NOT NULL DEFAULT 0,
                estimate_count        INTEGER NOT NULL DEFAULT 0,
                show_in_tray          INTEGER NOT NULL DEFAULT 0,
                tray_display          TEXT NOT NULL DEFAULT 'balance',
                created_at       INTEGER NOT NULL DEFAULT 0,
                updated_at       INTEGER NOT NULL DEFAULT 0,
                deleted_at       INTEGER NOT NULL DEFAULT 0
            );",
        ).unwrap();
        let extra = r#"{"_ui_collapsed":true,"peak_hours":[{"start_at":1790784000,"multiplier":2.0,"start_hour":0,"end_hour":24}]}"#;
        conn.execute(
            "INSERT INTO platform (name, platform_type, extra, created_at, updated_at, deleted_at)
             VALUES ('glm-coding-test', 'glm_coding', ?1, 0, 0, 0)",
            rusqlite::params![extra],
        ).unwrap();
        run_migrations_platform_early(&conn).expect("run_migrations_platform_early should succeed");
        run_migrations_platform_late(&conn).expect("run_migrations_platform_late should succeed");
        let new_extra: String = conn.query_row(
            "SELECT extra FROM platform WHERE name = 'glm-coding-test'",
            [],
            |r| r.get(0),
        ).unwrap();
        let v: serde_json::Value = serde_json::from_str(&new_extra).unwrap();
        assert!(v.get("peak_hours").is_none(), "W2 窗口是数组唯一元素, 删空后应整个删掉 peak_hours 键, got {new_extra}");
        // 20260829-02 把 peak_hours 改名为 peak：W2 窗口删空后改名也不应留下键
        assert!(v.get("peak").is_none(), "改名后 peak 键也不应存在, got {new_extra}");
        assert_eq!(v.get("_ui_collapsed").and_then(|b| b.as_bool()), Some(true), "其余字段应原样保留");
    }

    /// Migration 20260729-01: 只中部分条件（用户自建窗口, 如不同 multiplier）应被保留, 不误删。
    #[test]
    fn migration_20260729_01_keeps_user_defined_peak_window() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS platform (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                name             TEXT NOT NULL DEFAULT '',
                platform_type    TEXT NOT NULL DEFAULT '',
                base_url         TEXT NOT NULL DEFAULT '',
                api_key          TEXT NOT NULL DEFAULT '',
                extra            TEXT NOT NULL DEFAULT '',
                models           TEXT NOT NULL DEFAULT '{}',
                available_models TEXT NOT NULL DEFAULT '[]',
                endpoints        TEXT NOT NULL DEFAULT '[]',
                enabled          INTEGER NOT NULL DEFAULT 1,
                est_balance_remaining REAL NOT NULL DEFAULT 0,
                est_coding_plan       TEXT NOT NULL DEFAULT '',
                last_real_query_at    INTEGER NOT NULL DEFAULT 0,
                estimate_count        INTEGER NOT NULL DEFAULT 0,
                show_in_tray          INTEGER NOT NULL DEFAULT 0,
                tray_display          TEXT NOT NULL DEFAULT 'balance',
                created_at       INTEGER NOT NULL DEFAULT 0,
                updated_at       INTEGER NOT NULL DEFAULT 0,
                deleted_at       INTEGER NOT NULL DEFAULT 0
            );",
        ).unwrap();
        // start_at 相同魔数但 multiplier 不同（用户手配撞上起点巧合）—— 三条件未全中, 应保留
        let extra = r#"{"peak_hours":[{"start_at":1790784000,"multiplier":1.5,"start_hour":0,"end_hour":24}]}"#;
        conn.execute(
            "INSERT INTO platform (name, platform_type, extra, created_at, updated_at, deleted_at)
             VALUES ('user-custom', 'glm_coding', ?1, 0, 0, 0)",
            rusqlite::params![extra],
        ).unwrap();
        run_migrations_platform_early(&conn).expect("run_migrations_platform_early should succeed");
        run_migrations_platform_late(&conn).expect("run_migrations_platform_late should succeed");
        let new_extra: String = conn.query_row(
            "SELECT extra FROM platform WHERE name = 'user-custom'",
            [],
            |r| r.get(0),
        ).unwrap();
        let v: serde_json::Value = serde_json::from_str(&new_extra).unwrap();
        // 20260829-02 已把 extra 键 peak_hours 改名为 peak，窗口值原样保留
        let arr = v.get("peak").and_then(|a| a.as_array()).expect("peak 应保留");
        assert_eq!(arr.len(), 1, "只部分中条件的用户自建窗口不应被删");
        assert_eq!(arr[0].get("multiplier").and_then(|m| m.as_f64()), Some(1.5));
    }

    /// seed 幂等：重复调用（模拟重启）不重复插入。
    #[tokio::test]
    async fn c4_seed_is_idempotent_on_restart() {
        let db = test_db().await;
        let before = list_middleware_rules(&db).await.unwrap().len();
        // 再次跑一遍 init_tables（含 seed），模拟重启
        crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(()))).await.unwrap();
        let after = list_middleware_rules(&db).await.unwrap().len();
        assert_eq!(before, after, "重启不应重复 seed");
    }



