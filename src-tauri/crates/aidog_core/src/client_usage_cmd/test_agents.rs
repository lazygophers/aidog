//! 票 04：会话详情主线 / 子代理分组。meta.json 夹具落 tempdir，不碰真 ~/.claude。
use super::agents::group_agents;
use aidog_db::{ClientUsageAgentGroup, ClientUsageFileGroup};

fn file(path: &str, side: bool, calls: i64, cost: f64, unpriced: i64) -> ClientUsageFileGroup {
    ClientUsageFileGroup {
        source_file_id: path.into(),
        is_sidechain: side,
        calls,
        input_tokens: calls,
        output_tokens: 2 * calls,
        cache_read_tokens: 3 * calls,
        cache_write_tokens: 4 * calls,
        cost,
        unpriced_calls: unpriced,
    }
}

fn group(kind: &str, id: Option<&str>, ty: Option<&str>, calls: i64, cost: f64, unpriced: i64) -> ClientUsageAgentGroup {
    ClientUsageAgentGroup {
        kind: kind.into(),
        agent_id: id.map(Into::into),
        agent_type: ty.map(Into::into),
        calls,
        input_tokens: calls,
        output_tokens: 2 * calls,
        cache_read_tokens: 3 * calls,
        cache_write_tokens: 4 * calls,
        cost,
        unpriced_calls: unpriced,
    }
}

#[test]
fn claude_code_splits_subagents_by_file_and_reads_agent_type_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sub = dir.path().join("s1").join("subagents");
    std::fs::create_dir_all(&sub).expect("mkdir");
    std::fs::write(sub.join("agent-a1.meta.json"), r#"{"agentType":"Explore","description":"演示派发文本-机密A"}"#).expect("meta a");
    std::fs::write(sub.join("agent-b2.meta.json"), r#"{"agentType":"general-purpose","description":"演示派发文本-机密B"}"#).expect("meta b");
    std::fs::write(sub.join("agent-d4.meta.json"), "not json").expect("meta d");
    let p = |name: &str| sub.join(name).to_string_lossy().into_owned();
    let main = dir.path().join("s1.jsonl").to_string_lossy().into_owned();

    let out = group_agents(
        "claude_code",
        vec![
            file(&main, false, 3, 0.75, 0),
            file(&p("agent-a1.jsonl"), true, 2, 1.0, 1),
            file(&p("agent-b2.jsonl"), true, 1, 0.5, 0),
            file(&p("agent-c3.jsonl"), true, 1, 0.0, 1),
            file(&p("agent-d4.jsonl"), true, 1, 0.25, 0),
        ],
    );
    assert_eq!(
        out,
        vec![
            group("main", None, None, 3, 0.75, 0),
            group("subagent", Some("a1"), Some("Explore"), 2, 1.0, 1),
            group("subagent", Some("b2"), Some("general-purpose"), 1, 0.5, 0),
            group("subagent", Some("c3"), None, 1, 0.0, 1),
            group("subagent", Some("d4"), None, 1, 0.25, 0),
        ],
        "主线单列；子代理按文件拆分，agentType 作名称；缺 meta / 坏 meta 回落 None"
    );
    let json = serde_json::to_string(&out).expect("serialize");
    assert!(!json.contains("机密"), "description 不得进返回值: {json}");
}

#[test]
fn non_claude_clients_only_split_main_and_subagent() {
    for code in ["codex", "gemini_cli", "opencode"] {
        let out = group_agents(
            code,
            vec![
                file("/x/a.jsonl", false, 2, 0.5, 0),
                file("/x/s1/subagents/agent-z.jsonl", true, 1, 0.25, 1),
                file("/x/b.jsonl", true, 3, 1.0, 0),
                file("/x/c.jsonl", false, 1, 0.25, 1),
            ],
        );
        assert_eq!(
            out,
            vec![group("main", None, None, 3, 0.75, 1), group("subagent", None, None, 4, 1.25, 1)],
            "{code} 只返回主线 / 子代理两组"
        );
    }
}
