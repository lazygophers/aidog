#![cfg(test)]
use super::*;
use super::test_support::*;
use rusqlite::params;



    /// endpoints 反序列化容错：DB 中含未知 client_type（如旧数据 "anthropic"）的
    /// endpoint 数组应仍能完整解析，而非因单个未知枚举值整体失败 → 空 Vec → 前端丢失。
    #[tokio::test]
    async fn endpoints_with_unknown_client_type_still_parse() {
        let json = r#"[{"protocol":"openai","base_url":"https://x/v1","client_type":"codex_tui","coding_plan":false},{"protocol":"anthropic","base_url":"https://x/anthropic","client_type":"anthropic","coding_plan":false}]"#;
        let parsed = parse_endpoints(json);
        assert_eq!(parsed.len(), 2, "未知 client_type 不应导致整个数组解析失败");
        assert_eq!(parsed[1].client_type, ClientType::Default, "未知值回退为 Default");
        assert_eq!(parsed[1].protocol, Protocol::Anthropic);

        // 端到端：写入 DB 后 list_platforms 应带回 endpoints
        let db = test_db().await;
        let mut input = sample_platform("p1");
        input.endpoints = Some(vec![PlatformEndpoint {
            protocol: Protocol::OpenAI,
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            client_type: ClientType::CodexTui,
            coding_plan: true,
        }]);
        create_platform(&db, input).await.unwrap();
        let listed = list_platforms(&db).await.unwrap();
        assert_eq!(listed[0].endpoints.len(), 1, "list_platforms 应返回 endpoints");
    }



    // ── R9 软删除：delete 后 deleted_at>0；list 不含；get 返回 None ──
    #[tokio::test]
    async fn r9_soft_delete_platform() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("del")).await.unwrap();
        assert_eq!(list_platforms(&db).await.unwrap().len(), 1);

        delete_platform(&db, p.id).await.unwrap();

        // list 不返回已删行
        assert_eq!(list_platforms(&db).await.unwrap().len(), 0);
        // get 返回 None
        assert!(get_platform(&db, p.id).await.unwrap().is_none());

        // 行仍存在且 deleted_at > 0（物理保留）
        let pid = p.id as i64;
        let deleted_at: i64 = db.call_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn.query_row("SELECT deleted_at FROM platform WHERE id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert!(deleted_at > 0, "deleted_at should be set, got {deleted_at}");
    }

    // ── 删平台只清关联，不连带销毁手动组与有其他成员的 auto 组 ──
    #[tokio::test]
    async fn delete_platform_preserves_groups_with_other_members() {
        let db = test_db().await;
        let p_del = create_platform(&db, sample_platform("del-src")).await.unwrap();
        let p_keep = create_platform(&db, sample_platform("keep")).await.unwrap();

        // ① 手动组：同时含待删平台与另一存活平台。
        let m = create_group(&db, sample_group("m-shared", vec![])).await.unwrap();
        set_group_platforms(&db, m.id, &[
            GroupPlatformInput { platform_id: p_del.id, priority: Some(0), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_keep.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        // ② p_del 的 auto 组，用户额外把 p_keep 也拖了进来。
        let auto_g = create_group(&db, CreateGroup {
            name: "del-src-auto".into(), group_key: Some("del-src-auto".into()),
            routing_mode: RoutingMode::Failover, auto_from_platform: p_del.id.to_string(),
            request_timeout_secs: 0, connect_timeout_secs: 0,
            source_protocol: None, max_retries: 2, model_mappings: vec![],
        }).await.unwrap();
        set_group_platforms(&db, auto_g.id, &[
            GroupPlatformInput { platform_id: p_del.id, priority: Some(0), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_keep.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        // ③ p_del 的孤儿 auto 组（仅含自己），删平台后应被清掉。
        let auto_orphan = create_group(&db, CreateGroup {
            name: "del-src-orphan".into(), group_key: Some("del-src-orphan".into()),
            routing_mode: RoutingMode::Failover, auto_from_platform: p_del.id.to_string(),
            request_timeout_secs: 0, connect_timeout_secs: 0,
            source_protocol: None, max_retries: 2, model_mappings: vec![],
        }).await.unwrap();
        set_group_platforms(&db, auto_orphan.id, &[
            GroupPlatformInput { platform_id: p_del.id, priority: Some(0), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        delete_platform(&db, p_del.id).await.unwrap();

        // 手动组仍在，仅剩存活平台，悬空关联已清除。
        assert!(get_group(&db, m.id).await.unwrap().is_some(), "手动组不应被删");
        let m_plats = get_group_platforms(&db, m.id).await.unwrap();
        assert_eq!(m_plats.len(), 1, "手动组仅余存活平台");
        assert_eq!(m_plats[0].platform.id, p_keep.id);

        // 含其他成员的 auto 组保留，只剩 p_keep。
        assert!(get_group(&db, auto_g.id).await.unwrap().is_some(), "有其他成员的 auto 组不应被删");
        let a_plats = get_group_platforms(&db, auto_g.id).await.unwrap();
        assert_eq!(a_plats.len(), 1);
        assert_eq!(a_plats[0].platform.id, p_keep.id);

        // 孤儿 auto 组（删后无成员）被删。
        assert!(get_group(&db, auto_orphan.id).await.unwrap().is_none(), "孤儿 auto 组应被删");

        // 全表无指向已删平台的关联残留。
        let pid = p_del.id as i64;
        let stale: i64 = db.call_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM group_platform WHERE platform_id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(stale, 0, "不应残留指向已删平台的 group_platform 行");
    }



    // ── 一键清理失效平台：全局全删 / 分组独占删 / 分组共享仅移除关联 ──
    #[tokio::test]
    async fn purge_auto_disabled_global_deletes_all() {
        let db = test_db().await;
        // 两个 auto_disabled + 一个 enabled（应保留）。
        let p_dead1 = create_platform(&db, sample_platform("dead1")).await.unwrap();
        let p_dead2 = create_platform(&db, sample_platform("dead2")).await.unwrap();
        let p_alive = create_platform(&db, sample_platform("alive")).await.unwrap();
        set_platform_auto_disabled(&db, p_dead1.id).await.unwrap();
        set_platform_auto_disabled(&db, p_dead2.id).await.unwrap();

        let r = purge_auto_disabled_platforms(&db, None).await.unwrap();
        assert_eq!(r.deleted_ids.len(), 2, "全局应删两个 auto_disabled");
        assert!(r.unassigned_ids.is_empty(), "全局模式不应有 unassigned");
        assert!(r.deleted_ids.contains(&(p_dead1.id as u64)));
        assert!(r.deleted_ids.contains(&(p_dead2.id as u64)));

        // 已删平台行仍存在但 deleted_at>0；enabled 平台未动。
        assert!(get_platform(&db, p_dead1.id).await.unwrap().is_none());
        assert!(get_platform(&db, p_dead2.id).await.unwrap().is_none());
        assert!(get_platform(&db, p_alive.id).await.unwrap().is_some(), "enabled 平台不应被删");
    }



    #[tokio::test]
    async fn purge_auto_disabled_group_exclusive_deletes_shared_unassigns() {
        let db = test_db().await;
        // 平台 A：仅属 g1，auto_disabled → 分组级清理应永久删。
        let p_a = create_platform(&db, sample_platform("a-exclusive")).await.unwrap();
        // 平台 B：属 g1 + g2，auto_disabled → 分组级清理 g1 应仅移除 g1 关联，平台行保留、g2 关联保留。
        let p_b = create_platform(&db, sample_platform("b-shared")).await.unwrap();
        set_platform_auto_disabled(&db, p_a.id).await.unwrap();
        set_platform_auto_disabled(&db, p_b.id).await.unwrap();

        let g1 = create_group(&db, sample_group("g1", vec![])).await.unwrap();
        let g2 = create_group(&db, sample_group("g2", vec![])).await.unwrap();
        set_group_platforms(&db, g1.id, &[
            GroupPlatformInput { platform_id: p_a.id, priority: Some(0), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_b.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();
        set_group_platforms(&db, g2.id, &[
            GroupPlatformInput { platform_id: p_b.id, priority: Some(0), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        let r = purge_auto_disabled_platforms(&db, Some(g1.id)).await.unwrap();
        // 独占本分组的 A 被永久删；共享的 B 仅从 g1 移除关联。
        assert_eq!(r.deleted_ids, vec![p_a.id as u64], "独占本分组的 A 应永久删");
        assert_eq!(r.unassigned_ids, vec![p_b.id as u64], "共享的 B 应仅移除本分组关联");

        // A 已软删；B 行仍在。
        assert!(get_platform(&db, p_a.id).await.unwrap().is_none(), "A 应被软删");
        assert!(get_platform(&db, p_b.id).await.unwrap().is_some(), "B 行应保留");

        // g1 成员已空（A 删 + B 移除）；g2 仍保留 B。
        let g1_plats = get_group_platforms(&db, g1.id).await.unwrap();
        assert!(g1_plats.is_empty(), "g1 应无成员残留");
        let g2_plats = get_group_platforms(&db, g2.id).await.unwrap();
        assert_eq!(g2_plats.len(), 1, "g2 应仍含 B");
        assert_eq!(g2_plats[0].platform.id, p_b.id);

        // 全表无指向已删平台 A 的关联残留（delete_platform 清所有 group_platform）。
        let pid_a = p_a.id as i64;
        let stale_a: i64 = db.call_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM group_platform WHERE platform_id = ?1", params![pid_a], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(stale_a, 0, "A 不应残留任何 group_platform 行");
    }



    #[tokio::test]
    async fn purge_auto_disabled_group_skips_enabled() {
        let db = test_db().await;
        // 本分组含一个 enabled 平台 + 一个 auto_disabled 平台。
        let p_alive = create_platform(&db, sample_platform("alive-g")).await.unwrap();
        let p_dead = create_platform(&db, sample_platform("dead-g")).await.unwrap();
        set_platform_auto_disabled(&db, p_dead.id).await.unwrap();
        let g = create_group(&db, sample_group("g", vec![])).await.unwrap();
        set_group_platforms(&db, g.id, &[
            GroupPlatformInput { platform_id: p_alive.id, priority: Some(0), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_dead.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        let r = purge_auto_disabled_platforms(&db, Some(g.id)).await.unwrap();
        assert_eq!(r.deleted_ids, vec![p_dead.id as u64], "仅 auto_disabled 应被删");
        assert!(r.unassigned_ids.is_empty());

        // enabled 平台行 + 分组关联保留。
        assert!(get_platform(&db, p_alive.id).await.unwrap().is_some());
        let g_plats = get_group_platforms(&db, g.id).await.unwrap();
        assert_eq!(g_plats.len(), 1, "g 应仅余 enabled 平台");
        assert_eq!(g_plats[0].platform.id, p_alive.id);
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
        }).await.unwrap();
        assert_eq!(updated.base_url, "https://crud.example/v2");
        assert_eq!(get_platform(&db, created.id).await.unwrap().unwrap().base_url, "https://crud.example/v2");

        // delete（软删）→ list 不含、get None
        delete_platform(&db, created.id).await.unwrap();
        assert_eq!(list_platforms(&db).await.unwrap().len(), 0);
        assert!(get_platform(&db, created.id).await.unwrap().is_none());
    }



    /// 缓存正确性（问题2）：setting 写后读返回新值（失效生效）；
    /// group 写后 list_groups 返回新集合（不返回陈旧缓存）。
    #[tokio::test]
    async fn hot_cache_invalidates_on_write() {
        let db = test_db().await;
        // ── setting 缓存 ──
        // 先读（不存在 → 缓存 None 槽），再写，再读必须见新值。
        assert!(get_setting(&db, "proxy", "logging").await.unwrap().is_none());
        set_setting(&db, SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: serde_json::json!({"enabled": true}),
        }).await.unwrap();
        let v = get_setting(&db, "proxy", "logging").await.unwrap();
        assert_eq!(v, Some(serde_json::json!({"enabled": true})), "写后读见新值（缓存已失效）");
        // 改值再读。
        set_setting(&db, SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: serde_json::json!({"enabled": false}),
        }).await.unwrap();
        assert_eq!(
            get_setting(&db, "proxy", "logging").await.unwrap(),
            Some(serde_json::json!({"enabled": false})),
        );
        // delete 后读为 None。
        delete_setting(&db, "proxy", "logging").await.unwrap();
        assert!(get_setting(&db, "proxy", "logging").await.unwrap().is_none());

        // ── group 缓存 ──
        assert_eq!(list_groups(&db).await.unwrap().len(), 0);
        let g = create_group(&db, sample_group("gc", vec![])).await.unwrap();
        // 缓存失效 → list_groups 见新建 group。
        let groups = list_groups(&db).await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "gc");
        // 删除后 list_groups 不再含该 group。
        force_delete_group(&db, g.id).await.unwrap();
        assert_eq!(list_groups(&db).await.unwrap().len(), 0);
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
