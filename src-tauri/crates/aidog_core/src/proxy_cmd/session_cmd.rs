//! 会话「一键继续」（2026-11-21 日志统计-会话列表 票）：在系统终端打开并执行
//! `claude --resume <session_id> --settings <home>/.aidog/settings.<group>.json`（绝对路径：单引号内 `~` 不展开），
//! 沿用会话原分组路由（settings.<group>.json = 分组代理地址 + 密钥，与
//! Groups 页「复制启动命令」同一链路）。仅 macOS（Terminal.app + osascript）。
//!
//! 命令注入防护：session_id 与分组名拼进命令前过 POSIX 单引号转义
//! （`shell_squote`，与前端 `domains/groups/commands.ts::shellSquote` 同算法），
//! AppleScript 双引号串再过一层 `apple_script_escape`。测试只测字符串拼装，
//! 不真跑 osascript（test-no-real-calls.md）。

/// POSIX shell 单引号安全转义：内部单引号闭合 / 转义 / 重开（`'\''`）。
pub(crate) fn shell_squote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// AppleScript 字符串字面量转义（反斜杠与双引号）。
pub(crate) fn apple_script_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '\\' || c == '"' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// 拼装 resume 命令（`shell_squote` 后拼进 shell 行）。`aidog_dir` = `~/.aidog` 的绝对路径。分组空 = 不带 `--settings`
///（无分组会话回落默认配置；`claude --resume <sid>` 本身已可续会话）。
pub(crate) fn build_resume_command(session_id: &str, group_key: &str, aidog_dir: &std::path::Path) -> String {
    let mut parts = vec!["claude".to_string(), "--resume".to_string(), shell_squote(session_id)];
    if !group_key.is_empty() {
        parts.push("--settings".to_string());
        parts.push(shell_squote(&aidog_dir.join(format!("settings.{group_key}.json")).to_string_lossy()));
    }
    parts.join(" ")
}

/// `session_resume` 结果：launched=已在 Terminal.app 打开；false = 本平台不能开终端，
/// 前端把 `command` 复制给用户自己跑。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResumeResult {
    pub launched: bool,
    pub command: String,
}

/// 四家继续命令行（扩展-会话页 票 07，spec S4 / S5）：workdir 非空先 `cd -- <dir> &&`。
/// Gemini 会话按项目目录隔离，无 workdir → Err。
pub(crate) fn build_session_resume_line(
    client_code: &str,
    session_id: &str,
    group_key: &str,
    workdir: &str,
    aidog_dir: &std::path::Path,
) -> Result<String, String> {
    if session_id.is_empty() {
        return Err("session_id is empty".to_string());
    }
    // 单引号挡不住「以 - 开头被当选项」与换行，这两类直接拒。
    for (name, v) in [("session_id", session_id), ("group_key", group_key), ("workdir", workdir)] {
        if v.starts_with('-') || v.chars().any(char::is_control) {
            return Err(format!("invalid {name}"));
        }
    }
    let sid = shell_squote(session_id);
    let cmd = match client_code {
        "claude_code" => build_resume_command(session_id, group_key, aidog_dir),
        "codex" if group_key.is_empty() => format!("codex resume {sid}"),
        "codex" => {
            let g = shell_squote(group_key);
            format!("AIDOG_KEY={g} codex -p {g} resume {sid}")
        }
        "gemini_cli" if workdir.is_empty() => return Err("gemini_cli session has no workdir".to_string()),
        "gemini_cli" => format!("gemini --resume {sid}"),
        "opencode" if workdir.is_empty() => format!("opencode -s {sid}"),
        "opencode" => format!("opencode -s {sid} {}", shell_squote(workdir)),
        other => return Err(format!("unsupported client: {other}")),
    };
    Ok(if workdir.is_empty() {
        cmd
    } else {
        format!("cd -- {} && {cmd}", shell_squote(workdir))
    })
}

/// 统一继续入口的可测面：`run` 拿到整行命令执行（生产 = osascript 开终端，测试 = 记录），
/// 返回是否已打开终端。opencode_root 为 None = 本机无 OpenCode 数据目录。
pub(crate) async fn session_resume_with(
    db: &aidog_db::Db,
    client_code: &str,
    session_id: &str,
    workdir: &str,
    opencode_root: Option<&std::path::Path>,
    aidog_dir: &std::path::Path,
    run: &(dyn Fn(&str) -> Result<bool, String> + Send + Sync),
) -> Result<SessionResumeResult, String> {
    // 先做纯校验（Gemini 无目录、非法参数），失败不查库。
    build_session_resume_line(client_code, session_id, "", workdir, aidog_dir)?;
    if client_code == "opencode" {
        let root = opencode_root.ok_or("opencode data dir not found")?.to_path_buf();
        let sid = session_id.to_string();
        let exists = tokio::task::spawn_blocking(move || {
            crate::client_usage_cmd::reader_opencode::session_exists(&root, &sid)
        })
        .await
        .map_err(|e| e.to_string())?;
        if !exists {
            return Err(format!("opencode session not found: {session_id}"));
        }
    }
    let group = if matches!(client_code, "claude_code" | "codex") {
        aidog_logs::session_group_key(db, session_id).await?.unwrap_or_default()
    } else {
        String::new()
    };
    let command = build_session_resume_line(client_code, session_id, &group, workdir, aidog_dir)?;
    let launched = run(&command)?;
    Ok(SessionResumeResult { launched, command })
}

/// `~/.aidog` 的绝对路径：命令里的路径在单引号内，shell 不展开 `~`（票 11）。
fn aidog_dir() -> Result<std::path::PathBuf, String> {
    Ok(dirs::home_dir().ok_or("cannot resolve home directory")?.join(".aidog"))
}

/// osascript 参数（票 12）：`activate` 把 Terminal 切到前台——只 `do script` 时新窗口落在 AiDog 后面，
/// 用户看到「点了没反应」。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn osascript_args(line: &str) -> Vec<String> {
    let script = format!("do script \"{}\"", apple_script_escape(line));
    ["tell application \"Terminal\"", "activate", &script, "end tell"]
        .iter()
        .flat_map(|s| ["-e".to_string(), s.to_string()])
        .collect()
}

/// osascript 结束后的结果判定：非 0 退出（含拒绝自动化授权 -1743）→ Err 带 stderr 原文。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn osascript_result(success: bool, stderr: &[u8]) -> Result<(), String> {
    if success {
        return Ok(());
    }
    let msg = String::from_utf8_lossy(stderr).trim().to_string();
    Err(if msg.is_empty() { "osascript failed".to_string() } else { msg })
}

/// 等 osascript 结束（不留僵尸进程）并按退出码判定。授权弹窗会让它阻塞到用户点选，
/// 故在 async 命令里经 `block_in_place` 调用。
#[cfg(target_os = "macos")]
fn run_osascript(line: &str) -> Result<(), String> {
    let out = std::process::Command::new("osascript")
        .args(osascript_args(line))
        .output()
        .map_err(|e| format!("open Terminal: {e}"))?;
    osascript_result(out.status.success(), &out.stderr)
}

/// 生产执行器：macOS 开 Terminal.app 执行整行命令；其它平台不执行（返回 false 供前端复制）。
fn open_terminal(line: &str) -> Result<bool, String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = line;
        Ok(false)
    }
    #[cfg(target_os = "macos")]
    {
        tokio::task::block_in_place(|| run_osascript(line))?;
        Ok(true)
    }
}

crate::tauri_command! {
/// 统一「继续会话」（扩展-会话页 票 07）：四家客户端各自的继续命令，先 cd 到会话工作目录；
/// Claude / Codex 沿用 proxy_log 记录的原分组。无测试直调：真跑会弹终端窗（测 `session_resume_with`）。
pub async fn session_resume(
    client_code: String,
    session_id: String,
    workdir: String) -> Result<SessionResumeResult, String> {
    let oc = crate::client_usage_cmd::client_root("opencode");
    let dir = aidog_dir()?;
    session_resume_with(aidog_ctx::db(), &client_code, &session_id, &workdir, oc.as_deref(), &dir, &open_terminal).await
}
}

crate::tauri_command! {
/// 会话一键继续：macOS 用 osascript 开 Terminal.app 执行 resume 命令
/// （沿用会话原分组 setting）。非 macOS 返回 Err。无测试：真跑会弹终端窗。
pub async fn session_open_terminal(
    session_id: String,
    group_key: String) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&session_id, &group_key);
        return Err("open_terminal unsupported on this platform".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        if session_id.is_empty() {
            return Err("session_id is empty".to_string());
        }
        let line = build_resume_command(&session_id, &group_key, &aidog_dir()?);
        tokio::task::block_in_place(|| run_osascript(&line))
    }
}
}

#[cfg(test)]
mod test_session_cmd {
    use super::*;

    const AD: &str = "/Users/demo/.aidog";
    fn ad() -> &'static std::path::Path {
        std::path::Path::new(AD)
    }

    /// POSIX 单引号转义：内嵌单引号与 shell 元字符都出不来（与前端 shellSquote 同算法同结果）。
    #[test]
    fn shell_squote_escapes_metacharacters() {
        assert_eq!(shell_squote("abc"), "'abc'");
        assert_eq!(shell_squote("a'b; rm -rf /"), "'a'\\''b; rm -rf /'");
        assert_eq!(shell_squote(""), "''");
    }

    /// AppleScript 转义：双引号与反斜杠不出字符串字面量。
    #[test]
    fn apple_script_escape_quotes() {
        assert_eq!(apple_script_escape("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(apple_script_escape("a\\b"), "a\\\\b");
    }

    /// resume 命令拼装：session_id / 分组名带注入载荷时被单引号困住；空分组不带 --settings。
    #[test]
    fn resume_command_escapes_and_guards() {
        assert_eq!(
            build_resume_command("deadbeef", "teamA", ad()),
            "claude --resume 'deadbeef' --settings '/Users/demo/.aidog/settings.teamA.json'"
        );
        let cmd = build_resume_command("x'; rm -rf /", "g' ; calc", ad());
        assert!(cmd.starts_with("claude --resume 'x'\\''; rm -rf /'"), "{cmd}");
        assert!(cmd.contains("--settings '/Users/demo/.aidog/settings.g'\\'' ; calc.json'"), "{cmd}");
        assert_eq!(build_resume_command("deadbeef", "", ad()), "claude --resume 'deadbeef'");
    }

    /// 票 11：`~` 在单引号内不展开，--settings 必须是后端解析出的绝对路径（仍单引号包裹）。
    #[test]
    fn resume_settings_path_is_absolute() {
        let dir = std::path::Path::new("/Users/my user/.aidog");
        let cmd = build_resume_command("s1", "teamA", dir);
        assert_eq!(cmd, "claude --resume 's1' --settings '/Users/my user/.aidog/settings.teamA.json'");
        assert!(!cmd.contains('~'), "{cmd}");
    }

    /// 四家 × 有 / 无分组 × 有 / 无目录：拼出的整行命令逐字符对齐。
    #[test]
    fn session_resume_line_four_clients() {
        let b = |c: &str, s: &str, g: &str, w: &str| build_session_resume_line(c, s, g, w, ad());
        assert_eq!(
            b("claude_code", "s1", "teamA", "/w/p").unwrap(),
            "cd -- '/w/p' && claude --resume 's1' --settings '/Users/demo/.aidog/settings.teamA.json'"
        );
        assert_eq!(b("claude_code", "s1", "", "").unwrap(), "claude --resume 's1'");
        assert_eq!(
            b("codex", "u-1", "teamA", "/w/p").unwrap(),
            "cd -- '/w/p' && AIDOG_KEY='teamA' codex -p 'teamA' resume 'u-1'"
        );
        assert_eq!(b("codex", "u-1", "", "").unwrap(), "codex resume 'u-1'");
        assert_eq!(b("gemini_cli", "g-1", "", "/w/p").unwrap(), "cd -- '/w/p' && gemini --resume 'g-1'");
        assert_eq!(
            b("opencode", "ses_1", "", "/w/p").unwrap(),
            "cd -- '/w/p' && opencode -s 'ses_1' '/w/p'"
        );
        assert_eq!(b("opencode", "ses_1", "", "").unwrap(), "opencode -s 'ses_1'");
    }

    /// Gemini / OpenCode 不注入分组：传了分组也不出现在命令里。
    #[test]
    fn session_resume_line_gemini_opencode_ignore_group() {
        let g = build_session_resume_line("gemini_cli", "g-1", "teamA", "/w", ad()).unwrap();
        let o = build_session_resume_line("opencode", "ses_1", "teamA", "/w", ad()).unwrap();
        assert!(!g.contains("teamA") && !o.contains("teamA"), "{g} | {o}");
    }

    /// 防注入：含空格 / 单引号 / `;` 的 id、目录、分组名全被单引号困住；
    /// 以 `-` 开头（会被当选项）与含控制字符的参数直接拒绝。
    #[test]
    fn session_resume_line_injection() {
        let line = build_session_resume_line("codex", "a'b; rm -rf ~", "g' ;x", "/tmp/my dir'; echo pwn", ad()).unwrap();
        assert_eq!(
            line,
            "cd -- '/tmp/my dir'\\''; echo pwn' && AIDOG_KEY='g'\\'' ;x' codex -p 'g'\\'' ;x' resume 'a'\\''b; rm -rf ~'"
        );
        let o = build_session_resume_line("opencode", "x y", "", "/a b", ad()).unwrap();
        assert_eq!(o, "cd -- '/a b' && opencode -s 'x y' '/a b'");
        for (sid, wd) in [("--yolo", "/w"), ("ok", "-rf"), ("a\nb", "/w"), ("ok", "/w\r"), ("", "/w")] {
            assert!(build_session_resume_line("claude_code", sid, "", wd, ad()).is_err(), "{sid:?} {wd:?}");
        }
        assert!(build_session_resume_line("claude_code", "ok", "g\nx", "/w", ad()).is_err());
    }

    /// 票 12：osascript 先 activate 再 do script，Terminal 切到前台；命令行经 AppleScript 转义。
    #[test]
    fn osascript_args_activate_terminal() {
        let args = osascript_args("echo \"hi\"");
        let script: Vec<&str> = args.chunks(2).map(|p| { assert_eq!(p[0], "-e"); p[1].as_str() }).collect();
        assert_eq!(script, vec!["tell application \"Terminal\"", "activate", "do script \"echo \\\"hi\\\"\"", "end tell"]);
    }

    /// 票 12：非 0 退出码 → Err 带 stderr 原文（拒绝自动化授权不再被吞）；0 退出 → Ok。
    #[test]
    fn osascript_nonzero_exit_is_err() {
        let e = osascript_result(false, b"execution error: Not authorized to send Apple events to Terminal. (-1743)\n").unwrap_err();
        assert!(e.contains("-1743"), "{e}");
        assert!(osascript_result(false, b"").is_err());
        assert_eq!(osascript_result(true, b""), Ok(()));
    }

    /// Gemini 无目录 / 未知客户端 → Err。
    #[test]
    fn session_resume_line_errors() {
        assert!(build_session_resume_line("gemini_cli", "g-1", "", "", ad()).is_err());
        assert!(build_session_resume_line("pi", "x", "", "/w", ad()).is_err());
    }

    fn oc_db(dir: &std::path::Path, ids: &[&str]) -> std::path::PathBuf {
        let p = dir.join("opencode.db");
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute_batch("CREATE TABLE session_v2 (id TEXT PRIMARY KEY, parent_id TEXT, title TEXT, directory TEXT);")
            .unwrap();
        for id in ids {
            c.execute("INSERT INTO session_v2 (id, parent_id, title, directory) VALUES (?1, '', 't', '/w')", [id])
                .unwrap();
        }
        p
    }

    /// 统一入口：Claude / Codex 按 session_id 取 proxy_log 最近非空分组；Gemini / OpenCode 不取；
    /// OpenCode 会话不存在 → Err 且不执行；执行器拿到的就是返回的命令。
    #[tokio::test]
    async fn session_resume_with_groups_and_opencode_guard() {
        let (db, _d) = aidog_db::test_support::four_file_db().await;
        for (id, sid, group, at) in [("l1", "c1", "old", 1_000), ("l2", "c1", "teamA", 2_000), ("l3", "c1", "", 3_000), ("l4", "x1", "teamX", 1_000)] {
            let mut log = aidog_db::test_support::sample_log(id, group, at);
            log.session_id = sid.into();
            aidog_logs::upsert_proxy_log(&db, log).await.unwrap();
        }
        let oc = tempfile::tempdir().unwrap();
        let oc_root = oc_db(oc.path(), &["ses_ok"]);
        let ran = std::sync::Mutex::new(Vec::<String>::new());
        let run = |l: &str| {
            ran.lock().unwrap().push(l.to_string());
            Ok(true)
        };
        let r = session_resume_with(&db, "claude_code", "c1", "/w", Some(&oc_root), ad(), &run).await.unwrap();
        assert_eq!(r, SessionResumeResult { launched: true, command: "cd -- '/w' && claude --resume 'c1' --settings '/Users/demo/.aidog/settings.teamA.json'".into() });
        let r = session_resume_with(&db, "codex", "x1", "", Some(&oc_root), ad(), &run).await.unwrap();
        assert_eq!(r.command, "AIDOG_KEY='teamX' codex -p 'teamX' resume 'x1'");
        let r = session_resume_with(&db, "codex", "nogw", "", Some(&oc_root), ad(), &run).await.unwrap();
        assert_eq!(r.command, "codex resume 'nogw'");
        let r = session_resume_with(&db, "gemini_cli", "c1", "/w", Some(&oc_root), ad(), &run).await.unwrap();
        assert_eq!(r.command, "cd -- '/w' && gemini --resume 'c1'");
        let r = session_resume_with(&db, "opencode", "ses_ok", "/w", Some(&oc_root), ad(), &run).await.unwrap();
        assert_eq!(r.command, "cd -- '/w' && opencode -s 'ses_ok' '/w'");
        assert_eq!(ran.lock().unwrap().len(), 5);
        // 不存在 / 无 OpenCode 数据目录 → Err，执行器不被调用。
        assert!(session_resume_with(&db, "opencode", "ses_missing", "/w", Some(&oc_root), ad(), &run).await.is_err());
        assert!(session_resume_with(&db, "opencode", "ses_ok", "/w", None, ad(), &run).await.is_err());
        assert!(session_resume_with(&db, "gemini_cli", "c1", "", Some(&oc_root), ad(), &run).await.is_err());
        assert_eq!(ran.lock().unwrap().len(), 5);
        // 执行器报不能开终端 → launched=false，命令照样返回供复制。
        let r = session_resume_with(&db, "codex", "nogw", "", None, ad(), &|_| Ok(false)).await.unwrap();
        assert_eq!(r, SessionResumeResult { launched: false, command: "codex resume 'nogw'".into() });
        // 执行器失败（osascript 非 0 退出）→ Err 透传给前端，不再报 launched。
        let e = session_resume_with(&db, "codex", "nogw", "", None, ad(), &|_| Err("osascript: -1743".into())).await;
        assert_eq!(e, Err("osascript: -1743".to_string()));
    }
}
