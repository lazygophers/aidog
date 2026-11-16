//! 子代理单元测试（随 subagents.rs 1:1）。全部落 tempdir，不碰真实 ~/.claude、~/.codex。
use super::*;

fn dirs_in(tmp: &Path) -> Dirs {
    Dirs {
        claude: tmp.join("claude"),
        codex: tmp.join("codex"),
    }
}

fn sub(name: &str, agents: &[&str]) -> Subagent {
    Subagent {
        name: name.into(),
        description: "Reviews code".into(),
        prompt: "You review code.\n".into(),
        agents: agents.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

#[test]
fn resolve_uses_claude_config_dir_and_codex_home() {
    let g = aidog_db::test_support::HomeGuard::new();
    let d = Dirs::resolve().unwrap();
    assert_eq!(d.claude, g.home().join(".claude"));
    assert_eq!(d.codex, g.home().join(".codex"));
}

#[test]
fn writes_claude_markdown_and_codex_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let d = dirs_in(tmp.path());
    let mut s = sub("reviewer", &[AGENT_CLAUDE, AGENT_CODEX]);
    s.codex_model = Some("gpt-5".into());
    assert!(sync_entry(&mut s, &d).is_empty());

    let md = std::fs::read_to_string(d.claude.join("agents/reviewer.md")).unwrap();
    assert_eq!(
        md,
        "---\nname: reviewer\ndescription: \"Reviews code\"\n---\n\nYou review code.\n"
    );
    let toml_text = std::fs::read_to_string(d.codex.join("agents/reviewer.toml")).unwrap();
    let v: toml::Table = toml::from_str(&toml_text).unwrap();
    assert_eq!(v["name"].as_str(), Some("reviewer"));
    assert_eq!(v["description"].as_str(), Some("Reviews code"));
    assert_eq!(v["model"].as_str(), Some("gpt-5"));
    assert_eq!(v["developer_instructions"].as_str(), Some("You review code.\n"));
    assert_eq!(s.written.len(), 2);
    assert_eq!(agent_status(&s, AGENT_CLAUDE, &d), "on");
    assert_eq!(agent_status(&s, AGENT_CODEX, &d), "on");

    // 无 model → 不写 model 键
    let mut s2 = sub("plain", &[AGENT_CODEX]);
    sync_entry(&mut s2, &d);
    let v: toml::Table =
        toml::from_str(&std::fs::read_to_string(d.codex.join("agents/plain.toml")).unwrap()).unwrap();
    assert!(!v.contains_key("model"));
}

#[test]
fn disable_deletes_only_own_unmodified_file() {
    let tmp = tempfile::tempdir().unwrap();
    let d = dirs_in(tmp.path());
    let mut s = sub("reviewer", &[AGENT_CLAUDE, AGENT_CODEX]);
    sync_entry(&mut s, &d);
    // 用户改了 codex 文件
    let codex_path = d.codex.join("agents/reviewer.toml");
    std::fs::write(&codex_path, "name = \"reviewer\"\n# mine\n").unwrap();

    s.agents.clear();
    sync_entry(&mut s, &d);
    assert!(!d.claude.join("agents/reviewer.md").exists(), "own file removed");
    assert!(!s.written.contains_key(AGENT_CLAUDE));
    assert!(codex_path.exists(), "user-modified file kept");
    assert_eq!(agent_status(&s, AGENT_CODEX, &d), "modified");
    assert_eq!(agent_status(&s, AGENT_CLAUDE, &d), "off");
}

#[test]
fn foreign_file_with_same_name_not_overwritten() {
    let tmp = tempfile::tempdir().unwrap();
    let d = dirs_in(tmp.path());
    let p = d.claude.join("agents/reviewer.md");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, "theirs").unwrap();

    let mut s = sub("reviewer", &[AGENT_CLAUDE]);
    sync_entry(&mut s, &d);
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "theirs");
    assert!(s.written.is_empty());
    assert_eq!(agent_status(&s, AGENT_CLAUDE, &d), "conflict");

    // 关闭也不删外来文件
    s.agents.clear();
    sync_entry(&mut s, &d);
    assert!(p.exists());
}

fn write(p: &Path, s: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, s).unwrap();
}

fn agent_md(name: &str, desc: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {desc}\ncolor: red\n---\n{body}")
}

#[test]
fn import_have_update_edited() {
    let tmp = tempfile::tempdir().unwrap();
    let d = dirs_in(tmp.path());
    let src = tmp.path().join("repo");
    write(&src.join("agents/alpha.md"), &agent_md("Alpha Agent", "first", "A body"));
    write(&src.join("beta.md"), &agent_md("beta", "second", "B body"));
    write(&src.join("README.md"), "# no front matter");
    write(&src.join("noDesc.md"), "---\nname: x\n---\nbody");
    write(&src.join("skill/SKILL.md"), &agent_md("skill", "s", "s"));
    write(&src.join("skill/extra.md"), &agent_md("inskill", "s", "s"));
    write(&src.join("node_modules/m.md"), &agent_md("nm", "s", "s"));
    let source = src.to_string_lossy().to_string();

    let mut list = Vec::new();
    let r = import_into(&mut list, &src, &source, &[AGENT_CLAUDE.into()], &d);
    let mut added = r.added.clone();
    added.sort();
    assert_eq!(added, vec!["alpha-agent", "beta"]);
    assert!(d.claude.join("agents/alpha-agent.md").exists(), "ticked agent written");
    let alpha = list.iter().find(|s| s.name == "alpha-agent").unwrap();
    assert_eq!(alpha.description, "first");
    assert_eq!(alpha.prompt, "A body");

    // 原样再导入 → have
    let r = import_into(&mut list, &src, &source, &[], &d);
    assert_eq!(r.have.len(), 2);
    assert!(r.added.is_empty());

    // 上游改了 alpha、本地未改 → update；本地改了 beta、上游也改 → edited（不动）
    write(&src.join("agents/alpha.md"), &agent_md("Alpha Agent", "first v2", "A2"));
    list.iter_mut().find(|s| s.name == "beta").unwrap().prompt = "mine".into();
    write(&src.join("beta.md"), &agent_md("beta", "second v2", "B2"));
    let r = import_into(&mut list, &src, &source, &[], &d);
    assert_eq!(r.updated, vec!["alpha-agent"]);
    assert_eq!(r.edited, vec!["beta"]);
    let alpha = list.iter().find(|s| s.name == "alpha-agent").unwrap();
    assert_eq!((alpha.description.as_str(), alpha.prompt.as_str()), ("first v2", "A2"));
    assert_eq!(
        std::fs::read_to_string(d.claude.join("agents/alpha-agent.md")).unwrap(),
        "---\nname: alpha-agent\ndescription: \"first v2\"\n---\n\nA2",
        "update rewrites own file"
    );
    assert_eq!(list.iter().find(|s| s.name == "beta").unwrap().prompt, "mine");
    assert_eq!(list.len(), 2);
}

#[test]
fn import_name_clash_takes_next_free_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    let d = dirs_in(tmp.path());
    let src = tmp.path().join("repo");
    write(&src.join("reviewer.md"), &agent_md("reviewer", "d", "p"));
    let mut list = vec![sub("reviewer", &[])];
    let r = import_into(&mut list, &src, "src-a", &[], &d);
    assert_eq!(r.added, vec!["reviewer-2"]);
    let r = import_into(&mut list, &src, "src-b", &[], &d);
    assert_eq!(r.added, vec!["reviewer-3"]);
    assert_eq!(list[0], sub("reviewer", &[]), "existing entry untouched");
}
