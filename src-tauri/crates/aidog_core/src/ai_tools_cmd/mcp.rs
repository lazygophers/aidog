use crate::gateway;
use aidog_db::Db;

crate::tauri_command! {
    /// 列出 DB 中所有 MCP server（env/headers 已脱敏）。
    pub async fn mcp_list() -> Result<Vec<gateway::mcp::McpServerInfo>, String> {
    let db = aidog_ctx::db();
        let rows = aidog_mcp::store::list_mcp_servers(db).await?;
        Ok(rows.into_iter().map(gateway::mcp::McpServerInfo::from).collect())
    }
}

crate::tauri_command! {
    /// 扫描 Claude Code + Codex 配置的所有 MCP，去重合并（env/headers 已脱敏）。
    pub async fn mcp_scan() -> Result<Vec<gateway::mcp::McpScanItem>, String> {
    let db = aidog_ctx::db();
        gateway::mcp::scan_all(db).await
    }
}

crate::tauri_command! {
    /// 批量导入 MCP（从 agent 配置取原值入 DB，enabled = source agent）。
    pub async fn mcp_import(
        items: Vec<gateway::mcp::McpImportPayload>) -> Result<gateway::mcp::ImportReport, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_import", count = items.len(), "command invoked");
        gateway::mcp::import_items(db, items).await
    }
}

crate::tauri_command! {
    /// 粘贴 JSON 导入 MCP（claude.json 协议）：解析 → 入库（enabled 空，不写 agent 配置；同名跳过）。
    pub async fn mcp_import_json(
        json: String) -> Result<gateway::mcp::ImportReport, String> {
    let db = aidog_ctx::db();
        gateway::mcp::import_pasted(db, &json).await
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
        gateway::mcp::set_agent_enabled(db, &name, agent, enabled).await
    }
}

crate::tauri_command! {
    /// 删除 MCP：DB + 所有 enabled agent 配置（破坏性，前端二次确认）。
    pub async fn mcp_delete( name: String) -> Result<(), String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_delete", name = %name, "command invoked");
        gateway::mcp::delete_server(db, &name).await
    }
}

crate::tauri_command! {
    /// 手动添加 MCP：校验 name 唯一 → 入库（enabled 空，不写 agent 配置）。
    pub async fn mcp_add(
        payload: gateway::mcp::McpUpdatePayload) -> Result<gateway::mcp::McpServerInfo, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_add", name = %payload.name, "command invoked");
        gateway::mcp::add_server(db, payload).await
    }
}

crate::tauri_command! {
    /// 编辑 MCP：全字段更新（含改名/transport 切换）+ 同步 enabled agent 配置。
    pub async fn mcp_update(
        old_name: String,
        payload: gateway::mcp::McpUpdatePayload) -> Result<gateway::mcp::McpServerInfo, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_update", old = %old_name, "command invoked");
        gateway::mcp::update_server(db, &old_name, payload).await
    }
}

crate::tauri_command! {
    /// 重新同步全部：从 DB 全量重写所有 enabled agent 的 MCP 配置文件，
    /// 修复外部污染（如 env:null 致 Claude Code 跳过 server）。返回重写条数。
    pub async fn mcp_resync() -> Result<usize, String> {
    let db = aidog_ctx::db();
        gateway::mcp::resync_all(db).await
    }
}

crate::tauri_command! {
    /// 导出单 MCP 可分享对象（claude.json 协议 `{mcpServers:{name:entry}}`，明文含 env/headers）。
    /// 接收端走 mcp_import_json，格式自洽。本地操作，不落 proxy_log。
    pub async fn mcp_share_export( name: String) -> Result<serde_json::Value, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_share_export", name = %name, "command invoked");
        gateway::mcp::share_server(db, &name).await
    }
}

crate::tauri_command! {
    /// 推荐安装清单：远程缓存（settings mcp/recommended）→ 编译期内置兜底
    /// （installed 态由前端对齐 mcp_list；本地操作，不落 proxy_log）。
    pub async fn mcp_recommended_list(
    ) -> Result<Vec<gateway::mcp::RecommendedEntry>, String> {
    let db = aidog_ctx::db();
        gateway::mcp_recommend::recommended_list(db).await
    }
}

/// `mcp_self_install` 报告：写进了哪个名字、用哪条命令、启用了哪些 agent。
#[derive(serde::Serialize, Clone, Debug)]
pub struct McpSelfInstallReport {
    /// 写入的 MCP server 名（固定 `aidog`）。
    pub name: String,
    /// 写入配置的 HTTP MCP 地址（`http://127.0.0.1:<代理端口>/mcp`）。
    pub url: String,
    /// 启用的 agent slug（claude-code / codex）。
    pub agents: Vec<String>,
}

/// `aidog` 这个 MCP server 条目名（写进用户 `~/.claude.json` / `~/.codex/config.toml`
/// 的 `mcpServers` 键名）。同名已有条目会被更新（一键安装语义）。
pub(crate) const SELF_INSTALL_NAME: &str = "aidog";

/// 一键写入本机 agent 配置：把 HTTP MCP（`http://127.0.0.1:<代理端口>/mcp`，
/// Authorization: Bearer <默认分组 key，无默认取第一个分组>）写进本机 Claude Code
/// （`~/.claude.json`）与 Codex（`~/.codex/config.toml`）的 mcpServers，复用 aidog_mcp 的
/// add_server / update_server / set_agent_enabled（按名字键整文件读改写、只动本条目，
/// 不覆盖用户其它 mcpServers）。同名旧 stdio 条目（`aidog-kernel mcp`）被整条替换。
/// 工具设置由 `/mcp` 端点直接读库，不再嵌入配置文件。幂等：条目已存在则更新。
pub async fn self_install(db: &Db) -> Result<McpSelfInstallReport, String> {
    let groups = aidog_db::list_groups(db).await?;
    let group = groups
        .iter()
        .find(|g| g.is_default)
        .or_else(|| groups.first())
        .ok_or("尚无分组：先创建一个分组（HTTP MCP 用分组 key 鉴权）")?;
    let port = crate::shared::load_proxy_settings(db).await?.port;
    let url = format!("http://127.0.0.1:{port}/mcp");
    let payload = gateway::mcp::McpUpdatePayload {
        name: SELF_INSTALL_NAME.into(),
        transport: "http".into(),
        command: String::new(),
        args: vec![],
        env: Default::default(),
        url: url.clone(),
        headers: std::collections::BTreeMap::from([(
            "Authorization".into(),
            format!("Bearer {}", group.group_key),
        )]),
    };
    // 已有同名条目（上一次一键安装 / 用户手加）→ 更新；否则新增。
    if gateway::mcp::store::get_mcp_server(db, SELF_INSTALL_NAME).await?.is_some() {
        gateway::mcp::update_server(db, SELF_INSTALL_NAME, payload).await?;
    } else {
        gateway::mcp::add_server(db, payload).await?;
    }
    const AGENTS: [gateway::mcp::McpAgent; 2] =
        [gateway::mcp::McpAgent::ClaudeCode, gateway::mcp::McpAgent::Codex];
    for agent in AGENTS {
        gateway::mcp::set_agent_enabled(db, SELF_INSTALL_NAME, agent, true).await?;
    }
    Ok(McpSelfInstallReport {
        name: SELF_INSTALL_NAME.into(),
        url,
        agents: AGENTS.iter().map(|a| a.slug().to_string()).collect(),
    })
}

crate::tauri_command! {
    /// 一键写入本机 agent 配置：把 HTTP MCP 地址 + 分组 Bearer 写进 Claude Code /
    /// Codex 的 mcpServers（幂等；同名旧 stdio 条目被替换）。
    pub async fn mcp_self_install() -> Result<McpSelfInstallReport, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "mcp_self_install", "command invoked");
        self_install(db).await
    }
}

#[cfg(test)]
#[path = "test_mcp.rs"]
mod test_mcp;
