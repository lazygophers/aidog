import type { EnvVar } from "../../services/api";

/** Build the `claude` CLI invocation for a given group settings file */
export function buildClaudeCommand(settingsName: string): string {
  return [
    "claude",
    "--brief",
    "--dangerously-skip-permissions",
    "--settings",
    `~/.aidog/settings.${settingsName}.json`,
  ].join(" ");
}

/** POSIX shell 单引号安全转义（内部单引号闭合/转义/重开），杜绝注入。 */
export function shellSquote(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

/**
 * Build the `codex` CLI invocation for a given group profile.
 * `AIDOG_KEY=<group>`（auth token=分组名，aidog 据此路由）+ `codex -p <group>`
 * 选 `~/.codex/<group>.config.toml` profile + bypass approvals/sandbox。
 *
 * Codex config.toml 不支持 env 注入（research/codex-env-support.md），用户 env_vars
 * 经前置 `export KEY=VALUE;` 注入 codex 进程环境。AIDOG_KEY 为 aidog 路由 token，
 * 用户同名变量须丢弃（shell 后者覆盖前者会破坏路由）。
 */
export function buildCodexCommand(groupKey: string, envVars?: EnvVar[]): string {
  const g = shellSquote(groupKey);
  const exports = (envVars ?? [])
    .filter(ev => ev.key.trim() !== "" && ev.value !== "" && ev.key !== "AIDOG_KEY")
    .map(ev => `export ${ev.key}=${shellSquote(ev.value)};`);
  return [
    ...exports,
    `AIDOG_KEY=${g}`,
    "codex",
    "-p",
    g,
    "--dangerously-bypass-approvals-and-sandbox",
    "-a",
    "never",
  ].join(" ");
}
