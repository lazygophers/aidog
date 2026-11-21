//! 会话详情的主线 / 子代理分组（票 04）。
//!
//! Claude Code：子代理文件 `<sessionId>/subagents/agent-<id>.jsonl` 各成一组，名称读同目录
//! `agent-<id>.meta.json` 的 `agentType`（只读该键；`description` 属派发文本，不读）。
//! 其余客户端只按 `is_sidechain` 分主线 / 子代理两组。不迁移、不重扫。

use std::path::Path;

use aidog_db::{ClientUsageAgentGroup, ClientUsageFileGroup};

/// 输入按首次调用时间升序；输出主线在前，其余保持输入顺序。
pub(crate) fn group_agents(client_code: &str, files: Vec<ClientUsageFileGroup>) -> Vec<ClientUsageAgentGroup> {
    let mut out: Vec<ClientUsageAgentGroup> = Vec::new();
    for f in files {
        let agent_id = if client_code == "claude_code" { claude_agent_id(&f.source_file_id) } else { None };
        let kind = if f.is_sidechain || agent_id.is_some() { "subagent" } else { "main" };
        let i = match out.iter().position(|g| g.kind == kind && g.agent_id == agent_id) {
            Some(i) => i,
            None => {
                let agent_type = agent_id.as_deref().and_then(|id| read_agent_type(&f.source_file_id, id));
                out.push(ClientUsageAgentGroup { kind: kind.into(), agent_id, agent_type, ..Default::default() });
                out.len() - 1
            }
        };
        let g = &mut out[i];
        g.calls += f.calls;
        g.input_tokens += f.input_tokens;
        g.output_tokens += f.output_tokens;
        g.cache_read_tokens += f.cache_read_tokens;
        g.cache_write_tokens += f.cache_write_tokens;
        g.cost += f.cost;
        g.unpriced_calls += f.unpriced_calls;
    }
    out.sort_by_key(|g| g.kind != "main");
    out
}

/// `…/subagents/agent-<id>.jsonl` → `<id>`；其它路径 None。
fn claude_agent_id(source_file_id: &str) -> Option<String> {
    let p = Path::new(source_file_id);
    if p.parent()?.file_name()? != "subagents" {
        return None;
    }
    let id = p.file_name()?.to_str()?.strip_prefix("agent-")?.strip_suffix(".jsonl")?;
    (!id.is_empty()).then(|| id.to_string())
}

/// 同目录 `agent-<id>.meta.json` 的 `agentType`；缺文件 / 坏 JSON / 空值 → None。
fn read_agent_type(source_file_id: &str, id: &str) -> Option<String> {
    let meta = Path::new(source_file_id).with_file_name(format!("agent-{id}.meta.json"));
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(meta).ok()?).ok()?;
    let t = v.get("agentType")?.as_str()?.trim();
    (!t.is_empty()).then(|| t.to_string())
}
