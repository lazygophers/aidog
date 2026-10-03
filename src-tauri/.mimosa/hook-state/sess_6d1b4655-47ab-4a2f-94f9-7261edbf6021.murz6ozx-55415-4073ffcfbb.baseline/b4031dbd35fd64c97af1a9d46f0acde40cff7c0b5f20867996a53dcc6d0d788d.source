//! 单 skill 写操作：enable / install / disable / update / uninstall(_all) + 命令 arg 构造。
//!
//! 注：原 `fs_fallback_remove`（非 npx 物理删 `~/.agents/skills`）已于 2026-06-25
//! skills-removal-recur 止血移除（唯一非用户主动的潜在 fs 删入口）。用户主动 uninstall
//! 现仅走 `npx skills remove`（npx.rs 有 `cfg(test)` 测试守卫）。第三方/手动 symlink
//! skill 若 npx 不识别将不删文件 — 止血期可接受降级。

use super::npx::{apply_scope, run_npx_in_scope};
use super::types::{SkillAgent, SkillScope, SkillsOpResult};

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
///
/// F3 诊断：执行 npx remove 前记 warn 日志含 skill/scope/args（半物理删，删该 agent symlink），
/// 便于 support 从事后日志追溯「谁删的」。trigger 由调用方的 `#[tracing::instrument]` span 提供。
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
    tracing::warn!(
        skill = %name,
        agent = ?agent,
        scope = ?scope,
        args = ?args,
        trigger = "skills_disable/align_agents",
        "物理删除 skill：npx skills remove（半物理：删该 agent 启用）"
    );
    run_npx_in_scope(&args, scope, proxy_url)
}

/// 构造 update（更新）命令 args：`update [-g] -y`。
/// 抽出便于单测断言（不真跑 npx）。行为对称 `enable_args` / `disable_args`。
pub(super) fn update_args(scope: &SkillScope) -> Vec<String> {
    let mut args = vec!["update".to_string()];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    args
}

/// 更新已装 skills：`npx skills update [-g] -y`。
pub fn update(scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    let args = update_args(scope);
    run_npx_in_scope(&args, scope, proxy_url)
}

/// 构造一键卸载全部命令 args：`remove --all [-g]`。
/// 抽出便于单测断言（不真跑 npx，避免破坏性 `npx skills remove --all -g` 操作用户 `~/.agents`）。
/// 行为对称 `enable_args` / `disable_args` / `uninstall_args`。
pub(super) fn uninstall_all_args(scope: &SkillScope) -> Vec<String> {
    let mut args = vec!["remove".to_string(), "--all".to_string()];
    apply_scope(&mut args, scope);
    args
}

/// 一键卸载当前 scope 下所有平台所有 skills：`npx skills remove --all [-g]`。
/// `--all` = `--skill '*' --agent '*' -y`（删规范存储 + 所有 agent symlink）。
///
/// F3 诊断：执行 npx remove 前记 warn 日志含 scope/args（真物理删，不可恢复）。
pub fn uninstall_all(scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    let args = uninstall_all_args(scope);
    tracing::warn!(
        scope = ?scope,
        args = ?args,
        trigger = "skills_uninstall_all",
        "物理删除 skill：npx skills remove --all（真物理删：规范存储 + 所有 agent symlink，不可恢复）"
    );
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
/// **止血决策 (2026-06-25, skills-removal-recur)**：移除原 fs 兜底删路径
/// (`fs_fallback_remove`)。fs 兜底是唯一非 npx 的 `~/.agents/skills` 物理删入口，
/// 不受 npx.rs 的 cfg(test) 守卫保护，任何调用链误触都会真删用户数据。
/// 用户主动卸载现在完全走 `npx skills remove -s <name>`，npx 已有测试守卫；
/// 第三方/手动 symlink skill（npx 返 "No matching skills found"）卸载将不删文件，
/// 属止血期可接受降级（用户可在文件管理器手动删 symlink）。
/// 详见 prd `06-25-skills-removal-recur` 加固方案 3。
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
    tracing::warn!(
        skill = %name,
        scope = ?scope,
        args = ?args,
        trigger = "skills_uninstall",
        "物理删除 skill：npx skills remove -s <name>（真物理删：规范存储 + 所有 agent symlink，不可恢复）"
    );
    run_npx_in_scope(&args, scope, proxy_url)
}

/// 校验 skill name 安全（防路径遍历：禁 `..` / `/` / `\` / 空 / `.`）。
///
/// 仅测试用：fs 兜底删已在 06-25-skills-removal-recur 止血移除，本函数无生产消费者。
/// 保留作纵深防御输入校验，后续若恢复兜底删（带更强运行时守卫）可复用。
#[cfg(test)]
pub(super) fn is_safe_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
}

#[cfg(test)]
#[path = "test_ops.rs"]
mod test_ops;
