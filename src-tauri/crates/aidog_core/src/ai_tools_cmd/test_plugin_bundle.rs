//! Plugin bundle tests use a temporary HOME and local folder/git fixtures only.
use super::*;
use aidog_db::test_support::HomeGuard;
use serde_json::json;
use std::path::Path;

fn fixture(root: &Path, name: &str) {
    std::fs::create_dir_all(root.join(".claude-plugin")).unwrap();
    std::fs::write(
        root.join(".claude-plugin/plugin.json"),
        format!(r#"{{"name":"{name}","description":"test","version":"1"}}"#),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("skills/review")).unwrap();
    std::fs::write(
        root.join("skills/review/SKILL.md"),
        "---\nname: review\ndescription: Review\n---\n\n${CLAUDE_PLUGIN_ROOT}\n",
    )
    .unwrap();
    std::fs::write(root.join("skills/review/extra.md"), "owned file").unwrap();
    std::fs::create_dir_all(root.join("agents")).unwrap();
    std::fs::write(
        root.join("agents/helper.md"),
        "---\nname: helper\ndescription: Helper\n---\n\nUse ${CLAUDE_PLUGIN_DATA}.",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("hooks")).unwrap();
    // Official plugin shape: hooks/hooks.json wraps events in a top-level "hooks" object.
    std::fs::write(root.join("hooks/hooks.json"), r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"node ${CLAUDE_PLUGIN_ROOT}/hooks.js"}]}]}}"#).unwrap();
    std::fs::write(root.join(".mcp.json"), r#"{"mcpServers":{"docs":{"type":"stdio","command":"node","args":["${CLAUDE_PLUGIN_ROOT}/mcp.js"]}}}"#).unwrap();
    std::fs::create_dir_all(root.join("commands")).unwrap();
    std::fs::write(root.join("commands/ignored.md"), "slash command").unwrap();
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[tokio::test]
async fn folder_install_disable_enable_and_remove_tracks_only_own_files() {
    let home = HomeGuard::new();
    let db = aidog_db::test_support::test_db().await;
    let src = tempfile::tempdir().unwrap();
    let root = src.path().join("bundle");
    fixture(&root, "demo");

    let result = install_impl(
        &db,
        root.to_string_lossy().into_owned(),
        vec![AGENTS[0].into()],
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.plugins.len(), 1);
    let p = &result.plugins[0];
    assert_eq!(p.name, "demo");
    assert_eq!(p.skills, ["review"]);
    assert_eq!(p.subagents, ["helper"]);
    assert_eq!(p.servers, ["docs"]);
    assert!(
        p.left.iter().any(|x| x.code == "commands"),
        "left: {:?}",
        p.left
    );
    assert!(
        home.home()
            .join(".aidog/plugins/demo/.claude-plugin/plugin.json")
            .exists()
    );
    assert!(home.home().join(".claude/skills/review/SKILL.md").exists());
    assert!(home.home().join(".claude/agents/helper.md").exists());
    let hook_cmd = format!(
        "node {}/hooks.js",
        home.home().join(".aidog/plugins/demo").display()
    );
    let cfg = context_compress::load_claude_cfg_pub(&db).await;
    assert!(
        context_compress::has_hook(&cfg, "PreToolUse", &hook_cmd),
        "resolved hook in Claude global config: {cfg}"
    );
    assert!(
        !home.home().join(".codex/hooks.json").exists(),
        "codex not selected: no codex hook written"
    );

    // A change to any copied skill file transfers ownership to the user.
    let user_file = home.home().join(".claude/skills/review/extra.md");
    std::fs::write(&user_file, "mine").unwrap();
    let mut p = load(&db).await.remove(0);
    p.off = true;
    let mut warnings = Vec::new();
    sync_parts(&db, &mut p, &mut warnings).await.unwrap();
    save(&db, &[p.clone()]).await.unwrap();
    assert!(user_file.exists());
    assert!(
        warnings
            .iter()
            .any(|w| w.code == "skill_modified" && w.args["name"] == "review"),
        "warnings: {warnings:?}"
    );

    p.off = false;
    sync_parts(&db, &mut p, &mut warnings).await.unwrap();
    assert_eq!(std::fs::read_to_string(&user_file).unwrap(), "mine");

    p.off = true;
    sync_parts(&db, &mut p, &mut warnings).await.unwrap();
    remove_parts_db(&db, &p).await.unwrap();
    assert_eq!(std::fs::read_to_string(&user_file).unwrap(), "mine");
    assert!(
        gateway::mcp::store::get_mcp_server(&db, "docs")
            .await
            .unwrap()
            .is_none()
    );
    assert!(!home.home().join(".claude/agents/helper.md").exists());
    let cfg = context_compress::load_claude_cfg_pub(&db).await;
    assert!(
        !context_compress::has_hook(&cfg, "PreToolUse", &hook_cmd),
        "own hook retracted"
    );
}

#[tokio::test]
async fn name_conflicts_get_plugin_prefix_and_user_items_survive_remove() {
    let home = HomeGuard::new();
    let db = aidog_db::test_support::test_db().await;
    let user_skill = home.home().join(".claude/skills/review");
    std::fs::create_dir_all(&user_skill).unwrap();
    std::fs::write(user_skill.join("SKILL.md"), "user skill").unwrap();
    let src = tempfile::tempdir().unwrap();
    let root = src.path().join("bundle");
    fixture(&root, "demo");
    // Bare-map .mcp.json with url only: transport inferred as http.
    std::fs::write(
        root.join(".mcp.json"),
        r#"{"docs":{"url":"http://127.0.0.1:1/mcp"}}"#,
    )
    .unwrap();

    let result = install_impl(
        &db,
        root.to_string_lossy().into_owned(),
        vec![AGENTS[0].into()],
        None,
    )
    .await;
    let result = match result {
        Ok(r) => r,
        Err(e) => panic!("install must accept bare-map .mcp.json: {e}"),
    };
    assert_eq!(result.plugins[0].skills, ["demo-review"]);
    let server = gateway::mcp::store::get_mcp_server(&db, "docs")
        .await
        .unwrap()
        .expect("server stored");
    assert_eq!(server.transport, "http");
    assert!(
        home.home()
            .join(".claude/skills/demo-review/extra.md")
            .exists()
    );

    let left = plugins_remove_impl(&db, "demo").await.unwrap();
    assert!(left.plugins.is_empty());
    assert!(!home.home().join(".claude/skills/demo-review").exists());
    assert!(!home.home().join(".aidog/plugins/demo").exists());
    assert_eq!(
        std::fs::read_to_string(user_skill.join("SKILL.md")).unwrap(),
        "user skill"
    );
}

#[tokio::test]
async fn update_keeps_user_modified_skill_and_reports_it() {
    let home = HomeGuard::new();
    let db = aidog_db::test_support::test_db().await;
    let src = tempfile::tempdir().unwrap();
    let root = src.path().join("bundle");
    fixture(&root, "upd");
    let source = root.to_string_lossy().into_owned();
    install_impl(&db, source.clone(), vec![AGENTS[0].into()], None)
        .await
        .unwrap();
    let user_file = home.home().join(".claude/skills/review/extra.md");
    std::fs::write(&user_file, "mine").unwrap();

    let result = install_impl(&db, source, Vec::new(), None).await.unwrap();
    assert_eq!(std::fs::read_to_string(&user_file).unwrap(), "mine");
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.code == "skill_modified" && w.args["agent"] == AGENTS[0]),
        "warnings: {:?}",
        result.warnings
    );
}

#[tokio::test]
async fn update_replaces_unmodified_parts_and_keeps_agent_selection() {
    let home = HomeGuard::new();
    let db = aidog_db::test_support::test_db().await;
    let src = tempfile::tempdir().unwrap();
    let root = src.path().join("bundle");
    fixture(&root, "update-demo");
    let source = root.to_string_lossy().into_owned();

    install_impl(&db, source.clone(), vec![AGENTS[0].into()], None)
        .await
        .unwrap();
    let skill_md = root.join("skills/review/SKILL.md");
    std::fs::write(
        &skill_md,
        "---\nname: review\ndescription: Updated\n---\n\nUpdated skill",
    )
    .unwrap();
    let result = install_impl(&db, source, Vec::new(), None).await.unwrap();
    let updated = result
        .plugins
        .iter()
        .find(|p| p.name == "update-demo")
        .unwrap();
    assert_eq!(updated.agents, [AGENTS[0]]);
    let copied = home
        .home()
        .join(".aidog/plugins/update-demo/skills/review/SKILL.md");
    assert!(
        std::fs::read_to_string(&copied)
            .unwrap()
            .contains("Updated skill")
    );
    let installed = home.home().join(".claude/skills/review/SKILL.md");
    assert!(
        std::fs::read_to_string(&installed)
            .unwrap()
            .contains("Updated skill")
    );
}

#[tokio::test]
async fn marketplace_file_and_local_git_sources_are_supported() {
    let _home = HomeGuard::new();
    let db = aidog_db::test_support::test_db().await;
    let root = tempfile::tempdir().unwrap();
    let plugins_dir = root.path().join("plugins/demo");
    fixture(&plugins_dir, "demo");
    std::fs::create_dir_all(root.path().join(".claude-plugin")).unwrap();
    std::fs::write(
        root.path().join(".claude-plugin/marketplace.json"),
        r#"{"plugins":[{"name":"demo","source":"plugins/demo"}]}"#,
    )
    .unwrap();
    let c = candidates(root.path(), &[]).unwrap();
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].rel, "plugins/demo");

    git(root.path(), &["init", "-q", "-b", "main"]);
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-q", "-m", "fixture"]);
    let git_url = format!("file://{}", root.path().display());
    let (cloned, temp) = stage(&git_url).await.unwrap();
    assert!(temp.is_some());
    assert!(cloned.join(".claude-plugin/marketplace.json").exists());
    let records = load(&db).await;
    assert!(candidates(&cloned, &records).is_ok());

    // Marketplace entries can point at a separate git repository and subfolder.
    let market = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(market.path().join(".claude-plugin")).unwrap();
    std::fs::write(
        market.path().join(".claude-plugin/marketplace.json"),
        format!(r#"{{"plugins":[{{"name":"demo","source":{{"source":"git","repo":"{}","path":"plugins/demo"}}}}]}}"#, git_url),
    )
    .unwrap();
    let result = install_impl(
        &db,
        market.path().to_string_lossy().into_owned(),
        vec![AGENTS[0].into()],
        None,
    )
    .await
    .unwrap();
    assert!(result.plugins.iter().any(|p| p.name == "demo"));
}

#[test]
fn library_delete_refuses_paths_outside_plugins_folder() {
    let home = HomeGuard::new();
    std::fs::create_dir_all(home.home().join(".aidog/plugins")).unwrap();
    std::fs::write(home.home().join(".aidog/keep.txt"), "x").unwrap();
    assert!(remove_library_dir("..").is_err());
    assert!(home.home().join(".aidog/keep.txt").exists());
}

#[test]
fn source_variable_spellings_resolve() {
    let root = Path::new("/library/plugins/demo");
    let data = Path::new("/library/plugin-data/demo");
    assert_eq!(
        resolve("${CLAUDE_PLUGIN_ROOT}/x", root, data),
        "/library/plugins/demo/x"
    );
    assert_eq!(
        resolve("%CLAUDE_PLUGIN_DATA%/x", root, data),
        "/library/plugin-data/demo/x"
    );
    assert!(
        resolve_hook("node -e process.env.CLAUDE_PLUGIN_ROOT", root, data)
            .contains("'/library/plugins/demo'")
    );
}

#[test]
fn notes_are_codes_and_legacy_strings_still_load() {
    let p: Plugin = serde_json::from_value(json!({
        "name": "old", "source": "/x", "pick": null, "agents": [],
        "left": ["commands: slash commands are not carried", {"code": "lsp", "args": {}}]
    }))
    .unwrap();
    assert_eq!(p.left[0].code, "text");
    assert_eq!(
        p.left[0].args["text"],
        "commands: slash commands are not carried"
    );
    assert_eq!(p.left[1], note("lsp", &[]));
    assert_eq!(
        serde_json::to_value(note("hook_type", &[("event", "Stop")])).unwrap(),
        json!({"code": "hook_type", "args": {"event": "Stop"}})
    );
    let root = tempfile::tempdir().unwrap();
    fixture(root.path(), "demo");
    assert_eq!(
        unsupported_parts(root.path()).unwrap(),
        [note("commands", &[])]
    );
}

#[tokio::test]
async fn codex_gets_mapped_hooks_and_claude_only_events_are_left_out() {
    let home = HomeGuard::new();
    let db = aidog_db::test_support::test_db().await;
    let src = tempfile::tempdir().unwrap();
    let root = src.path().join("bundle");
    fixture(&root, "cx");
    std::fs::write(
        root.join("hooks/hooks.json"),
        r#"{"hooks":{
            "PreToolUse":[{"matcher":"Edit|Write","hooks":[{"type":"command","command":"fmt"}]}],
            "PostToolUse":[{"matcher":"Read","hooks":[{"type":"command","command":"read-only"}]}],
            "SessionStart":[{"hooks":[{"type":"command","command":"hello"}]}],
            "Notification":[{"hooks":[{"type":"command","command":"ping"}]}]
        }}"#,
    )
    .unwrap();
    let result = install_impl(
        &db,
        root.to_string_lossy().into_owned(),
        vec![subagents::AGENT_CODEX.into()],
        None,
    )
    .await
    .unwrap();
    let path = home.home().join(".codex/hooks.json");
    let codex: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let hooks = &codex["hooks"];
    assert_eq!(hooks["PreToolUse"][0]["matcher"], "apply_patch", "{codex}");
    assert_eq!(hooks["SessionStart"][0]["hooks"][0]["command"], "hello");
    assert!(hooks.get("Notification").is_none(), "{codex}");
    assert!(hooks.get("PostToolUse").is_none(), "{codex}");
    let left = &result.plugins[0].left;
    for event in ["Notification", "PostToolUse"] {
        assert!(
            left.contains(&note("hook_claude_only", &[("event", event)])),
            "left: {left:?}"
        );
    }

    plugins_remove_impl(&db, "cx").await.unwrap();
    let codex: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(
        codex["hooks"].as_object().is_none_or(|m| m.is_empty()),
        "own codex hooks retracted: {codex}"
    );
}

#[tokio::test]
async fn failed_update_undoes_its_own_writes_and_a_retry_finishes() {
    let home = HomeGuard::new();
    let db = aidog_db::test_support::test_db().await;
    let src = tempfile::tempdir().unwrap();
    let root = src.path().join("bundle");
    fixture(&root, "rb");
    let source = root.to_string_lossy().into_owned();
    install_impl(&db, source.clone(), vec![AGENTS[0].into()], None)
        .await
        .unwrap();
    let user_file = home.home().join(".claude/skills/review/extra.md");
    std::fs::write(&user_file, "mine").unwrap();

    // The new version adds a skill whose target is blocked, so distribution fails halfway.
    std::fs::create_dir_all(root.join("skills/zz")).unwrap();
    std::fs::write(root.join("skills/zz/SKILL.md"), "---\nname: zz\n---\n").unwrap();
    let blocker = home.home().join(".claude/skills/zz");
    std::os::unix::fs::symlink(home.home().join("missing"), &blocker).unwrap();
    assert!(
        install_impl(&db, source.clone(), Vec::new(), None)
            .await
            .is_err()
    );

    // User-edited skill stays; this attempt's own writes are gone; the record stays for a retry.
    assert_eq!(std::fs::read_to_string(&user_file).unwrap(), "mine");
    assert!(!home.home().join(".claude/agents/helper.md").exists());
    assert!(
        gateway::mcp::store::get_mcp_server(&db, "docs")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !home.home().join(".claude/skills/rb-review").exists(),
        "no orphan copy of the new skill"
    );
    let rows = load(&db).await;
    assert_eq!(rows.len(), 1, "record kept for retry");
    assert!(rows[0].servers.is_empty() && rows[0].subagents.is_empty());
    assert!(
        !home.home().join(".aidog/plugins/rb/skills/zz").exists(),
        "previous library folder restored"
    );

    std::fs::remove_file(&blocker).unwrap();
    let done = install_impl(&db, source, Vec::new(), None).await.unwrap();
    let p = &done.plugins[0];
    assert!(p.on);
    assert_eq!(p.servers, ["docs"]);
    assert_eq!(p.subagents, ["helper"]);
    assert!(home.home().join(".claude/skills/zz/SKILL.md").exists());
    assert!(home.home().join(".claude/agents/helper.md").exists());
    assert_eq!(std::fs::read_to_string(&user_file).unwrap(), "mine");
}
