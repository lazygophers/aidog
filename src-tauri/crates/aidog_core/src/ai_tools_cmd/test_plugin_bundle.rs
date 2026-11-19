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

// ─── Claude Code 原生插件只读取清单（HOME 隔离）────────────────

fn claude_installed(entries: &[(&str, &str, &str)], enabled: &[(&str, bool)]) {
    let home = dirs::home_dir().unwrap();
    let dir = home.join(".claude/plugins");
    std::fs::create_dir_all(&dir).unwrap();
    let mut map = serde_json::Map::new();
    for (id, path, version) in entries {
        map.insert(
            id.to_string(),
            json!([{ "scope": "user", "installPath": path, "version": version }]),
        );
    }
    std::fs::write(
        dir.join("installed_plugins.json"),
        json!({ "version": 2, "plugins": map }).to_string(),
    )
    .unwrap();
    let mut enabled_plugins = serde_json::Map::new();
    for (id, enabled) in enabled {
        enabled_plugins.insert(id.to_string(), json!(enabled));
    }
    std::fs::write(
        home.join(".claude/settings.json"),
        serde_json::to_string_pretty(
            &json!({ "model": "opus", "enabledPlugins": enabled_plugins }),
        )
        .unwrap(),
    )
    .unwrap();
}

fn fake_claude_cli(args: &[&str], _project_cwd: Option<&Path>) -> Result<String, String> {
    assert_eq!(args, ["plugin", "list", "--json"]);
    let home = dirs::home_dir().unwrap();
    let installed_path = home.join(".claude/plugins/installed_plugins.json");
    let installed: Value =
        serde_json::from_str(&std::fs::read_to_string(&installed_path).unwrap()).unwrap();
    let mut rows = Vec::new();
    for (id, records) in installed["plugins"].as_object().unwrap() {
        for record in records.as_array().unwrap() {
            let scope = record["scope"].as_str().unwrap();
            let project_path = record.get("projectPath").and_then(Value::as_str);
            let settings_path = match scope {
                "user" => home.join(".claude/settings.json"),
                "project" => Path::new(project_path.unwrap()).join(".claude/settings.json"),
                "local" => Path::new(project_path.unwrap()).join(".claude/settings.local.json"),
                _ => unreachable!(),
            };
            let settings = std::fs::read_to_string(settings_path)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .unwrap_or_else(|| json!({}));
            rows.push(json!({
                "id": id,
                "scope": scope,
                "installPath": record["installPath"],
                "projectPath": record.get("projectPath"),
                "enabled": settings["enabledPlugins"][id].as_bool().unwrap_or(false),
            }));
        }
    }
    Ok(serde_json::to_string(&rows).unwrap())
}

#[tokio::test]
async fn claude_installed_plugins_merge_into_list() {
    let _home = HomeGuard::new();
    let db = aidog_db::test_support::test_db().await;
    claude_installed(
        &[
            ("ponytail@ponytail", "/x/p", "5.3.3"),
            ("caveman@caveman", "/x/c", "4.1.0"),
        ],
        &[("ponytail@ponytail", true)],
    );
    let project = std::env::current_dir().unwrap();
    let list = list_impl_in_project(&db, &CliMode::Fake(fake_claude_cli), Some(&project))
        .await
        .unwrap();
    assert_eq!(
        list.plugins.len(),
        2,
        "both installed plugins listed: {:?}",
        list.plugins
    );
    let p = list
        .plugins
        .iter()
        .find(|p| p.name == "ponytail@ponytail")
        .unwrap();
    assert!(p.on);
    assert!(p.external);
    assert_eq!(p.scope.as_deref(), Some("user"));

    let src = tempfile::tempdir().unwrap();
    let root = src.path().join("bundle");
    fixture(&root, "demo");
    install_impl(
        &db,
        root.to_string_lossy().into_owned(),
        vec![AGENTS[0].into()],
        None,
    )
    .await
    .unwrap();
    let merged = list_impl_in_project(&db, &CliMode::Fake(fake_claude_cli), Some(&project))
        .await
        .unwrap();
    assert_eq!(merged.plugins.len(), 3);
    assert!(
        merged
            .plugins
            .iter()
            .any(|p| p.name == "demo" && !p.external)
    );
}

#[tokio::test]
async fn project_and_local_plugins_are_filtered_to_current_project() {
    let _home = HomeGuard::new();
    let home = dirs::home_dir().unwrap();
    let project = home.join("repo");
    std::fs::create_dir_all(home.join(".claude/plugins")).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    let project_path = project.to_string_lossy().into_owned();
    std::fs::write(
        home.join(".claude/plugins/installed_plugins.json"),
        json!({
            "version": 2,
            "plugins": {
                "user@m": [{"scope":"user","installPath":"/user"}],
                "project@m": [{"scope":"project","projectPath":project_path,"installPath":"/project"}],
                "local@m": [{"scope":"local","projectPath":project_path,"installPath":"/local"}],
                "other@m": [{"scope":"project","projectPath":"/other/project","installPath":"/other"}]
            }
        }).to_string(),
    ).unwrap();
    let db = aidog_db::test_support::test_db().await;
    let list = list_impl_in_project(&db, &CliMode::Fake(fake_claude_cli), Some(&project))
        .await
        .unwrap();
    for (id, scope) in [
        ("user@m", "user"),
        ("project@m", "project"),
        ("local@m", "local"),
    ] {
        assert!(
            list.plugins
                .iter()
                .any(|p| p.name == id && p.scope.as_deref() == Some(scope)),
            "missing {scope} {id}"
        );
    }
    assert!(
        !list.plugins.iter().any(|p| p.name == "other@m"),
        "other project records are excluded"
    );
}

#[tokio::test]
async fn external_settings_changes_are_read_on_next_list() {
    let _home = HomeGuard::new();
    let home = dirs::home_dir().unwrap();
    let db = aidog_db::test_support::test_db().await;
    claude_installed(&[("a@m", "/x/a", "1.0")], &[("a@m", false)]);
    let project = std::env::current_dir().unwrap();
    let before = list_impl_in_project(&db, &CliMode::Fake(fake_claude_cli), Some(&project))
        .await
        .unwrap();
    assert!(!before.plugins.iter().find(|p| p.name == "a@m").unwrap().on);

    let path = home.join(".claude/settings.json");
    std::fs::write(&path, json!({"enabledPlugins":{"a@m":true}}).to_string()).unwrap();
    let after = list_impl_in_project(&db, &CliMode::Fake(fake_claude_cli), Some(&project))
        .await
        .unwrap();
    assert!(after.plugins.iter().find(|p| p.name == "a@m").unwrap().on);
}

static CLI_CALLS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn record(args: &[&str], project_cwd: Option<&Path>) {
    CLI_CALLS.lock().unwrap().push(format!(
        "{} [cwd {}]",
        args.join(" "),
        project_cwd.map(|p| p.display().to_string()).unwrap_or_default()
    ));
}

fn fake_cli_recorder(args: &[&str], project_cwd: Option<&Path>) -> Result<String, String> {
    record(args, project_cwd);
    if args[1] == "list" {
        fake_claude_cli(args, project_cwd)
    } else {
        Ok("ok".into())
    }
}

fn fake_cli_failing(args: &[&str], project_cwd: Option<&Path>) -> Result<String, String> {
    record(args, project_cwd);
    if args[1] == "list" {
        fake_claude_cli(args, project_cwd)
    } else {
        Err("boom: plugin not found".into())
    }
}

#[tokio::test]
async fn native_plugin_enable_update_run_cli_with_scope() {
    let _home = HomeGuard::new();
    let home = dirs::home_dir().unwrap();
    let project = home.join("repo");
    std::fs::create_dir_all(&project).unwrap();
    claude_installed(&[("a@m", "/a", "1.0")], &[("a@m", false)]);
    let db = aidog_db::test_support::test_db().await;
    CLI_CALLS.lock().unwrap().clear();

    native_set_impl(&db, &CliMode::Fake(fake_cli_recorder), "a@m", true, Some(&project))
        .await
        .unwrap();
    native_update_impl(&db, &CliMode::Fake(fake_cli_recorder), "a@m", Some(&project))
        .await
        .unwrap();
    let calls = CLI_CALLS.lock().unwrap().clone();
    assert!(
        calls.iter().any(|c| c.contains("plugin enable a@m --scope user")),
        "calls: {calls:?}"
    );
    assert!(
        calls.iter().any(|c| c.contains("plugin update a@m --scope user")),
        "calls: {calls:?}"
    );

    let err = native_set_impl(&db, &CliMode::Fake(fake_cli_recorder), "gone@m", true, Some(&project))
        .await
        .unwrap_err();
    assert!(err.contains("no plugin called gone@m"), "err: {err}");
}

#[tokio::test]
async fn native_plugin_disable_project_scope_runs_cli_in_project() {
    let _home = HomeGuard::new();
    let home = dirs::home_dir().unwrap();
    let project = home.join("repo");
    std::fs::create_dir_all(project.join(".claude")).unwrap();
    std::fs::create_dir_all(home.join(".claude/plugins")).unwrap();
    std::fs::write(
        home.join(".claude/plugins/installed_plugins.json"),
        json!({"version":2,"plugins":{"p@m":[{"scope":"project","projectPath":project.to_string_lossy(),"installPath":"/p","version":"1"}]}}).to_string(),
    ).unwrap();
    std::fs::write(
        project.join(".claude/settings.json"),
        json!({"enabledPlugins":{"p@m":true}}).to_string(),
    ).unwrap();
    let db = aidog_db::test_support::test_db().await;
    CLI_CALLS.lock().unwrap().clear();

    let list = native_set_impl(&db, &CliMode::Fake(fake_cli_recorder), "p@m", false, Some(&project))
        .await
        .unwrap();
    let calls = CLI_CALLS.lock().unwrap().clone();
    assert!(
        calls.iter().any(|c| {
            c.contains("plugin disable p@m --scope project")
                && c.contains(&format!("[cwd {}", project.display()))
        }),
        "calls: {calls:?}"
    );
    // Fake CLI does not mutate settings, so the refreshed row still reads enabled=true.
    assert!(list.plugins.iter().find(|p| p.name == "p@m").unwrap().on);
}

#[tokio::test]
async fn native_plugin_cli_failure_is_surfaced() {
    let _home = HomeGuard::new();
    claude_installed(&[("a@m", "/a", "1.0")], &[("a@m", false)]);
    let db = aidog_db::test_support::test_db().await;
    let err = native_set_impl(&db, &CliMode::Fake(fake_cli_failing), "a@m", true, None)
        .await
        .unwrap_err();
    assert!(err.contains("boom: plugin not found"), "err: {err}");
}
