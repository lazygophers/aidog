#![cfg(test)]
use aidog_db::test_support::test_db;
use super::*;
use crate::McpServerRow;

fn row(name: &str) -> McpServerRow {
    McpServerRow {
        id: 0,
        name: name.into(),
        transport: "stdio".into(),
        command: "npx".into(),
        args_json: "[\"-y\",\"pkg\"]".into(),
        env_json: "{}".into(),
        url: String::new(),
        headers_json: "{}".into(),
        enabled_agents: "claude_code".into(),
        created_at: now(),
        updated_at: now(),
    }
}

#[tokio::test]
async fn mcp_crud_lifecycle() {
    let db = test_db().await;

    // empty
    assert!(list_mcp_servers(&db).await.unwrap().is_empty());
    assert!(get_mcp_server(&db, "x").await.unwrap().is_none());
    assert!(list_mcp_server_names(&db).await.unwrap().is_empty());

    // insert
    upsert_mcp_server(&db, &row("srv1")).await.unwrap();
    upsert_mcp_server(&db, &row("srv2")).await.unwrap();
    assert_eq!(list_mcp_servers(&db).await.unwrap().len(), 2);
    assert_eq!(list_mcp_server_names(&db).await.unwrap().len(), 2);

    // get
    let got = get_mcp_server(&db, "srv1").await.unwrap().unwrap();
    assert_eq!(got.command, "npx");

    // update (conflict path)
    let mut updated = row("srv1");
    updated.command = "uvx".into();
    upsert_mcp_server(&db, &updated).await.unwrap();
    assert_eq!(get_mcp_server(&db, "srv1").await.unwrap().unwrap().command, "uvx");

    // set enabled agents
    set_mcp_server_enabled_agents(&db, "srv1", "claude_code,codex").await.unwrap();
    assert_eq!(get_mcp_server(&db, "srv1").await.unwrap().unwrap().enabled_agents, "claude_code,codex");

    // delete
    delete_mcp_server(&db, "srv1").await.unwrap();
    assert!(get_mcp_server(&db, "srv1").await.unwrap().is_none());
    assert_eq!(list_mcp_servers(&db).await.unwrap().len(), 1);
}

// ── db-field-encryption 票 02：env_json/headers_json 列级加密 ──────────

use aidog_db::crypt::ENC_PREFIX;
use aidog_db::test_support::three_file_db;
use rusqlite::params;

/// 直接 sqlite 打开主库读两列原始值（不经行编解码层）。
fn raw_json_cols(dir: &std::path::Path, name: &str) -> (String, String) {
    let conn = rusqlite::Connection::open(dir.join("aidog.db")).unwrap();
    conn.query_row(
        "SELECT env_json, headers_json FROM mcp_server WHERE name = ?1",
        params![name],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

#[tokio::test]
async fn upsert_stores_ciphertext_reads_back_plaintext() {
    let (db, dir) = three_file_db().await;
    let mut r = row("crypt-srv");
    r.env_json = r#"{"API_TOKEN":"sk-mcp-secret-99"}"#.into();
    r.headers_json = r#"{"Authorization":"Bearer bk-mcp-77"}"#.into();
    upsert_mcp_server(&db, &r).await.unwrap();

    let (env_raw, headers_raw) = raw_json_cols(dir.path(), "crypt-srv");
    assert!(env_raw.starts_with(ENC_PREFIX), "env_json 须密文形态: {env_raw}");
    assert!(headers_raw.starts_with(ENC_PREFIX), "headers_json 须密文形态: {headers_raw}");
    assert!(!env_raw.contains("sk-mcp-secret-99"), "列值不得含明文凭据");
    assert!(!headers_raw.contains("bk-mcp-77"), "列值不得含明文凭据");

    // 读回透明解密 + 按 name 定位行为不变
    let got = get_mcp_server(&db, "crypt-srv").await.unwrap().unwrap();
    assert_eq!(got.env_json, r.env_json);
    assert_eq!(got.headers_json, r.headers_json);
    assert_eq!(got.env_map().get("API_TOKEN").map(String::as_str), Some("sk-mcp-secret-99"));
    assert_eq!(list_mcp_servers(&db).await.unwrap()[0].headers_json, r.headers_json);
}

#[tokio::test]
async fn legacy_plaintext_row_reads_transparently() {
    // 存量未迁移行（裸 SQL 塞明文）读侧按明文兼容返回，MCP 启动侧无感知
    let (db, _dir) = three_file_db().await;
    let (env, headers) = (
        r#"{"K":"v-legacy"}"#.to_string(),
        r#"{"H":"h-legacy"}"#.to_string(),
    );
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "INSERT INTO mcp_server (name, transport, command, args_json, env_json, url, headers_json, enabled_agents, created_at, updated_at) \
             VALUES ('legacy-plain', 'stdio', 'npx', '[]', ?1, '', ?2, '', 0, 0)",
            params![env, headers],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    let got = get_mcp_server(&db, "legacy-plain").await.unwrap().unwrap();
    assert_eq!(got.env_map().get("K").map(String::as_str), Some("v-legacy"));
    assert_eq!(got.headers_map().get("H").map(String::as_str), Some("h-legacy"));
}
