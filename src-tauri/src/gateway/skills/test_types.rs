use super::*;

#[test]
fn agent_slug_and_display() {
    // 关键修正：claude slug 必须 "claude-code"（旧值 "claude" 是错的）。
    assert_eq!(SkillAgent::Claude.cli_slug(), "claude-code");
    assert_eq!(SkillAgent::Codex.cli_slug(), "codex");
    assert_eq!(SkillAgent::Claude.display_name(), "Claude Code");
    assert_eq!(SkillAgent::Codex.display_name(), "Codex");
}

#[test]
fn cache_key_global_and_project_distinct() {
    assert_eq!(SkillScope::Global.cache_key(), "global");
    let p = SkillScope::Project {
        path: "/proj/a".to_string(),
    };
    assert_eq!(p.cache_key(), "project:/proj/a");
    // 不同项目 path 不串。
    let q = SkillScope::Project {
        path: "/proj/b".to_string(),
    };
    assert_ne!(p.cache_key(), q.cache_key());
    // global ≠ 任意 project。
    assert_ne!(SkillScope::Global.cache_key(), p.cache_key());
}

#[test]
fn cache_key_trims_project_path() {
    let p = SkillScope::Project {
        path: "  /proj/a  ".to_string(),
    };
    assert_eq!(p.cache_key(), "project:/proj/a");
}
