//! 单 skill 写操作：enable / install / disable / update / uninstall(_all) + 命令 arg 构造 + fs 兜底删。

use super::npx::{apply_scope, run_npx_in_scope};
use super::types::{SkillAgent, SkillScope, SkillsOpResult};
use std::fs;
use std::path::PathBuf;

/// 构造 enable（启用）命令 args：`add <path> -a <slug> [-g] -y`。
/// 用 skill 本地 path 作 add package（list json `path`），对所有 skill 通用，不依赖锁文件 source。
/// 单 skill 目录 add 无需 `-s <name>`。抽出便于单测断言（不真跑 npx）。
pub(super) fn enable_args(path: &str, agent: SkillAgent, scope: &SkillScope) -> Vec<String> {
    let mut args = vec![
        "add".to_string(),
        path.to_string(),
        "-a".to_string(),
        agent.cli_slug().to_string(),
    ];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    args
}

/// 构造 disable（关闭）命令 args：`remove -s <name> -a <slug> [-g] -y`。
pub(super) fn disable_args(name: &str, agent: SkillAgent, scope: &SkillScope) -> Vec<String> {
    let mut args = vec![
        "remove".to_string(),
        "-s".to_string(),
        name.to_string(),
        "-a".to_string(),
        agent.cli_slug().to_string(),
    ];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    args
}

/// 为某 agent 启用 skill：用 skill 本地 path 作 add package → `npx skills add <path> -a <slug> [-g] -y`。
/// path 为空 → 明确错误。Project scope 在项目目录内执行（不带 `-g`）。
pub fn enable(
    name: &str,
    path: &str,
    agent: SkillAgent,
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    let name = name.trim();
    if name.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill name is empty".to_string(),
        };
    }
    let path = path.trim();
    if path.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: format!("skill '{name}' has no installed path; cannot enable"),
        };
    }
    let args = enable_args(path, agent, scope);
    run_npx_in_scope(&args, scope, proxy_url)
}

/// `npx skills add <id> -a <slug> [-g] -y` 参数构造。`id` 含 `@skill`，无需 `-s`。
pub(super) fn install_args(id: &str, agent: SkillAgent, scope: &SkillScope) -> Vec<String> {
    let mut args = vec![
        "add".to_string(),
        id.to_string(),
        "-a".to_string(),
        agent.cli_slug().to_string(),
    ];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    args
}

/// 从 catalog 安装 skill（`npx skills add <id> -a <slug> [-g] -y`，逐 agent 合并）。
///
/// `id` = CatalogEntry.id，形如 `owner/repo@skill`（`@skill` 已选定子 skill，无需 `-s`）。
/// 多 agent 逐个 `run_npx_in_scope`；任一失败 → success=false，stderr 聚合全部失败明细。
/// 成功后调用方负责 `invalidate(scope)`。
pub fn install(
    id: &str,
    agents: &[SkillAgent],
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    let id = id.trim();
    if id.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill id is empty".to_string(),
        };
    }
    if agents.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "no agent selected".to_string(),
        };
    }
    let mut success = true;
    let mut stdout = String::new();
    let mut stderr = String::new();
    for agent in agents {
        let args = install_args(id, *agent, scope);
        let res = run_npx_in_scope(&args, scope, proxy_url);
        if !res.success {
            success = false;
            if !stderr.is_empty() {
                stderr.push_str("\n---\n");
            }
            stderr.push_str(&format!("[{}] {}", agent.cli_slug(), res.stderr.trim()));
        }
        if !res.stdout.trim().is_empty() {
            if !stdout.is_empty() {
                stdout.push_str("\n---\n");
            }
            stdout.push_str(&format!("[{}] {}", agent.cli_slug(), res.stdout.trim()));
        }
    }
    SkillsOpResult {
        success,
        stdout,
        stderr,
    }
}

/// 为某 agent 关闭 skill：`npx skills remove -s <name> -a <slug> [-g] -y`。
pub fn disable(
    name: &str,
    agent: SkillAgent,
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    let name = name.trim();
    if name.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill name is empty".to_string(),
        };
    }
    let args = disable_args(name, agent, scope);
    run_npx_in_scope(&args, scope, proxy_url)
}

/// 更新已装 skills：`npx skills update [-g] -y`。
pub fn update(scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    let mut args = vec!["update".to_string()];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    run_npx_in_scope(&args, scope, proxy_url)
}

/// 一键卸载当前 scope 下所有平台所有 skills：`npx skills remove --all [-g]`。
/// `--all` = `--skill '*' --agent '*' -y`（删规范存储 + 所有 agent symlink）。
pub fn uninstall_all(scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    let mut args = vec!["remove".to_string(), "--all".to_string()];
    apply_scope(&mut args, scope);
    run_npx_in_scope(&args, scope, proxy_url)
}

/// 构造单一 skill 卸载 args：`remove -s <name> [-g] -y`。
/// **不带 `-a`** = 删该 skill 在所有 agent 的启用配置 + 规范存储（实测验证）。
/// ⚠️ `-a` 不接受通配（`-a '*'` 报 `Invalid agents: *` exit 1）；仅 `--all` 简写内部展开。
/// 故单 skill 全卸载只能省略 `-a`，等效 `--all` 但限定单个 skill。
pub(super) fn uninstall_args(name: &str, scope: &SkillScope) -> Vec<String> {
    let mut args = vec!["remove".to_string(), "-s".to_string(), name.to_string()];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    args
}

/// 卸载单一 skill：`npx skills remove -s <name> [-g] -y`（破坏性，前端二次确认）。
/// 删规范存储目录 + 所有 agent 的启用配置（symlink / 锁文件项）。
///
/// **fs 兜底**：第三方/手动 symlink skill（如 understand-*，非 npx 装、不在锁文件）
/// npx remove 返回 "No matching skills found"（exit 0 但没删）。检测到此输出 → fs 兜底
/// 删规范存储 symlink + 各 agent 目录 symlink（用户决策 A，突破"全 npx"约束，对称于
/// enable 用 path 绕锁文件）。
pub fn uninstall(name: &str, scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    let name = name.trim();
    if name.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill name is empty".to_string(),
        };
    }
    let args = uninstall_args(name, scope);
    let res = run_npx_in_scope(&args, scope, proxy_url);
    // 检测 npx 不认该 skill（第三方/手动 symlink，非锁文件注册）→ fs 兜底删。
    let no_match = res.stdout.contains("No matching skills found")
        || res.stderr.contains("No matching skills found");
    if no_match {
        let (removed, errs) = fs_fallback_remove(name, scope);
        let success = !removed.is_empty() && errs.is_empty();
        return SkillsOpResult {
            success,
            stdout: format!(
                "fs fallback removed {} path(s): [{}]",
                removed.len(),
                removed.join(", ")
            ),
            stderr: if errs.is_empty() {
                String::new()
            } else {
                format!("fs fallback errors: {}", errs.join("; "))
            },
        };
    }
    res
}

/// 校验 skill name 安全（防路径遍历：禁 `..` / `/` / `\` / 空 / `.`）。
pub(super) fn is_safe_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
}

/// 删除单个路径（symlink → remove_file 不跟随；目录 → remove_dir_all；不存在 → skip）。
/// 返回 Some(()) 表示删成功，None 表示不存在，Some(Err) 转 errs。
fn remove_path(p: &PathBuf, removed: &mut Vec<String>, errs: &mut Vec<String>) {
    let meta = match fs::symlink_metadata(p) {
        Ok(m) => m,
        Err(_) => return, // 不存在 → skip
    };
    let r = if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p) // symlink 或文件：不跟随 symlink target
    };
    match r {
        Ok(()) => removed.push(p.display().to_string()),
        Err(e) => errs.push(format!("remove {}: {e}", p.display())),
    }
}

/// fs 兜底删第三方/手动 symlink skill。返回 (已删路径, 错误)。
///
/// - **规范存储**：global `~/.agents/skills/<name>`，project `<project>/.agents/skills/<name>`。
/// - **各 agent symlink**（仅 global）：扫 `~/` 下 `.` 开头目录（.claude/.codex/.trae-cn/...），
///   若 `<dir>/skills/<name>` 存在则删。不硬编码 agent 列表，通配扫。
///
/// 安全：name 经 `is_safe_skill_name` 校验，防路径遍历。
fn fs_fallback_remove(name: &str, scope: &SkillScope) -> (Vec<String>, Vec<String>) {
    let mut removed: Vec<String> = Vec::new();
    let mut errs: Vec<String> = Vec::new();

    if !is_safe_skill_name(name) {
        return (removed, vec![format!("unsafe skill name: '{name}'")]);
    }

    // case-insensitive 匹配：`npx skills list` 返 name 小写化，
    // 但磁盘目录保留原大小写（如 cc-switch 管的 `SkillAnything`）。
    // 列目录条目，to_lowercase() == name.to_lowercase() 即匹配删除。
    let name_lc = name.to_lowercase();
    let remove_in = |dir: &std::path::Path, removed: &mut Vec<String>, errs: &mut Vec<String>| {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().to_lowercase() == name_lc {
                remove_path(&e.path(), removed, errs);
            }
        }
    };

    match scope {
        SkillScope::Global => {
            if let Some(home) = dirs::home_dir() {
                // 规范存储
                remove_in(&home.join(".agents").join("skills"), &mut removed, &mut errs);
                // 各 agent 目录（home 下 . 开头目录，排除 .agents 本身）
                if let Ok(entries) = fs::read_dir(&home) {
                    for e in entries.flatten() {
                        let dir_name = e.file_name();
                        let dn = dir_name.to_string_lossy();
                        if !dn.starts_with('.') || dn == ".agents" {
                            continue;
                        }
                        remove_in(&home.join(dn.as_ref()).join("skills"), &mut removed, &mut errs);
                    }
                }
            } else {
                errs.push("cannot resolve home directory".to_string());
            }
        }
        SkillScope::Project { path } => {
            remove_in(
                &PathBuf::from(path).join(".agents").join("skills"),
                &mut removed,
                &mut errs,
            );
        }
    }

    (removed, errs)
}

#[cfg(test)]
#[path = "test_ops.rs"]
mod test_ops;
