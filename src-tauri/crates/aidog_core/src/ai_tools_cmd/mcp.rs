use crate::gateway;

crate::tauri_command! {
    /// 列出 DB 中所有 MCP server（env/headers 已脱敏）。
    pub async fn mcp_list() -> Result<Vec<gateway::mcp::McpServerInfo>, String> {
    let db = aidog_ctx::db();
        let rows = aidog_mcp::store::list_mcp_servers(&db).await?;
        Ok(rows.into_iter().map(gateway::mcp::McpServerInfo::from).collect())
    }
}

crate::tauri_command! {
    /// 扫描 Claude Code + Codex 配置的所有 MCP，去重合并（env/headers 已脱敏）。
    pub async fn mcp_scan() -> Result<Vec<gateway::mcp::McpScanItem>, String> {
    let db = aidog_ctx::db();
        gateway::mcp::scan_all(&db).await
    }
}

crate::tauri_command! {
    /// 批量导入 MCP（从 agent 配置取原值入 DB，enabled = source agent）。
    pub async fn mcp_import(
        items: Vec<gateway::mcp::McpImportPayload>) -> Result<gateway::mcp::ImportReport, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_import", count = items.len(), "command invoked");
        gateway::mcp::import_items(&db, items).await
    }
}

crate::tauri_command! {
    /// 粘贴 JSON 导入 MCP（claude.json 协议）：解析 → 入库（enabled 空，不写 agent 配置；同名跳过）。
    pub async fn mcp_import_json(
        json: String) -> Result<gateway::mcp::ImportReport, String> {
    let db = aidog_ctx::db();
        gateway::mcp::import_pasted(&db, &json).await
    }
}

crate::tauri_command! {
    /// per-agent 启用/禁用：改 DB enabled_agents + 同步写/删 agent 配置。
    pub async fn mcp_set_agent(
        name: String,
        agent: String,
        enabled: bool) -> Result<(), String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_set_agent", name = %name, agent = %agent, enabled, "command invoked");
        let agent = gateway::mcp::McpAgent::from_slug(&agent)
            .ok_or_else(|| format!("unknown agent slug: {agent}"))?;
        gateway::mcp::set_agent_enabled(&db, &name, agent, enabled).await
    }
}

crate::tauri_command! {
    /// 删除 MCP：DB + 所有 enabled agent 配置（破坏性，前端二次确认）。
    pub async fn mcp_delete( name: String) -> Result<(), String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_delete", name = %name, "command invoked");
        gateway::mcp::delete_server(&db, &name).await
    }
}

crate::tauri_command! {
    /// 手动添加 MCP：校验 name 唯一 → 入库（enabled 空，不写 agent 配置）。
    pub async fn mcp_add(
        payload: gateway::mcp::McpUpdatePayload) -> Result<gateway::mcp::McpServerInfo, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_add", name = %payload.name, "command invoked");
        gateway::mcp::add_server(&db, payload).await
    }
}

crate::tauri_command! {
    /// 编辑 MCP：全字段更新（含改名/transport 切换）+ 同步 enabled agent 配置。
    pub async fn mcp_update(
        old_name: String,
        payload: gateway::mcp::McpUpdatePayload) -> Result<gateway::mcp::McpServerInfo, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_update", old = %old_name, "command invoked");
        gateway::mcp::update_server(&db, &old_name, payload).await
    }
}

crate::tauri_command! {
    /// 重新同步全部：从 DB 全量重写所有 enabled agent 的 MCP 配置文件，
    /// 修复外部污染（如 env:null 致 Claude Code 跳过 server）。返回重写条数。
    pub async fn mcp_resync() -> Result<usize, String> {
    let db = aidog_ctx::db();
        gateway::mcp::resync_all(&db).await
    }
}

crate::tauri_command! {
    /// 导出单 MCP 可分享对象（claude.json 协议 `{mcpServers:{name:entry}}`，明文含 env/headers）。
    /// 接收端走 mcp_import_json，格式自洽。本地操作，不落 proxy_log。
    pub async fn mcp_share_export( name: String) -> Result<serde_json::Value, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_share_export", name = %name, "command invoked");
        gateway::mcp::share_server(&db, &name).await
    }
}

#[cfg(test)]
#[path = "test_mcp.rs"]
mod test_mcp;
