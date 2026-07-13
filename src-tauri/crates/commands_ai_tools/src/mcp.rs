use aidog_core::gateway::{self, db::Db};
use tauri::State;


/// 列出 DB 中所有 MCP server（env/headers 已脱敏）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn mcp_list(db: State<'_, Db>) -> Result<Vec<gateway::mcp::McpServerInfo>, String> {
    tracing::debug!(command = "mcp_list", "command invoked");
    let rows = gateway::db::list_mcp_servers(&db).await?;
    Ok(rows.into_iter().map(gateway::mcp::McpServerInfo::from).collect())
}

/// 扫描 Claude Code + Codex 配置的所有 MCP，去重合并（env/headers 已脱敏）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn mcp_scan(db: State<'_, Db>) -> Result<Vec<gateway::mcp::McpScanItem>, String> {
    tracing::debug!(command = "mcp_scan", "command invoked");
    gateway::mcp::scan_all(&db).await
}

/// 批量导入 MCP（从 agent 配置取原值入 DB，enabled = source agent）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn mcp_import(
    db: State<'_, Db>,
    items: Vec<gateway::mcp::McpImportPayload>,
) -> Result<gateway::mcp::ImportReport, String> {
    tracing::debug!(command = "mcp_import", count = items.len(), "command invoked");
    gateway::mcp::import_items(&db, items).await
}

/// 粘贴 JSON 导入 MCP（claude.json 协议）：解析 → 入库（enabled 空，不写 agent 配置；同名跳过）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn mcp_import_json(
    db: State<'_, Db>,
    json: String,
) -> Result<gateway::mcp::ImportReport, String> {
    tracing::debug!(command = "mcp_import_json", "command invoked");
    gateway::mcp::import_pasted(&db, &json).await
}

/// per-agent 启用/禁用：改 DB enabled_agents + 同步写/删 agent 配置。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn mcp_set_agent(
    db: State<'_, Db>,
    name: String,
    agent: String,
    enabled: bool,
) -> Result<(), String> {
    tracing::debug!(command = "mcp_set_agent", name = %name, agent = %agent, enabled, "command invoked");
    let agent = gateway::mcp::McpAgent::from_slug(&agent)
        .ok_or_else(|| format!("unknown agent slug: {agent}"))?;
    gateway::mcp::set_agent_enabled(&db, &name, agent, enabled).await
}

/// 删除 MCP：DB + 所有 enabled agent 配置（破坏性，前端二次确认）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn mcp_delete(db: State<'_, Db>, name: String) -> Result<(), String> {
    tracing::debug!(command = "mcp_delete", name = %name, "command invoked");
    gateway::mcp::delete_server(&db, &name).await
}

/// 手动添加 MCP：校验 name 唯一 → 入库（enabled 空，不写 agent 配置）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn mcp_add(
    db: State<'_, Db>,
    payload: gateway::mcp::McpUpdatePayload,
) -> Result<gateway::mcp::McpServerInfo, String> {
    tracing::debug!(command = "mcp_add", name = %payload.name, "command invoked");
    gateway::mcp::add_server(&db, payload).await
}

/// 编辑 MCP：全字段更新（含改名/transport 切换）+ 同步 enabled agent 配置。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn mcp_update(
    db: State<'_, Db>,
    old_name: String,
    payload: gateway::mcp::McpUpdatePayload,
) -> Result<gateway::mcp::McpServerInfo, String> {
    tracing::debug!(command = "mcp_update", old = %old_name, "command invoked");
    gateway::mcp::update_server(&db, &old_name, payload).await
}

/// 重新同步全部：从 DB 全量重写所有 enabled agent 的 MCP 配置文件，
/// 修复外部污染（如 env:null 致 Claude Code 跳过 server）。返回重写条数。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn mcp_resync(db: State<'_, Db>) -> Result<usize, String> {
    tracing::debug!(command = "mcp_resync", "command invoked");
    gateway::mcp::resync_all(&db).await
}

/// 导出单 MCP 可分享对象（claude.json 协议 `{mcpServers:{name:entry}}`，明文含 env/headers）。
/// 接收端走 mcp_import_json，格式自洽。本地操作，不落 proxy_log。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn mcp_share_export(db: State<'_, Db>, name: String) -> Result<serde_json::Value, String> {
    tracing::debug!(command = "mcp_share_export", name = %name, "command invoked");
    gateway::mcp::share_server(&db, &name).await
}

#[cfg(test)]
#[path = "test_mcp.rs"]
mod test_mcp;
