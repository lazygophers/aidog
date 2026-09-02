//! 下架平台启动期清理（DELISTED_PLATFORM_CODES）测试。
//! （test_schema.rs 自 eedd2943 拆 crate 起为孤儿模块未编译，勿再往里加测试。）

/// 2026-08-28 registry 下架 20 家中转站后的启动期清理：
/// api_key 空的下架行软删 + 清 group_platform；填过 key 的下架行与其他平台行不动；
/// 主库镜像表（platform_preset / model_entry）对应 code 行无条件清。
#[test]
fn cleanup_delisted_platform_rows_soft_deletes_unconfigured_only() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    super::run_migrations_platform_early(&conn).unwrap();
    // 两行下架平台：一个从未填 key（裸 wire 名形态），一个填过 key（带引号 JSON 串形态）
    conn.execute(
        "INSERT INTO platform (name, platform_type, api_key, created_at, updated_at, deleted_at)
             VALUES ('micu-empty', 'micu', '', 0, 0, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO platform (name, platform_type, api_key, created_at, updated_at, deleted_at)
             VALUES ('dmxapi-in-use', '\"dmxapi\"', 'sk-live', 0, 0, 0)",
        [],
    )
    .unwrap();
    // 保留平台（不在清单）：即使无 key 也不动
    conn.execute(
        "INSERT INTO platform (name, platform_type, api_key, created_at, updated_at, deleted_at)
             VALUES ('pipellm', 'pipellm', '', 0, 0, 0)",
        [],
    )
    .unwrap();
    // 空配置行的组成员关系应随软删清除
    let pid = conn.last_insert_rowid();
    let _ = pid;
    let empty_id: i64 = conn
        .query_row(
            "SELECT id FROM platform WHERE name = 'micu-empty'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO \"group\" (name, created_at, updated_at, deleted_at) VALUES ('g1', 0, 0, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO group_platform (group_id, platform_id, created_at) VALUES (1, ?1, 0)",
        [empty_id],
    )
    .unwrap();

    super::cleanup_delisted_platform_rows(&conn);

    let soft_deleted: i64 = conn
        .query_row(
            "SELECT deleted_at FROM platform WHERE name = 'micu-empty'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(soft_deleted > 0, "未配置的下架行应被软删");
    let kept: i64 = conn
        .query_row(
            "SELECT deleted_at FROM platform WHERE name = 'dmxapi-in-use'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kept, 0, "填过 key 的下架行必须保留");
    let untouched: i64 = conn
        .query_row(
            "SELECT deleted_at FROM platform WHERE name = 'pipellm'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(untouched, 0, "非清单平台不受影响");
    let memberships: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM group_platform WHERE platform_id = ?1",
            [empty_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(memberships, 0, "软删行的组成员关系应被清除");

    // 幂等：重跑无新变化
    super::cleanup_delisted_platform_rows(&conn);
    let again: i64 = conn
        .query_row(
            "SELECT deleted_at FROM platform WHERE name = 'micu-empty'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(again, soft_deleted, "重跑不得重复置位");
}

/// 主库镜像表清理：下架 code 行无条件删，其他 code 保留。
#[test]
fn cleanup_delisted_registry_mirror_rows_removes_only_delisted_codes() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE platform_preset (
                code TEXT NOT NULL PRIMARY KEY, preset_data TEXT NOT NULL DEFAULT '{}',
                created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
                deleted_at INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE model_entry (
                platform_code TEXT NOT NULL, model_id TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (platform_code, model_id));",
    )
    .unwrap();
    for code in ["micu", "packycode", "pipellm"] {
        conn.execute("INSERT INTO platform_preset (code) VALUES (?1)", [code])
            .unwrap();
        conn.execute(
            "INSERT INTO model_entry (platform_code) VALUES (?1)",
            [code],
        )
        .unwrap();
    }
    super::cleanup_delisted_registry_mirror_rows(&conn);
    let presets: Vec<String> = conn
        .prepare("SELECT code FROM platform_preset ORDER BY code")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        presets,
        vec!["pipellm".to_string()],
        "镜像表只留下架清单外的 code"
    );
    let entries: Vec<String> = conn
        .prepare("SELECT platform_code FROM model_entry ORDER BY platform_code")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(entries, vec!["pipellm".to_string()]);
}
