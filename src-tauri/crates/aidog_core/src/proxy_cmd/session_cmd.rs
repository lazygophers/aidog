//! 会话「一键继续」（2026-11-21 日志统计-会话列表 票）：在系统终端打开并执行
//! `claude --resume <session_id> --settings ~/.aidog/settings.<group>.json`，
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

/// 拼装 resume 命令（`shell_squote` 后拼进 shell 行）。分组空 = 不带 `--settings`
///（无分组会话回落默认配置；`claude --resume <sid>` 本身已可续会话）。
pub(crate) fn build_resume_command(session_id: &str, group_key: &str) -> String {
    let mut parts = vec!["claude".to_string(), "--resume".to_string(), shell_squote(session_id)];
    if !group_key.is_empty() {
        parts.push("--settings".to_string());
        parts.push(shell_squote(&format!("~/.aidog/settings.{group_key}.json")));
    }
    parts.join(" ")
}

crate::tauri_command! {
/// 会话一键继续：macOS 用 osascript 开 Terminal.app 执行 resume 命令
/// （沿用会话原分组 setting）。非 macOS 返回 Err。无测试：真跑会弹终端窗。
pub fn session_open_terminal(
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
        let shell_line = build_resume_command(&session_id, &group_key);
        let script = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            apple_script_escape(&shell_line)
        );
        std::process::Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| format!("open Terminal: {e}"))?;
        Ok(())
    }
}
}

#[cfg(test)]
mod test_session_cmd {
    use super::*;

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
            build_resume_command("deadbeef", "teamA"),
            "claude --resume 'deadbeef' --settings '~/.aidog/settings.teamA.json'"
        );
        let cmd = build_resume_command("x'; rm -rf /", "g' ; calc");
        assert!(cmd.starts_with("claude --resume 'x'\\''; rm -rf /'"), "{cmd}");
        assert!(cmd.contains("--settings '~/.aidog/settings.g'\\'' ; calc.json'"), "{cmd}");
        assert_eq!(build_resume_command("deadbeef", ""), "claude --resume 'deadbeef'");
    }
}
