use super::*;
use crate::gateway::skills::types::SkillScope;

#[test]
fn parse_skill_sources_json_handles_cases() {
    // 正常：foo 有 source，bar 空 source 不入，baz 无 source 字段不入。
    let m = parse_skill_sources_json(
        r#"{"version":1,"skills":{"foo":{"source":"owner/repo","sourceType":"github"},"bar":{"source":"   "},"baz":{"sourceType":"github"}}}"#,
    );
    assert_eq!(m.len(), 1);
    assert_eq!(m.get("foo").map(String::as_str), Some("owner/repo"));

    // 损坏 JSON → 空。
    assert!(parse_skill_sources_json("not json {{{").is_empty());
    // 无 skills 对象 → 空。
    assert!(parse_skill_sources_json(r#"{"version":1}"#).is_empty());
    // 多条合法 source。
    let m2 = parse_skill_sources_json(r#"{"skills":{"a":{"source":"x/y"},"b":{"source":"p/q"}}}"#);
    assert_eq!(m2.len(), 2);
    assert_eq!(m2.get("a").map(String::as_str), Some("x/y"));
    assert_eq!(m2.get("b").map(String::as_str), Some("p/q"));
}

#[test]
fn parse_list_json_maps_enabled_agents() {
    let stdout = r#"[
        {"name":"alpha","path":"/p/alpha","scope":"global","agents":["Claude Code","Codex","Zed"]},
        {"name":"beta","path":"/p/beta","scope":"global","agents":["Codex"]},
        {"name":"gamma","path":"/p/gamma","scope":"global","agents":["Gemini CLI"]}
    ]"#;
    let out = parse_list_json(stdout, &SkillScope::Global);
    assert_eq!(out.len(), 3);
    // 排序后 alpha/beta/gamma。
    assert_eq!(out[0].name, "alpha");
    assert_eq!(
        out[0].enabled_agents,
        vec![SkillAgent::Claude, SkillAgent::Codex]
    );
    assert_eq!(out[0].installed_path.as_deref(), Some("/p/alpha"));
    assert_eq!(out[1].enabled_agents, vec![SkillAgent::Codex]);
    // gamma 无 claude/codex → 空。
    assert!(out[2].enabled_agents.is_empty());
}

#[test]
fn parse_list_json_bad_json_is_empty() {
    assert!(parse_list_json("not json", &SkillScope::Global).is_empty());
}

#[test]
fn parse_list_json_wrapped_object() {
    let stdout = r#"{"skills":[{"name":"x","agents":["Claude Code"]}]}"#;
    let out = parse_list_json(stdout, &SkillScope::Global);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].enabled_agents, vec![SkillAgent::Claude]);
}

#[test]
fn frontmatter_description_plain() {
    let md = "---\nname: foo\ndescription: A great skill for stuff.\n---\nbody\n";
    assert_eq!(
        parse_skill_description_from_frontmatter(md).as_deref(),
        Some("A great skill for stuff.")
    );
}

#[test]
fn frontmatter_description_quoted() {
    let md = "---\nname: foo\ndescription: \"Quoted desc\"\n---\n";
    assert_eq!(
        parse_skill_description_from_frontmatter(md).as_deref(),
        Some("Quoted desc")
    );
}

#[test]
fn frontmatter_description_single_quoted() {
    let md = "---\ndescription: 'single'\n---\n";
    assert_eq!(
        parse_skill_description_from_frontmatter(md).as_deref(),
        Some("single")
    );
}

#[test]
fn frontmatter_no_frontmatter() {
    let md = "just plain markdown\nno frontmatter\n";
    assert!(parse_skill_description_from_frontmatter(md).is_none());
}

#[test]
fn frontmatter_no_description_field() {
    let md = "---\nname: foo\n---\nbody\n";
    assert!(parse_skill_description_from_frontmatter(md).is_none());
}

#[test]
fn frontmatter_empty_description() {
    let md = "---\ndescription: \"\"\n---\n";
    assert!(parse_skill_description_from_frontmatter(md).is_none());
}

#[test]
fn frontmatter_desc_only_inside_frontmatter() {
    // description 行在正文 (非 frontmatter) 不应被解析。
    let md = "---\nname: foo\n---\ndescription: fake in body\n";
    assert!(parse_skill_description_from_frontmatter(md).is_none());
}
