use super::*;
use crate::gateway::skills::types::{SkillAgent, SkillScope};
use std::fs;
use std::os::unix::fs::symlink;
use std::sync::Mutex;
use tempfile::TempDir;

/// 跨测试互斥锁：避免多个改 HOME/CLAUDE_CONFIG_DIR/CODEX_HOME 的测试并行时互相干扰
/// （set_var 是进程级，cargo 默认并行跑 test 会串扰）。
/// 测试内部持有 `let _g = ENV_LOCK.lock().unwrap();` 即可串行化所有 env-mutating 测试。
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Save/restore guard：测试期间临时改 HOME/CLAUDE_CONFIG_DIR/CODEX_HOME，
/// Drop 时还原（避免污染后续测试 / 主仓 env）。
struct EnvGuard {
    prev_home: Option<String>,
    prev_claude_cfg: Option<String>,
    prev_codex_home: Option<String>,
}
impl EnvGuard {
    fn new(home: &std::path::Path) -> Self {
        let prev_home = std::env::var("HOME").ok();
        let prev_claude_cfg = std::env::var("CLAUDE_CONFIG_DIR").ok();
        let prev_codex_home = std::env::var("CODEX_HOME").ok();
        unsafe {
            std::env::set_var("HOME", home);
            std::env::remove_var("CLAUDE_CONFIG_DIR");
            std::env::remove_var("CODEX_HOME");
        }
        Self {
            prev_home,
            prev_claude_cfg,
            prev_codex_home,
        }
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.prev_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match &self.prev_claude_cfg {
                Some(v) => std::env::set_var("CLAUDE_CONFIG_DIR", v),
                None => std::env::remove_var("CLAUDE_CONFIG_DIR"),
            }
            match &self.prev_codex_home {
                Some(v) => std::env::set_var("CODEX_HOME", v),
                None => std::env::remove_var("CODEX_HOME"),
            }
        }
    }
}

/// 构造合法锁文件 JSON 文本（含可选字段）。
fn lock_json(version: i64, skills_body: &str) -> String {
    format!(r#"{{"version":{version},"skills":{skills_body}}}"#)
}

#[test]
fn parse_lock_file_v3_ok() {
    let body = r#"{
        "foo":{"source":"o/r","sourceType":"github","sourceUrl":"https://github.com/o/r.git","skillPath":"skills/foo/SKILL.md","skillFolderHash":"abc","installedAt":"2026-06-01T00:00:00Z","updatedAt":"2026-06-02T00:00:00Z"},
        "bar":{"source":"  ","sourceType":"","pluginName":"plug"}
    }"#;
    let lf = parse_lock_file(&lock_json(3, body)).expect("v3 合法应解析成功");
    assert_eq!(lf.version, 3);
    assert_eq!(lf.skills.len(), 2);
    let foo = lf.skills.get("foo").unwrap();
    assert_eq!(foo.source.as_deref(), Some("o/r"));
    assert_eq!(foo.source_type.as_deref(), Some("github"));
    assert_eq!(foo.skill_folder_hash.as_deref(), Some("abc"));
    assert_eq!(foo.installed_at.as_deref(), Some("2026-06-01T00:00:00Z"));
    let bar = lf.skills.get("bar").unwrap();
    assert!(bar.source.as_deref().unwrap_or("").trim().is_empty()); // 空 source
    assert_eq!(bar.plugin_name.as_deref(), Some("plug"));
}

#[test]
fn parse_lock_file_bad_json_err() {
    assert!(parse_lock_file("not json {{{").is_err());
}

#[test]
fn parse_lock_file_wrong_version_err() {
    // version 2 应被守卫拒（仅支持 v3）。
    let body = r#"{"foo":{"source":"o/r"}}"#;
    assert!(parse_lock_file(&lock_json(2, body)).is_err());
}

#[test]
fn parse_lock_file_missing_version_err() {
    // 缺 version 字段 → 反序列化失败（无 default）。
    let body = r#"{"foo":{"source":"o/r"}}"#;
    assert!(parse_lock_file(&format!(r#"{{"skills":{body}}}"#)).is_err());
}

#[test]
fn build_skill_infos_emits_all_fields_and_filters_empty() {
    let body = r#"{
        "alpha":{"source":"o/r","sourceType":"github","sourceUrl":"https://github.com/o/r.git","skillFolderHash":"h1","pluginName":"plug","installedAt":"2026-01-01T00:00:00Z","updatedAt":"2026-02-01T00:00:00Z"},
        "beta":{"source":"  ","sourceType":""}
    }"#;
    let lf = parse_lock_file(&lock_json(3, body)).unwrap();
    let scope = SkillScope::Project {
        // tempdir 路径随便给（不真读 fs，build_skill_infos 只拼字符串路径）。
        path: "/tmp/nonexistent-test-project".to_string(),
    };
    let items = build_skill_infos(lf, &scope);
    assert_eq!(items.len(), 2);
    // name 按 sort 排（调用方排，这里顺序未定，逐个找）。
    let alpha = items.iter().find(|s| s.name == "alpha").unwrap();
    assert_eq!(alpha.source.as_deref(), Some("o/r"));
    assert_eq!(alpha.source_type.as_deref(), Some("github"));
    assert_eq!(alpha.source_url.as_deref(), Some("https://github.com/o/r.git"));
    assert_eq!(alpha.skill_folder_hash.as_deref(), Some("h1"));
    assert_eq!(alpha.plugin_name.as_deref(), Some("plug"));
    assert_eq!(alpha.installed_at.as_deref(), Some("2026-01-01T00:00:00Z"));
    assert_eq!(alpha.updated_at.as_deref(), Some("2026-02-01T00:00:00Z"));
    // project scope 未创建本地 symlink → enabled_agents 应为空。
    assert!(alpha.enabled_agents.is_empty());
    // installed_path 应含 .agents/skills/<name>。
    assert!(alpha
        .installed_path
        .as_deref()
        .unwrap()
        .ends_with("/.agents/skills/alpha"));

    // beta 空/whitespace source/sourceType → 这些字段过滤为 None。
    let beta = items.iter().find(|s| s.name == "beta").unwrap();
    assert!(beta.source.is_none());
    assert!(beta.source_type.is_none());
    assert!(beta.plugin_name.is_none());
    assert!(beta.installed_at.is_none());
}

#[test]
fn list_installed_reads_lockfile_and_detects_symlinks_global() {
    // 真 fs fixture：tempdir 作 HOME，~/.agents/.skill-lock.json + ~/.agents/skills/<n>/SKILL.md
    // + ~/.claude/skills/<n> symlink + ~/.codex/skills/<n> symlink。
    // 持 ENV_LOCK 串行化 + EnvGuard 还原，避免并行测试改 HOME 串扰。
    let _g = ENV_LOCK.lock().unwrap();
    let home = TempDir::new().unwrap();
    let _env = EnvGuard::new(home.path());
    let home_path = home.path();

    let agents_dir = home_path.join(".agents");
    let skills_dir = agents_dir.join("skills");
    fs::create_dir_all(&skills_dir).unwrap();
    // 规范存储 alpha + beta（带 SKILL.md frontmatter description）。
    for name in ["alpha", "beta"] {
        let d = skills_dir.join(name);
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Desc for {name}\n---\nbody\n"),
        )
        .unwrap();
    }
    // alpha 启用 claude + codex（建 symlink），beta 仅 claude。
    let claude_skills = home_path.join(".claude").join("skills");
    let codex_skills = home_path.join(".codex").join("skills");
    fs::create_dir_all(&claude_skills).unwrap();
    fs::create_dir_all(&codex_skills).unwrap();
    symlink(skills_dir.join("alpha"), claude_skills.join("alpha")).unwrap();
    symlink(skills_dir.join("alpha"), codex_skills.join("alpha")).unwrap();
    symlink(skills_dir.join("beta"), claude_skills.join("beta")).unwrap();
    // codex/beta 不建 → codex 未启用。

    // 锁文件：仅 alpha + beta 条目。
    let lock_body = r#"{
        "alpha":{"source":"o/r"},
        "beta":{"source":"p/q"}
    }"#;
    fs::write(
        agents_dir.join(".skill-lock.json"),
        lock_json(3, lock_body),
    )
    .unwrap();

    let (items, ok) = list_installed(&SkillScope::Global, None);
    assert!(ok);
    assert_eq!(items.len(), 2);
    let alpha = items.iter().find(|s| s.name == "alpha").unwrap();
    assert_eq!(
        alpha.enabled_agents,
        vec![SkillAgent::Claude, SkillAgent::Codex]
    );
    assert_eq!(alpha.source.as_deref(), Some("o/r"));
    assert_eq!(alpha.description.as_deref(), Some("Desc for alpha"));
    let beta = items.iter().find(|s| s.name == "beta").unwrap();
    assert_eq!(beta.enabled_agents, vec![SkillAgent::Claude]);
}

#[test]
fn list_installed_missing_lockfile_returns_empty_ok_global() {
    // 锁文件不存在 = 真空（非失败），返 ok=true + 空 vec。
    let _g = ENV_LOCK.lock().unwrap();
    let home = TempDir::new().unwrap();
    let _env = EnvGuard::new(home.path());
    let (items, ok) = list_installed(&SkillScope::Global, None);
    assert!(ok);
    assert!(items.is_empty());
}

#[test]
fn list_installed_bad_lockfile_returns_not_ok_global() {
    let _g = ENV_LOCK.lock().unwrap();
    let home = TempDir::new().unwrap();
    let _env = EnvGuard::new(home.path());
    let agents_dir = home.path().join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join(".skill-lock.json"), "not json {{{").unwrap();
    let (items, ok) = list_installed(&SkillScope::Global, None);
    assert!(!ok);
    assert!(items.is_empty());
}

#[test]
fn list_installed_wrong_version_returns_not_ok_global() {
    let _g = ENV_LOCK.lock().unwrap();
    let home = TempDir::new().unwrap();
    let _env = EnvGuard::new(home.path());
    let agents_dir = home.path().join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join(".skill-lock.json"),
        r#"{"version":99,"skills":{}}"#,
    )
    .unwrap();
    let (items, ok) = list_installed(&SkillScope::Global, None);
    assert!(!ok);
    assert!(items.is_empty());
}

#[test]
fn list_installed_project_scope_reads_project_lockfile() {
    // project scope 用 <project>/.agents/.skill-lock.json，不读 HOME（无 env 改动）。
    let project = TempDir::new().unwrap();
    let agents_dir = project.path().join(".agents");
    let skills_dir = agents_dir.join("skills");
    fs::create_dir_all(&skills_dir).unwrap();
    let foo = skills_dir.join("foo");
    fs::create_dir_all(&foo).unwrap();
    fs::write(foo.join("SKILL.md"), "---\nname: foo\n---\nbody\n").unwrap();
    fs::write(
        agents_dir.join(".skill-lock.json"),
        lock_json(3, r#"{"foo":{"source":"o/r"}}"#),
    )
    .unwrap();
    let scope = SkillScope::Project {
        path: project.path().to_string_lossy().into_owned(),
    };
    let (items, ok) = list_installed(&scope, None);
    assert!(ok);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "foo");
    assert_eq!(items[0].source.as_deref(), Some("o/r"));
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

#[test]
fn is_skill_enabled_for_agent_path_traversal_guard() {
    // 路径遍历输入被拒（不会去查 ~/.claude/skills/../etc）。
    let enabled = is_skill_enabled_for_agent("../etc", SkillAgent::Claude, &SkillScope::Global);
    assert!(!enabled);
    let enabled = is_skill_enabled_for_agent("a/b", SkillAgent::Claude, &SkillScope::Global);
    assert!(!enabled);
}

#[test]
fn agent_global_skills_dir_respects_env_override() {
    // CLAUDE_CONFIG_DIR / CODEX_HOME 优先。
    let _g = ENV_LOCK.lock().unwrap();
    let tmp = TempDir::new().unwrap();
    let _env_claude = scoped_var("CLAUDE_CONFIG_DIR", tmp.path().join("custom-claude"));
    let d = agent_global_skills_dir(SkillAgent::Claude).unwrap();
    assert!(d.ends_with("custom-claude/skills"));

    let _env_codex = scoped_var("CODEX_HOME", tmp.path().join("custom-codex"));
    let d = agent_global_skills_dir(SkillAgent::Codex).unwrap();
    assert!(d.ends_with("custom-codex/skills"));
}

/// 辅助：scoped env var（Drop 时还原），用于单测试内 env 切换。
struct ScopedVar {
    key: &'static str,
    prev: Option<String>,
}
fn scoped_var(key: &'static str, value: std::path::PathBuf) -> ScopedVar {
    let prev = std::env::var(key).ok();
    unsafe { std::env::set_var(key, value) };
    ScopedVar { key, prev }
}
impl Drop for ScopedVar {
    fn drop(&mut self) {
        unsafe {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

