//! domain 逻辑：scan / import / set_agent / add / delete / resync / update。

use std::collections::BTreeMap;

use super::backend_for;
use super::mask::{mask_env, merge_masked};
use super::types::{
    ImportReport, McpAgent, McpConfigRaw, McpImportPayload, McpScanItem, McpServerInfo,
    McpServerRow, McpTransport, McpUpdatePayload,
};
use crate::gateway::db::{self, Db};

/// 扫描所有 agent 配置，去重合并（同名取首次出现的配置）。
pub async fn scan_all(db: &Db) -> Result<Vec<McpScanItem>, String> {
    let existing: std::collections::HashSet<String> = db::list_mcp_server_names(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

    // name → (cfg, agents found in)
    let mut merged: BTreeMap<String, (McpConfigRaw, Vec<McpAgent>)> = BTreeMap::new();
    for agent in McpAgent::all() {
        let be = backend_for(*agent);
        let entries = be.read_all().unwrap_or_else(|e| {
            tracing::warn!(agent = agent.slug(), error = %e, "mcp scan: read agent config failed");
            vec![]
        });
        for (name, cfg) in entries {
            merged
                .entry(name)
                .or_insert_with(|| (cfg.clone(), vec![]))
                .1
                .push(*agent);
        }
    }

    let items = merged
        .into_iter()
        .map(|(name, (cfg, agents))| {
            let already = existing.contains(&name);
            McpScanItem {
                transport: cfg.transport.as_str().to_string(),
                found_in_agents: agents.iter().map(|a| a.slug().to_string()).collect(),
                already_imported: already,
                env: mask_env(cfg.env),
                headers: mask_env(cfg.headers),
                args: cfg.args,
                command: cfg.command,
                url: cfg.url,
                name,
            }
        })
        .collect();
    Ok(items)
}

/// 导入：每项从 source agent 配置取原值（优先于前端脱敏值），入 DB，enabled = source agent。
pub async fn import_items(db: &Db, items: Vec<McpImportPayload>) -> Result<ImportReport, String> {
    let mut imported = vec![];
    let mut skipped = vec![];
    for item in items {
        let source_agent = match McpAgent::from_slug(&item.source_agent) {
            Some(a) => a,
            None => {
                tracing::warn!(
                    name = %item.name,
                    source_agent = %item.source_agent,
                    "mcp import: unknown source agent slug"
                );
                skipped.push(item.name.clone());
                continue;
            }
        };
        // 从 source agent 配置取原值（env 含真实密钥，前端只拿到 ***）。
        let raw_cfg = match backend_for(source_agent).read_all() {
            Ok(entries) => entries
                .into_iter()
                .find(|(n, _)| n == &item.name)
                .map(|(_, c)| c),
            Err(e) => {
                tracing::warn!(error = %e, "mcp import: read source agent config failed");
                None
            }
        };
        let cfg = match raw_cfg {
            Some(c) => c,
            None => McpConfigRaw {
                // 取不到原值 → 用前端传值（env 可能是 *** 占位）。
                transport: McpTransport::parse(&item.transport),
                command: item.command,
                args: item.args,
                env: item.env,
                url: item.url,
                headers: item.headers,
            },
        };
        let now = db::now();
        let row = McpServerRow {
            id: 0,
            name: item.name.clone(),
            transport: cfg.transport.as_str().to_string(),
            command: cfg.command,
            args_json: serde_json::to_string(&cfg.args).unwrap_or_else(|_| "[]".into()),
            env_json: serde_json::to_string(&cfg.env).unwrap_or_else(|_| "{}".into()),
            url: cfg.url,
            headers_json: serde_json::to_string(&cfg.headers).unwrap_or_else(|_| "{}".into()),
            enabled_agents: source_agent.slug().to_string(),
            created_at: now,
            updated_at: now,
        };
        match db::upsert_mcp_server(db, &row).await {
            Ok(_) => imported.push(item.name),
            Err(e) => {
                tracing::warn!(error = %e, "mcp import: upsert failed");
                skipped.push(item.name);
            }
        }
    }
    Ok(ImportReport { imported, skipped })
}

/// per-agent 启用/禁用：改 DB enabled_agents + 同步写/删 agent 配置。
pub async fn set_agent_enabled(
    db: &Db,
    name: &str,
    agent: McpAgent,
    enabled: bool,
) -> Result<(), String> {
    let row = db::get_mcp_server(db, name)
        .await?
        .ok_or_else(|| format!("mcp server not found: {name}"))?;

    // transport 兼容检查（codex 仅 stdio）。
    let transport = row.transport_enum();
    if enabled && !transport.supported_by(agent) {
        return Err(format!(
            "transport {} not supported by {}",
            transport.as_str(),
            agent.display()
        ));
    }

    let mut agents = row.enabled_set();
    if enabled {
        if !agents.contains(&agent) {
            agents.push(agent);
        }
    } else {
        agents.retain(|a| *a != agent);
    }
    let csv = agents
        .iter()
        .map(|a| a.slug())
        .collect::<Vec<_>>()
        .join(",");
    db::set_mcp_server_enabled_agents(db, name, &csv).await?;

    // 同步 agent 配置文件。
    let be = backend_for(agent);
    if enabled {
        let cfg = row.to_raw_cfg();
        be.write(name, &cfg)?;
    } else {
        be.remove(name)?;
    }
    Ok(())
}

/// 手动添加：校验 name 非空/不重复 → upsert（enabled_agents 空，不写任何 agent 配置）。
/// 用户添加后通过 set_agent_enabled 逐 agent 启用（那时才写配置）。
pub async fn add_server(db: &Db, payload: McpUpdatePayload) -> Result<McpServerInfo, String> {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err("name is required".into());
    }
    if db::get_mcp_server(db, &name).await?.is_some() {
        return Err(format!("mcp server already exists: {name}"));
    }
    let transport = McpTransport::parse(&payload.transport);
    let cfg = McpConfigRaw {
        transport,
        command: payload.command,
        args: payload.args,
        env: payload.env,
        url: payload.url,
        headers: payload.headers,
    };
    let now = db::now();
    let row = McpServerRow {
        id: 0,
        name: name.clone(),
        transport: transport.as_str().to_string(),
        command: cfg.command,
        args_json: serde_json::to_string(&cfg.args).unwrap_or_else(|_| "[]".into()),
        env_json: serde_json::to_string(&cfg.env).unwrap_or_else(|_| "{}".into()),
        url: cfg.url,
        headers_json: serde_json::to_string(&cfg.headers).unwrap_or_else(|_| "{}".into()),
        enabled_agents: String::new(),
        created_at: now,
        updated_at: now,
    };
    db::upsert_mcp_server(db, &row).await?;
    db::get_mcp_server(db, &name)
        .await?
        .map(McpServerInfo::from)
        .ok_or_else(|| format!("mcp server not found after insert: {name}"))
}

/// 删除：从所有 enabled agent 配置移除 + DB 删行。
pub async fn delete_server(db: &Db, name: &str) -> Result<(), String> {
    let row = db::get_mcp_server(db, name).await?;
    if let Some(row) = &row {
        for agent in row.enabled_set() {
            let be = backend_for(agent);
            if let Err(e) = be.remove(name) {
                tracing::warn!(
                    agent = agent.slug(),
                    error = %e,
                    "mcp delete: remove from agent config failed"
                );
            }
        }
    }
    db::delete_mcp_server(db, name).await?;
    Ok(())
}

/// 重新同步：遍历所有 MCP server，对每个 enabled agent 从 DB 全量重写 agent 配置文件。
/// 修复 agent 配置文件被外部（CLI / app / 手动）污染导致的失效（如 env:null 致 Claude Code 跳过 server）。
/// aidog 的 write 恒为全量 replace（build_claude_entry 重建 entry），所以重写 = 用 DB 干净值覆盖文件。
/// 返回成功重写的 (agent, name) 数量；单条失败记 warn 不中断（best-effort）。
pub async fn resync_all(db: &Db) -> Result<usize, String> {
    let rows = db::list_mcp_servers(db).await?;
    let mut count = 0usize;
    for row in rows {
        for agent in row.enabled_set() {
            let be = backend_for(agent);
            let cfg = row.to_raw_cfg();
            match be.write(&row.name, &cfg) {
                Ok(()) => count += 1,
                Err(e) => tracing::warn!(
                    agent = agent.slug(),
                    server = %row.name,
                    error = %e,
                    "mcp resync: write agent config failed"
                ),
            }
        }
    }
    Ok(count)
}

/// 编辑 MCP：全字段更新（含改名 / transport 切换）+ 同步 enabled agent 配置。
/// - env/headers 脱敏 merge（***→旧 DB 明文）
/// - transport 切换后不支持的 enabled agent 自动移除（agent 配置 remove）
/// - 改名：旧 name agent 配置 remove + DB 旧名删行
/// - upsert 新 row（保留 id/created_at；改名时新行 created_at 沿用旧值）
pub async fn update_server(
    db: &Db,
    old_name: &str,
    payload: McpUpdatePayload,
) -> Result<McpServerInfo, String> {
    let old = db::get_mcp_server(db, old_name)
        .await?
        .ok_or_else(|| format!("mcp server not found: {old_name}"))?;

    let new_transport = McpTransport::parse(&payload.transport);

    let env = merge_masked(payload.env, &old.env_map());
    let headers = merge_masked(payload.headers, &old.headers_map());

    let cfg = McpConfigRaw {
        transport: new_transport,
        command: payload.command.clone(),
        args: payload.args.clone(),
        env: env.clone(),
        url: payload.url.clone(),
        headers: headers.clone(),
    };

    // transport 兼容重算：不支持的 enabled agent 移除。
    let enabled = old.enabled_set();
    let kept: Vec<McpAgent> = enabled
        .iter()
        .copied()
        .filter(|a| new_transport.supported_by(*a))
        .collect();
    let dropped: Vec<McpAgent> = enabled
        .into_iter()
        .filter(|a| !new_transport.supported_by(*a))
        .collect();

    // dropped: 旧 name 配置 remove。
    for agent in &dropped {
        if let Err(e) = backend_for(*agent).remove(old_name) {
            tracing::warn!(
                agent = agent.slug(),
                error = %e,
                "mcp update: remove dropped agent config failed"
            );
        }
    }
    // kept: 改名则先 remove 旧 name，再 write 新 name 新 cfg。
    for agent in &kept {
        let be = backend_for(*agent);
        if payload.name != old.name {
            if let Err(e) = be.remove(old_name) {
                tracing::warn!(
                    agent = agent.slug(),
                    error = %e,
                    "mcp update: remove old-name agent config failed"
                );
            }
        }
        be.write(&payload.name, &cfg)
            .map_err(|e| format!("write {} config: {e}", agent.slug()))?;
    }

    // DB：改名删旧 + upsert 新。
    if payload.name != old.name {
        db::delete_mcp_server(db, old_name).await?;
    }
    let enabled_csv = kept
        .iter()
        .map(|a| a.slug())
        .collect::<Vec<_>>()
        .join(",");
    let now = db::now();
    let row = McpServerRow {
        id: old.id,
        name: payload.name,
        transport: new_transport.as_str().to_string(),
        command: cfg.command,
        args_json: serde_json::to_string(&cfg.args).unwrap_or_else(|_| "[]".into()),
        env_json: serde_json::to_string(&env).unwrap_or_else(|_| "{}".into()),
        url: cfg.url,
        headers_json: serde_json::to_string(&headers).unwrap_or_else(|_| "{}".into()),
        enabled_agents: enabled_csv,
        created_at: old.created_at,
        updated_at: now,
    };
    db::upsert_mcp_server(db, &row).await?;

    db::get_mcp_server(db, &row.name)
        .await?
        .map(McpServerInfo::from)
        .ok_or_else(|| format!("mcp update: row vanished after upsert: {}", row.name))
}

#[cfg(test)]
#[path = "test_domain.rs"]
mod test_domain;
