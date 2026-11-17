//! 上下文压缩单元测试（随 context_compress.rs 1:1）。全部落 tempdir，不碰真实
//! ~/.claude、~/.claude.json、~/.codex，不调用真实 rtk / sqz。
use super::*;
use serde_json::json;

fn paths_in(tmp: &Path) -> Paths {
    let codex = tmp.join("codex");
    std::fs::create_dir_all(&codex).unwrap();
    Paths {
        claude_json: tmp.join("claude.json"),
        codex,
    }
}

fn claude_fixture() -> Value {
    json!({
        "userID": "u",
        "mcpServers": {
            "rea": {"type": "stdio", "command": "mise", "args": ["exec", "node@22", "—-", "npx", "-y", "rea-agents", "mcp"]},
            "gitnexus": {"command": "mise", "args": ["exec", "node@24", "—", "npx", "-y", "gitnexus"], "env": {"K": "v"}},
            "web": {"type": "http", "url": "http://127.0.0.1:9890/mcp", "headers": {"Authorization": "Bearer x"}},
            "events": {"type": "sse", "url": "http://x/sse"}
        },
        "projects": {
            "/p": {"mcpServers": {"local": {"type": "stdio", "command": "node", "args": ["srv.js", "—flag"]}}, "other": 1}
        }
    })
}

#[test]
fn resolve_uses_home_and_codex_home() {
    let g = aidog_db::test_support::HomeGuard::new();
    let p = Paths::resolve().unwrap();
    assert_eq!(p.claude_json, g.home().join(".claude.json"));
    assert_eq!(p.codex, g.home().join(".codex"));
}

#[test]
fn claude_json_wrap_unwrap_round_trip_keeps_non_ascii_and_http() {
    let original = claude_fixture();
    let mut v = original.clone();
    assert!(sqz_apply_claude_json(&mut v, true).unwrap());

    let s = &v["mcpServers"];
    assert_eq!(s["rea"]["command"], "sqz-mcp");
    assert_eq!(
        s["rea"]["args"],
        json!([
            "proxy",
            "--",
            "mise",
            "exec",
            "node@22",
            "—-",
            "npx",
            "-y",
            "rea-agents",
            "mcp"
        ])
    );
    assert_eq!(s["gitnexus"]["env"], json!({"K": "v"}));
    assert_eq!(s["web"], original["mcpServers"]["web"]);
    assert_eq!(s["events"], original["mcpServers"]["events"]);
    assert_eq!(
        v["projects"]["/p"]["mcpServers"]["local"]["args"],
        json!(["proxy", "--", "node", "srv.js", "—flag"])
    );
    assert_eq!(
        s["sqz"],
        json!({"type": "stdio", "command": "sqz-mcp", "args": ["--transport", "stdio"]})
    );
    assert_eq!(
        sqz_missing(&enabled_hooks(), &v, None),
        ["codex:unavailable"]
    );

    // 再开一次：不重复包装
    let once = v.clone();
    assert!(!sqz_apply_claude_json(&mut v, true).unwrap());
    assert_eq!(v, once);

    assert!(sqz_apply_claude_json(&mut v, false).unwrap());
    assert_eq!(v, original);
    // 字节级：序列化后与原始一致（含 em dash）
    assert_eq!(
        serde_json::to_string(&v).unwrap(),
        serde_json::to_string(&original).unwrap()
    );
}

#[test]
fn sqz_self_server_uses_free_name_without_overwriting_existing_entries() {
    let mut v = json!({"mcpServers": {
        "sqz": {"type": "http", "url": "http://sqz"},
        "sqz-mcp": {"type": "http", "url": "http://sqz-mcp"},
        "sqz-mcp-self": {"type": "http", "url": "http://user-self"}
    }});
    assert!(sqz_apply_claude_json(&mut v, true).unwrap());
    assert_eq!(v["mcpServers"]["sqz-mcp-self-2"]["command"], SQZ_BIN);
    assert_eq!(
        v["mcpServers"]["sqz"],
        json!({"type": "http", "url": "http://sqz"})
    );
    assert_eq!(
        v["mcpServers"]["sqz-mcp"],
        json!({"type": "http", "url": "http://sqz-mcp"})
    );
    assert_eq!(
        v["mcpServers"]["sqz-mcp-self"],
        json!({"type": "http", "url": "http://user-self"})
    );
}

#[test]
fn custom_sqz_mcp_command_is_wrapped_and_restored_without_becoming_self_server() {
    let original = json!({"mcpServers": {
        "custom": {"type": "stdio", "command": "sqz-mcp", "args": ["worker", "--profile", "user"]}
    }});
    let mut v = original.clone();
    assert!(sqz_apply_claude_json(&mut v, true).unwrap());
    assert_eq!(
        v["mcpServers"]["custom"]["args"],
        json!(["proxy", "--", "sqz-mcp", "worker", "--profile", "user"])
    );
    assert!(
        v["mcpServers"]
            .as_object()
            .unwrap()
            .values()
            .any(|server| json_is(server, Kind::SelfServer))
    );
    assert!(sqz_apply_claude_json(&mut v, false).unwrap());
    assert_eq!(v, original);
}

#[test]
fn user_wrapped_server_is_not_double_wrapped_and_unwraps_to_rest() {
    let mut v = json!({"mcpServers": {
        "rea": {"type": "stdio", "command": "sqz-mcp", "args": ["proxy", "—-", "mise", "exec", "node@22", "--", "npx", "rea"]},
        "lazy": {"command": "sqz-mcp", "args": ["proxy", "--lazy-tools", "--", "graphify-mcp"]},
        "sqz": {"type": "stdio", "command": "sqz-mcp", "args": ["—transport", "stdio"]}
    }});
    let before = v.clone();
    assert!(!sqz_apply_claude_json(&mut v, true).unwrap());
    assert_eq!(v, before);

    assert!(sqz_apply_claude_json(&mut v, false).unwrap());
    let s = &v["mcpServers"];
    assert_eq!(s["rea"]["command"], "mise");
    assert_eq!(
        s["rea"]["args"],
        json!(["exec", "node@22", "--", "npx", "rea"])
    );
    assert_eq!(s["lazy"]["command"], "graphify-mcp");
    assert_eq!(s["lazy"]["args"], json!([]));
    assert!(s.get("sqz").is_none());
}

#[test]
fn codex_toml_wrap_unwrap_round_trip() {
    let text = r#"model = "gpt-5"

[mcp_servers.gitnexus]
command = "mise"
args = ["exec", "node@24", "—", "npx", "-y", "gitnexus"]

[mcp_servers.gitnexus.env]
K = "v"

[mcp_servers.aidog]
url = "http://127.0.0.1:9890/mcp"
"#;
    let original: toml::Table = toml::from_str(text).unwrap();
    let mut t = original.clone();
    assert!(sqz_apply_codex(&mut t, true).unwrap());
    let g = &t["mcp_servers"]["gitnexus"];
    assert_eq!(g["command"].as_str(), Some("sqz-mcp"));
    let args: Vec<&str> = g["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert_eq!(
        args,
        [
            "proxy", "--", "mise", "exec", "node@24", "—", "npx", "-y", "gitnexus"
        ]
    );
    assert_eq!(g["env"]["K"].as_str(), Some("v"));
    assert_eq!(t["mcp_servers"]["aidog"], original["mcp_servers"]["aidog"]);
    assert_eq!(t["mcp_servers"]["sqz"]["command"].as_str(), Some("sqz-mcp"));

    let once = t.clone();
    assert!(!sqz_apply_codex(&mut t, true).unwrap());
    assert_eq!(t, once);

    assert!(sqz_apply_codex(&mut t, false).unwrap());
    assert_eq!(t, original);
}

fn enabled_hooks() -> Value {
    let mut cfg = json!({});
    let mut hl = SqzHookLedger::default();
    sqz_apply_hooks_ledger(&mut cfg, true, &mut hl).unwrap();
    cfg
}

#[test]
fn status_off_when_one_server_unwrapped() {
    let mut cj = claude_fixture();
    sqz_apply_claude_json(&mut cj, true).unwrap();
    let mut cx: toml::Table = toml::from_str("[mcp_servers.a]\ncommand = \"x\"\n").unwrap();
    sqz_apply_codex(&mut cx, true).unwrap();
    assert!(sqz_missing(&enabled_hooks(), &cj, Some(&cx)).is_empty());

    cj["projects"]["/p"]["mcpServers"]["new"] = json!({"command": "y", "args": []});
    assert_eq!(
        sqz_missing(&enabled_hooks(), &cj, Some(&cx)),
        ["wrap:claude:/p:new"]
    );
    assert_eq!(
        sqz_missing(&json!({}), &cj, Some(&cx)),
        ["hook:PostToolUse", "hook:PreCompact", "wrap:claude:/p:new"]
    );
    let bare: toml::Table = toml::from_str("[mcp_servers.a]\ncommand = \"x\"\n").unwrap();
    assert_eq!(
        sqz_missing(&enabled_hooks(), &cj, Some(&bare)),
        ["wrap:claude:/p:new", "wrap:codex:a", "self:codex"]
    );
    assert_eq!(
        sqz_missing(&enabled_hooks(), &cj, None),
        ["wrap:claude:/p:new", "codex:unavailable"]
    );
}

#[test]
fn hooks_disable_keeps_user_hooks() {
    let mut hl = SqzHookLedger::default();
    let mut cfg = json!({"hooks": {
        "PostToolUse": [{"matcher": "Bash", "hooks": [
            {"type": "command", "command": "my-lint"},
            {"type": "command", "command": "sqz hook claude-post"}
        ]}],
        "SessionStart": [{"matcher": "compact", "hooks": [{"type": "command", "command": "sqz resume"}]}]
    }});
    sqz_apply_hooks_ledger(&mut cfg, true, &mut hl).unwrap();
    assert!(has_hook(&cfg, "PreCompact", "sqz hook precompact"));
    // 已有的 claude-post 不重复加
    assert_eq!(cfg["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);

    sqz_apply_hooks_ledger(&mut cfg, false, &mut hl).unwrap();
    // 开启前已有的 claude-post（记入 preexisting）保留；开启时新加的 precompact 删除。
    assert_eq!(
        cfg,
        json!({"hooks": {
            "PostToolUse": [{"matcher": "Bash", "hooks": [
                {"type": "command", "command": "my-lint"},
                {"type": "command", "command": "sqz hook claude-post"}
            ]}],
            "SessionStart": [{"matcher": "compact", "hooks": [{"type": "command", "command": "sqz resume"}]}]
        }})
    );
    assert!(!has_hook(&cfg, "PreCompact", "sqz hook precompact"));
}

#[test]
fn rtk_claude_hook_matches_user_shape() {
    let mut cfg = json!({"model": "x"});
    add_hook(&mut cfg, "PreToolUse", None, RTK_CLAUDE_CMD).unwrap();
    assert_eq!(
        cfg["hooks"]["PreToolUse"],
        json!([{"hooks": [{"type": "command", "command": "rtk hook claude"}]}])
    );
    remove_hook(&mut cfg, "PreToolUse", RTK_CLAUDE_CMD);
    assert_eq!(cfg, json!({"model": "x"}));
}

/// 模拟 `rtk init -g --codex`（rtk 1.2.0 隔离 HOME 实测产物形状）。
fn fake_rtk_init(codex: &Path) {
    let rtk_md = codex.join("RTK.md");
    std::fs::write(
        &rtk_md,
        "<!-- rtk-owned: written by `rtk init --codex` -->\n\n# Command output\n",
    )
    .unwrap();
    let hooks = codex.join("hooks.json");
    let mut v: Value = std::fs::read_to_string(&hooks)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}));
    add_hook(&mut v, "PreToolUse", Some("Bash"), RTK_CODEX_CMD).unwrap();
    std::fs::write(&hooks, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    let agents = codex.join("AGENTS.md");
    let cur = std::fs::read_to_string(&agents).unwrap_or_default();
    std::fs::write(&agents, format!("{cur}\n@{}\n", rtk_md.display())).unwrap();
}

#[test]
fn rtk_codex_disable_restores_exactly_what_was_recorded() {
    let tmp = tempfile::tempdir().unwrap();
    let codex = paths_in(tmp.path()).codex;
    std::fs::write(codex.join("AGENTS.md"), "user agents line\n").unwrap();
    std::fs::write(codex.join("config.toml"), "model = \"m\"\n").unwrap();

    let mut rec = RtkCodexRecord::default();
    let before = snapshot(&codex);
    fake_rtk_init(&codex);
    record_changes(&mut rec, before, snapshot(&codex));
    assert_eq!(rec.len(), 3);
    assert!(rtk_codex_on(&codex));

    // 再开一次不丢最早的 before
    let before = snapshot(&codex);
    record_changes(&mut rec, before, snapshot(&codex));
    assert_eq!(
        rec["AGENTS.md"].before.as_deref(),
        Some("user agents line\n")
    );

    rtk_codex_disable(&codex, &rec).unwrap();
    assert_eq!(
        std::fs::read_to_string(codex.join("AGENTS.md")).unwrap(),
        "user agents line\n"
    );
    assert!(!codex.join("RTK.md").exists());
    assert!(!codex.join("hooks.json").exists());
    assert_eq!(
        std::fs::read_to_string(codex.join("config.toml")).unwrap(),
        "model = \"m\"\n"
    );
    assert!(!rtk_codex_on(&codex));
}

#[test]
fn rtk_codex_disable_keeps_hook_not_written_by_aidog() {
    let tmp = tempfile::tempdir().unwrap();
    let codex = paths_in(tmp.path()).codex;
    let mut hooks = json!({});
    add_hook(&mut hooks, "PreToolUse", Some("Bash"), RTK_CODEX_CMD).unwrap();
    std::fs::write(
        codex.join("hooks.json"),
        serde_json::to_string_pretty(&hooks).unwrap(),
    )
    .unwrap();

    let mut rec = RtkCodexRecord::default();
    let before = snapshot(&codex);
    fake_rtk_init(&codex);
    record_changes(&mut rec, before, snapshot(&codex));
    assert!(!rec.contains_key("hooks.json"));

    rtk_codex_disable(&codex, &rec).unwrap();
    let current: Value =
        serde_json::from_str(&std::fs::read_to_string(codex.join("hooks.json")).unwrap()).unwrap();
    assert!(has_hook(&current, "PreToolUse", RTK_CODEX_CMD));
}

#[test]
fn rtk_codex_disable_after_user_edits_removes_only_rtk_parts() {
    let tmp = tempfile::tempdir().unwrap();
    let codex = paths_in(tmp.path()).codex;
    std::fs::write(codex.join("AGENTS.md"), "user agents line\n").unwrap();

    let mut rec = RtkCodexRecord::default();
    let before = snapshot(&codex);
    fake_rtk_init(&codex);
    record_changes(&mut rec, before, snapshot(&codex));

    // 用户事后改了生成说明，并追加了自己的指令
    let rtk = std::fs::read_to_string(codex.join("RTK.md")).unwrap();
    std::fs::write(codex.join("RTK.md"), format!("{rtk}# My RTK note\n")).unwrap();
    let agents = std::fs::read_to_string(codex.join("AGENTS.md")).unwrap();
    std::fs::write(codex.join("AGENTS.md"), format!("{agents}my rule\n")).unwrap();
    let mut hooks: Value =
        serde_json::from_str(&std::fs::read_to_string(codex.join("hooks.json")).unwrap()).unwrap();
    add_hook(&mut hooks, "Stop", None, "my-stop").unwrap();
    std::fs::write(
        codex.join("hooks.json"),
        serde_json::to_string(&hooks).unwrap(),
    )
    .unwrap();

    rtk_codex_disable(&codex, &rec).unwrap();
    let agents = std::fs::read_to_string(codex.join("AGENTS.md")).unwrap();
    assert!(agents.contains("user agents line\n") && agents.contains("my rule\n"));
    assert!(!agents.contains("RTK.md"));
    assert_eq!(
        std::fs::read_to_string(codex.join("RTK.md")).unwrap(),
        "# My RTK note\n"
    );
    let hooks: Value =
        serde_json::from_str(&std::fs::read_to_string(codex.join("hooks.json")).unwrap()).unwrap();
    assert_eq!(
        hooks,
        json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "my-stop"}]}]}})
    );
    assert!(!rtk_codex_on(&codex));
}

#[test]
fn sqz_file_transaction_rolls_back_when_second_write_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let claude = tmp.path().join("claude.json");
    let codex = tmp.path().join("config.toml");
    std::fs::write(&codex, "model = \"before\"\n").unwrap();
    let mut writes = 0;
    let result = write_transaction(
        vec![
            (claude.clone(), "{\"hooks\": {}}".to_string(), None),
            (
                codex.clone(),
                "model = \"after\"\n".to_string(),
                Some("model = \"before\"\n".to_string()),
            ),
        ],
        |path, content| {
            writes += 1;
            if writes == 2 {
                return Err("simulated second-write failure".into());
            }
            std::fs::write(path, content).map_err(|e| e.to_string())
        },
        |path| std::fs::remove_file(path).map_err(|e| e.to_string()),
    );

    assert!(
        result
            .unwrap_err()
            .contains("simulated second-write failure")
    );
    assert!(!claude.exists());
    assert_eq!(
        std::fs::read_to_string(codex).unwrap(),
        "model = \"before\"\n"
    );
}

#[tokio::test]
async fn sqz_files_refuse_invalid_json_and_write_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let db = aidog_db::test_support::test_db().await;
    let p = paths_in(tmp.path());
    std::fs::write(&p.claude_json, "{bad").unwrap();
    let cx = p.codex.join("config.toml");
    std::fs::write(&cx, "[mcp_servers.a]\ncommand = \"x\"\n").unwrap();
    assert!(sqz_apply_files_ledger(&p, true, &db).await.is_err());
    assert_eq!(std::fs::read_to_string(&p.claude_json).unwrap(), "{bad");
    assert_eq!(
        std::fs::read_to_string(&cx).unwrap(),
        "[mcp_servers.a]\ncommand = \"x\"\n"
    );
}

#[tokio::test]
async fn sqz_enable_creates_missing_codex_config_directory() {
    let db = aidog_db::test_support::test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let p = Paths {
        claude_json: tmp.path().join("claude.json"),
        codex: tmp.path().join("codex"),
    };
    std::fs::write(&p.claude_json, "{}").unwrap();
    let _ = &db;
    sqz_apply_files_ledger(&p, true, &db).await.unwrap();
    let config = p.codex.join("config.toml");
    let parsed: toml::Table = toml::from_str(&std::fs::read_to_string(config).unwrap()).unwrap();
    assert!(toml_is(&parsed["mcp_servers"]["sqz"], Kind::SelfServer));
}

#[tokio::test]
async fn sqz_files_refuse_invalid_toml_and_write_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let db = aidog_db::test_support::test_db().await;
    let p = paths_in(tmp.path());
    std::fs::write(&p.claude_json, "{\"settings\": true}").unwrap();
    let cx = p.codex.join("config.toml");
    std::fs::write(&cx, "[mcp_servers.a\ncommand = \"x\"").unwrap();
    assert!(sqz_apply_files_ledger(&p, true, &db).await.is_err());
    assert_eq!(
        std::fs::read_to_string(&p.claude_json).unwrap(),
        "{\"settings\": true}"
    );
    assert_eq!(
        std::fs::read_to_string(&cx).unwrap(),
        "[mcp_servers.a\ncommand = \"x\""
    );
}

#[tokio::test]
async fn sqz_files_enable_then_disable_round_trip_on_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let db = aidog_db::test_support::test_db().await;
    let p = paths_in(tmp.path());
    std::fs::write(
        &p.claude_json,
        serde_json::to_string_pretty(&claude_fixture()).unwrap(),
    )
    .unwrap();
    let cx = p.codex.join("config.toml");
    std::fs::write(&cx, "[mcp_servers.a]\ncommand = \"x\"\nargs = [\"—y\"]\n").unwrap();

    let _ = &db;
    sqz_apply_files_ledger(&p, true, &db).await.unwrap();
    assert_eq!(
        sqz_file_missing(&p, &enabled_hooks()).unwrap(),
        Vec::<String>::new()
    );
    sqz_apply_files_ledger(&p, false, &db).await.unwrap();
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&p.claude_json).unwrap()).unwrap();
    assert_eq!(v, claude_fixture());
    let t: toml::Table = toml::from_str(&std::fs::read_to_string(&cx).unwrap()).unwrap();
    assert_eq!(
        t["mcp_servers"]["a"]["args"].as_array().unwrap()[0].as_str(),
        Some("—y")
    );
    assert!(t["mcp_servers"].get("sqz").is_none());
}

/// 用户自装的 `sqz-mcp proxy --custom x`（未知旗标组合）关闭时不还原——旧逻辑按
/// Wrapped 撕开会把上游命令撕成 `--custom`；照 Plain 处理，原样保留。
#[test]
fn sqz_disable_leaves_unknown_flag_proxy_shape_untouched() {
    let original = json!({"mcpServers": {
        "user": {"type": "stdio", "command": "sqz-mcp", "args": ["proxy", "--custom", "x"]}
    }});
    let mut v = original.clone();
    assert!(!sqz_apply_claude_json(&mut v, false).unwrap());
    assert_eq!(v, original, "unknown flag combination must not be unwrapped");
}

/// 未知旗标组合照 Plain 处理：开启时再包一层，关闭时可还原回用户原始形状。
#[test]
fn sqz_unknown_flag_shape_is_wrapped_and_restored() {
    let original = json!({"mcpServers": {
        "user": {"type": "stdio", "command": "sqz-mcp", "args": ["proxy", "--custom", "x"]}
    }});
    let mut v = original.clone();
    assert!(sqz_apply_claude_json(&mut v, true).unwrap());
    assert_eq!(
        v["mcpServers"]["user"]["args"],
        json!(["proxy", "--", "sqz-mcp", "proxy", "--custom", "x"])
    );
    assert!(sqz_apply_claude_json(&mut v, false).unwrap());
    assert_eq!(v, original);
}

/// 用户旗标经 dash_norm 归一（`—no-desc` 同 `--no-desc`）也应识别为已知形状并还原。
#[test]
fn sqz_unwrap_tolerates_em_dash_flags() {
    let original = json!({"mcpServers": {
        "u": {"type": "stdio", "command": "sqz-mcp", "args": ["proxy", "—no-desc", "—", "mise", "exec"]}
    }});
    let mut v = original.clone();
    assert!(sqz_apply_claude_json(&mut v, false).unwrap());
    assert_eq!(v["mcpServers"]["u"]["command"], "mise");
    assert_eq!(v["mcpServers"]["u"]["args"], json!(["exec"]));
}

/// RTK.md 前插内容后，rtk 段按段边界定位删除，前插内容保留。
#[test]
fn rtk_codex_disable_removes_rtk_md_block_even_with_user_prelude() {
    let tmp = tempfile::tempdir().unwrap();
    let codex = paths_in(tmp.path()).codex;
    std::fs::write(codex.join("AGENTS.md"), "user agents line\n").unwrap();

    let mut rec = RtkCodexRecord::default();
    let before = snapshot(&codex);
    fake_rtk_init(&codex);
    record_changes(&mut rec, before, snapshot(&codex));

    let rtk = std::fs::read_to_string(codex.join("RTK.md")).unwrap();
    std::fs::write(codex.join("RTK.md"), format!("# My preamble\n\n{rtk}")).unwrap();

    rtk_codex_disable(&codex, &rec).unwrap();
    assert_eq!(
        std::fs::read_to_string(codex.join("RTK.md")).unwrap(),
        "# My preamble\n\n",
        "rtk block must be removed, user preamble kept"
    );
}

/// 记录的 rtk 段在当前文件中定位不到（用户整份重写）→ 报明确错误而非静默成功。
#[test]
fn rtk_codex_disable_errors_when_rtk_md_block_unlocatable() {
    let tmp = tempfile::tempdir().unwrap();
    let codex = paths_in(tmp.path()).codex;
    std::fs::write(codex.join("AGENTS.md"), "user agents line\n").unwrap();

    let mut rec = RtkCodexRecord::default();
    let before = snapshot(&codex);
    fake_rtk_init(&codex);
    record_changes(&mut rec, before, snapshot(&codex));

    std::fs::write(codex.join("RTK.md"), "totally rewritten by user\n").unwrap();

    let err = rtk_codex_disable(&codex, &rec).unwrap_err();
    assert!(err.contains("RTK.md"), "error must name the file, got: {err}");
}

/// ── sqz own-write ledger：关闭只删 aidog 写入的内容 ──────────────────
/// 用户自装的同形条目（手写 sqz-mcp stdio / 手加钩子 / 手写包装）关闭时必须原样保留：
/// 无记录可对上的条目一律不删（不再按形状猜）。
#[test]
fn sqz_disable_via_ledger_spares_user_written_entries() {
    // 1. 用户自写的 sqz-mcp --transport stdio 顶层服务（形状与 aidog 写的一样）
    let mut cj = json!({"mcpServers": {
        "mine": {"type": "stdio", "command": "sqz-mcp", "args": ["—transport", "stdio"]}
    }});
    let mut ledger = SqzLedger::default();
    assert!(!sqz_apply_claude_json_ledger(&mut cj, false, &mut ledger).unwrap());
    assert_eq!(
        cj["mcpServers"]["mine"],
        json!({"type": "stdio", "command": "sqz-mcp", "args": ["—transport", "stdio"]}),
        "user-written self server must survive disable"
    );

    // 2. 用户手加的 sqz 钩子：开启时已被记为 preexisting → 关闭不删
    let mut cfg = json!({"hooks": {
        "PreCompact": [{"hooks": [{"type": "command", "command": "sqz hook precompact"}]}]
    }});
    let mut hook_ledger = SqzHookLedger {
        preexisting: vec![("PreCompact".into(), "sqz hook precompact".into())],
    };
    sqz_apply_hooks_ledger(&mut cfg, false, &mut hook_ledger).unwrap();
    assert!(
        has_hook(&cfg, "PreCompact", "sqz hook precompact"),
        "user-written hook recorded as preexisting must survive"
    );

    // 3. 用户手写的包装层：无记录 → 不还原
    let mut cj2 = json!({"mcpServers": {
        "rea": {"type": "stdio", "command": "sqz-mcp", "args": ["proxy", "—-", "mise", "exec", "node@22"]}
    }});
    sqz_apply_claude_json_ledger(&mut cj2, false, &mut ledger).unwrap();
    assert_eq!(cj2["mcpServers"]["rea"]["command"], "sqz-mcp", "user-written wrapper must survive");
}

/// aidog 开启时记 ledger、关闭时按 ledger 精确还原（含用户改过键名后仍能对上）。
#[test]
fn sqz_enable_records_ledger_and_disable_restores_exactly() {
    let mut cj = claude_fixture();
    let mut ledger = SqzLedger::default();
    sqz_apply_claude_json_ledger(&mut cj, true, &mut ledger).unwrap();
    assert!(!ledger.is_empty(), "enable must record changes");

    // 用户改名 aidog 加的 self server（其余未动）
    if let Some(m) = cj.get_mut("mcpServers").and_then(Value::as_object_mut) {
        let renamed = m.remove("sqz").expect("aidog-added self server");
        m.insert("sqz-mcp-self".to_string(), renamed);
    }

    sqz_apply_claude_json_ledger(&mut cj, false, &mut ledger).unwrap();
    let s = &cj["mcpServers"];
    // 被包装的上游全部还原
    assert_eq!(s["rea"]["command"], "mise");
    assert_eq!(s["gitnexus"]["command"], "mise");
    assert_eq!(
        cj["projects"]["/p"]["mcpServers"]["local"]["command"],
        "node"
    );
    // 用户改名后的 self server 仍被删（ledger 记录的是写入时的内容对，不靠名字）
    assert!(s.get("sqz-mcp-self").is_none(), "aidog-added self server must be removed even after rename");
}

/// codex 侧同构：ledger 记录 → 关闭还原，用户自写条目保留。
#[test]
fn sqz_codex_ledger_round_trip_spares_user_entries() {
    let text = r#"
[mcp_servers.aidog-mine]
command = "sqz-mcp"
args = ["--transport", "stdio"]

[mcp_servers.up]
command = "x"
args = ["—y"]
"#;
    let mut t: toml::Table = toml::from_str(text).unwrap();
    let mut ledger = SqzLedger::default();
    sqz_apply_codex_ledger(&mut t, true, &mut ledger).unwrap();

    sqz_apply_codex_ledger(&mut t, false, &mut ledger).unwrap();
    let servers = &t["mcp_servers"];
    assert!(servers.get("aidog-mine").is_some(), "user-written codex server must survive");
    assert!(servers.get("sqz").is_none(), "aidog-added codex self server must be removed");
    assert_eq!(servers["up"]["command"], toml::Value::String("x".into()));
}

/// 计数删除（复核①）：同形条目多于 before=None 记录时，多出的（用户自写）保留。
/// 同名（未改名）条目优先消费记录；其余按内容逐条至多删一条。
#[test]
fn sqz_disable_inserted_removal_is_counted_per_record() {
    // claude：开启插入 self server → 用户再自写两份同形 → 关闭只删 aidog 的那份
    let mut cj = json!({"mcpServers": {}});
    let mut ledger = SqzLedger::default();
    sqz_apply_claude_json_ledger(&mut cj, true, &mut ledger).unwrap();
    let self_entry = cj["mcpServers"]["sqz"].clone();

    let mut cj2 = json!({"mcpServers": {
        "mine": self_entry.clone(),
        "mine2": self_entry.clone()
    }});
    cj2["mcpServers"]["sqz"] = self_entry.clone();
    assert!(sqz_apply_claude_json_ledger(&mut cj2, false, &mut ledger).unwrap());
    let s = &cj2["mcpServers"];
    assert!(s.get("sqz").is_none(), "aidog-added self server must be removed");
    assert_eq!(s["mine"], self_entry, "user-written same-shape entry must survive (counted removal)");
    assert_eq!(s["mine2"], self_entry, "surplus user entries preserved when records exhausted");

    // codex：同构
    let mut t: toml::Table = toml::from_str("[mcp_servers]\n").unwrap();
    let mut ledger2 = SqzLedger::default();
    sqz_apply_codex_ledger(&mut t, true, &mut ledger2).unwrap();
    let self_val = t["mcp_servers"]["sqz"].clone();
    {
        let servers = t.get_mut("mcp_servers").unwrap().as_table_mut().unwrap();
        servers.insert("mine".into(), self_val.clone());
        servers.insert("mine2".into(), self_val.clone());
    }
    assert!(sqz_apply_codex_ledger(&mut t, false, &mut ledger2).unwrap());
    let servers = t["mcp_servers"].as_table().unwrap();
    assert!(!servers.contains_key("sqz"), "aidog-added codex self server must be removed");
    assert!(
        servers.contains_key("mine") && servers.contains_key("mine2"),
        "surplus user codex entries preserved, got keys: {:?}",
        servers.keys().collect::<Vec<_>>()
    );
}

/// wrap 残留报告（复核④）：改名后的 wrapped 条目（内容 = ledger 写后、键名对不上）
/// 报 `stale-wrap:`；可还原的不报；用户自写同形包装（内容无记录）不报。
#[test]
fn sqz_wrap_residue_reports_renamed_wrapped_entries_only() {
    let mut cj = claude_fixture();
    let mut ledger = SqzLedger::default();
    sqz_apply_claude_json_ledger(&mut cj, true, &mut ledger).unwrap();

    // 用户把 aidog 包装的 "rea" 改名成 "mine"：内容仍是 ledger 写后内容，键名对不上
    let mut renamed = cj.clone();
    if let Some(m) = renamed.get_mut("mcpServers").and_then(Value::as_object_mut) {
        let e = m.remove("rea").expect("wrapped entry present");
        m.insert("mine".to_string(), e);
    }
    let out = sqz_wrap_residue(&renamed, None, &ledger);
    assert_eq!(out, vec!["stale-wrap:claude:mine"], "renamed wrapped entry must be reported, got {out:?}");

    // 未改名（关闭可还原）不报
    let out2 = sqz_wrap_residue(&cj, None, &ledger);
    assert!(out2.is_empty(), "restorable wrapped entries are not residue, got {out2:?}");

    // 用户自写同形包装（ledger 无此内容）不报
    let mut manual = cj.clone();
    manual["mcpServers"]["userwrap"] =
        json!({"type": "stdio", "command": "sqz-mcp", "args": ["proxy", "--", "usercmd", "--x"]});
    let out3 = sqz_wrap_residue(&manual, None, &ledger);
    assert!(
        out3.iter().all(|k| !k.contains("userwrap")),
        "user-written same-shape wrap is not residue, got {out3:?}"
    );

    // codex 同构：改名 wrapped server 报 stale-wrap:codex:<n>
    let text = r#"
[mcp_servers.up]
command = "x"
args = ["—y"]
"#;
    let mut t: toml::Table = toml::from_str(text).unwrap();
    let mut ledger2 = SqzLedger::default();
    sqz_apply_codex_ledger(&mut t, true, &mut ledger2).unwrap();
    {
        let servers = t.get_mut("mcp_servers").unwrap().as_table_mut().unwrap();
        let e = servers.remove("up").unwrap();
        servers.insert("mine".into(), e);
    }
    let out4 = sqz_wrap_residue(&json!({}), Some(&t), &ledger2);
    assert_eq!(out4, vec!["stale-wrap:codex:mine"], "renamed codex wrapped entry must be reported, got {out4:?}");
}

// ─── 统计解析（sqz stats --json / rtk gain --format json 一手样例）───

#[tokio::test]
async fn rtk_double_write_reports_failures_per_side_and_runs_sibling() {
    let (claude, codex) = rtk_apply_both(
        async { Err::<(), _>("claude config failed".to_string()) },
        async { Ok::<(), String>(()) },
    )
    .await;
    assert_eq!(claude.as_deref(), Some("claude config failed"));
    assert_eq!(codex, None);

    let (claude, codex) = rtk_apply_both(
        async { Ok::<(), String>(()) },
        async { Err::<(), _>("codex init failed".to_string()) },
    )
    .await;
    assert_eq!(claude, None);
    assert_eq!(codex.as_deref(), Some("codex init failed"));
}

#[test]
fn parse_sqz_and_rtk_stats_from_cli_json() {
    let sqz_json = r#"{"avgReduction":27.56,"cacheEntries":91610,"cacheSize":12345,"regretExpands":143,"regretReruns":1,"tokensIn":23276773,"tokensOut":16860995,"tokensSaved":6415778,"totalCompressions":19037}"#;
    let sqz = parse_sqz_stats(sqz_json).expect("sqz stats json parses");
    assert_eq!(sqz.total_compressions, 19037);
    assert_eq!(sqz.tokens_saved, 6415778);
    assert!((sqz.avg_reduction - 27.56).abs() < 1e-9);

    let rtk_json = r#"{"summary":{"total_commands":227267,"total_input":68748427,"total_output":44138825,"total_saved":24670175,"avg_savings_pct":35.88,"total_time_ms":1,"avg_time_ms":1140,"oversized_excluded":3653}}"#;
    let rtk = parse_rtk_gain(rtk_json).expect("rtk gain json parses");
    assert_eq!(rtk.total_commands, 227267);
    assert_eq!(rtk.total_saved, 24670175);
    assert!((rtk.avg_savings_pct - 35.88).abs() < 1e-9);

    // 非法 / 空输出 → None（统计缺失不报错）
    assert!(parse_sqz_stats("").is_none());
    assert!(parse_sqz_stats("not json").is_none());
    assert!(parse_rtk_gain("{}").is_none());
    assert!(parse_rtk_gain("").is_none());
}
