use aidog_db::{Db, now};
use rusqlite::params;

/// db-field-encryption 票 02：读侧 env_json/headers_json 解密（非密文前缀按明文兼容，
/// 见 crypt.rs）。key 不可得 / 密文损坏 → 行级清晰报错，不 panic（同 platform.api_key idiom）。
fn decrypt_json_col(idx: usize, raw: String) -> rusqlite::Result<String> {
    aidog_db::crypt::decrypt_field(&raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            idx,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::other(e)),
        )
    })
}

#[track_caller]
pub fn list_mcp_servers(
    db: &Db,
) -> impl std::future::Future<Output = Result<Vec<crate::McpServerRow>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, transport, command, args_json, env_json, url, headers_json, \
             enabled_agents, created_at, updated_at FROM mcp_server ORDER BY name",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(crate::McpServerRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    transport: r.get(2)?,
                    command: r.get(3)?,
                    args_json: r.get(4)?,
                    env_json: decrypt_json_col(5, r.get(5)?)?,
                    url: r.get(6)?,
                    headers_json: decrypt_json_col(7, r.get(7)?)?,
                    enabled_agents: r.get(8)?,
                    created_at: r.get(9)?,
                    updated_at: r.get(10)?,
                })
            })?;
            let mut out = vec![];
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| format!("list mcp servers: {e}"))
    }
}

#[track_caller]
pub fn get_mcp_server<'a>(
    db: &'a Db,
    name: &'a str,
) -> impl std::future::Future<Output = Result<Option<crate::McpServerRow>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let name = name.to_string();
        db.call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, transport, command, args_json, env_json, url, headers_json, \
             enabled_agents, created_at, updated_at FROM mcp_server WHERE name = ?1",
            )?;
            let mut rows = stmt.query_map(params![name], |r| {
                Ok(crate::McpServerRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    transport: r.get(2)?,
                    command: r.get(3)?,
                    args_json: r.get(4)?,
                    env_json: decrypt_json_col(5, r.get(5)?)?,
                    url: r.get(6)?,
                    headers_json: decrypt_json_col(7, r.get(7)?)?,
                    enabled_agents: r.get(8)?,
                    created_at: r.get(9)?,
                    updated_at: r.get(10)?,
                })
            })?;
            match rows.next() {
                Some(r) => Ok(Some(r?)),
                None => Ok(None),
            }
        })
        .await
        .map_err(|e| format!("get mcp server: {e}"))
    }
}

/// INSERT 或 UPDATE（按 name 唯一冲突）。created_at 仅首次写入生效（UPDATE 不覆盖）。
#[track_caller]
pub fn upsert_mcp_server<'a>(
    db: &'a Db,
    row: &'a crate::McpServerRow,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let row = row.clone();
        // db-field-encryption 票 02：两 JSON 列整串加密落库（空串保持空，防双重加密，见 encrypt_field）
        let env_db = aidog_db::crypt::encrypt_field(&row.env_json)
            .map_err(|e| format!("upsert mcp server: encrypt env_json: {e}"))?;
        let headers_db = aidog_db::crypt::encrypt_field(&row.headers_json)
            .map_err(|e| format!("upsert mcp server: encrypt headers_json: {e}"))?;
        db.call_traced(None, __db_caller, move |conn| {
        conn.execute(
            "INSERT INTO mcp_server \
             (name, transport, command, args_json, env_json, url, headers_json, enabled_agents, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(name) DO UPDATE SET \
               transport=excluded.transport, command=excluded.command, args_json=excluded.args_json, \
               env_json=excluded.env_json, url=excluded.url, headers_json=excluded.headers_json, \
               enabled_agents=excluded.enabled_agents, updated_at=excluded.updated_at",
            params![
                row.name,
                row.transport,
                row.command,
                row.args_json,
                env_db,
                row.url,
                headers_db,
                row.enabled_agents,
                row.created_at,
                row.updated_at
            ],
        )?;
        Ok(())
    })
    .await
    .map_err(|e| format!("upsert mcp server: {e}"))
    }
}

#[track_caller]
pub fn delete_mcp_server<'a>(
    db: &'a Db,
    name: &'a str,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let name = name.to_string();
        db.call_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM mcp_server WHERE name = ?1", params![name])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete mcp server: {e}"))
    }
}

#[track_caller]
pub fn set_mcp_server_enabled_agents<'a>(
    db: &'a Db,
    name: &'a str,
    agents_csv: &'a str,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let name = name.to_string();
        let csv = agents_csv.to_string();
        db.call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE mcp_server SET enabled_agents = ?1, updated_at = ?2 WHERE name = ?3",
                params![csv, now(), name],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("set mcp enabled agents: {e}"))
    }
}

#[track_caller]
pub fn list_mcp_server_names(
    db: &Db,
) -> impl std::future::Future<Output = Result<Vec<String>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare("SELECT name FROM mcp_server")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = vec![];
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| format!("list mcp server names: {e}"))
    }
}

// ─── Tests: DB Schema v2 规范固化 ──────────────────────────

// 507facaf5 调整 mod 声明时丢失（test_store.rs 存在但从未编译），票 02 复测时补回。
#[cfg(test)]
#[path = "test_store.rs"]
mod test_store;
