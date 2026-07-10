//! 生成脚本的执行器选择与 command 串构造（uv / python3）。
//!
//! aidog 生成的 Python 脚本（通知 hook + statusline）由 `uv run --script`（uv 可用）
//! 或 `python3`（fallback）执行。哪个执行器写进各客户端的 command 串（Claude Code
//! `hooks.*.command` / `statusLine.command`、Codex `notify[]`）由此模块统一决定，避免
//! hook / statusline / codex 各自拼串漂移（见 code-reuse-rules: command 串构造抽公共函数）。
//!
//! 纯逻辑（无副作用、无 IO），便于单测；live 检测（`which uv`）与持久化在 command 层（lib.rs）。

/// 脚本执行器：uv 可用走 `uv run --script <path>`，否则 `python3 <path>`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptInvoker {
    /// `uv run --script <path>`（uv 已安装；PEP723 内联依赖隔离）。
    Uv,
    /// `python3 <path>`（uv 不可用回退；脚本仅用 stdlib，python3 足够）。
    Python3,
}

impl ScriptInvoker {
    /// 由「uv 是否可用」解析执行器（true → Uv，false → Python3）。
    pub fn from_uv_available(uv_available: bool) -> Self {
        if uv_available {
            ScriptInvoker::Uv
        } else {
            ScriptInvoker::Python3
        }
    }

    /// 由持久化设置串解析（`"uv"` → Uv，其余/缺失 → Python3）。
    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("uv") => ScriptInvoker::Uv,
            _ => ScriptInvoker::Python3,
        }
    }

    /// 持久化用字符串。
    pub fn as_setting(self) -> &'static str {
        match self {
            ScriptInvoker::Uv => "uv",
            ScriptInvoker::Python3 => "python3",
        }
    }

    /// 由脚本绝对路径构造完整 command 串。
    ///
    /// 路径含空格时整体用双引号包裹（command 串会被 shell 解析）。
    pub fn command_for(self, script_path: &str) -> String {
        let quoted = quote_if_needed(script_path);
        match self {
            ScriptInvoker::Uv => format!("uv run --script {quoted}"),
            ScriptInvoker::Python3 => format!("python3 {quoted}"),
        }
    }
}

/// 路径含空白/shell 特殊字符时用双引号包裹（简易：仅处理空格/制表符，
/// aidog 脚本路径在 `~/.aidog/scripts/` 下、文件名固定，不含引号本身）。
fn quote_if_needed(path: &str) -> String {
    if path.chars().any(|c| c.is_whitespace()) {
        format!("\"{path}\"")
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_uv_available_maps() {
        assert_eq!(ScriptInvoker::from_uv_available(true), ScriptInvoker::Uv);
        assert_eq!(
            ScriptInvoker::from_uv_available(false),
            ScriptInvoker::Python3
        );
    }

    #[test]
    fn from_setting_parses() {
        assert_eq!(ScriptInvoker::from_setting(Some("uv")), ScriptInvoker::Uv);
        assert_eq!(
            ScriptInvoker::from_setting(Some("python3")),
            ScriptInvoker::Python3
        );
        // 缺失/未知 → python3（保守 fallback）。
        assert_eq!(ScriptInvoker::from_setting(None), ScriptInvoker::Python3);
        assert_eq!(
            ScriptInvoker::from_setting(Some("bogus")),
            ScriptInvoker::Python3
        );
    }

    #[test]
    fn setting_roundtrip() {
        assert_eq!(
            ScriptInvoker::from_setting(Some(ScriptInvoker::Uv.as_setting())),
            ScriptInvoker::Uv
        );
        assert_eq!(
            ScriptInvoker::from_setting(Some(ScriptInvoker::Python3.as_setting())),
            ScriptInvoker::Python3
        );
    }

    #[test]
    fn command_for_uv() {
        assert_eq!(
            ScriptInvoker::Uv.command_for("/home/u/.aidog/scripts/aidog-notify-complete.py"),
            "uv run --script /home/u/.aidog/scripts/aidog-notify-complete.py"
        );
    }

    #[test]
    fn command_for_python3() {
        assert_eq!(
            ScriptInvoker::Python3.command_for("/home/u/.aidog/scripts/aidog-statusline.py"),
            "python3 /home/u/.aidog/scripts/aidog-statusline.py"
        );
    }

    #[test]
    fn command_quotes_path_with_space() {
        assert_eq!(
            ScriptInvoker::Uv.command_for("/home/My User/.aidog/scripts/x.py"),
            "uv run --script \"/home/My User/.aidog/scripts/x.py\""
        );
    }
}
