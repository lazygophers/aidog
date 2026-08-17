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

/** aidog 为分组生成的 pi provider id 前缀。与 Rust `gateway::pi::PROVIDER_PREFIX` 一致。 */
export const PI_PROVIDER_PREFIX = "aidog-";

/**
 * Build the `pi` CLI invocation for a given group.
 * pi 的 endpoint 只认单一全局 `~/.pi/agent/models.json`，aidog 在其中为每组写一个
 * provider `aidog-<group>`，用 `--provider` 选组。token 已写进该 provider 的 apiKey，
 * 因此命令行不带任何路由 env。
 */
export function buildPiCommand(groupKey: string, envVars?: EnvVar[]): string {
  const exports = (envVars ?? [])
    .filter(ev => ev.key.trim() !== "" && ev.value !== "")
    .map(ev => `export ${ev.key}=${shellSquote(ev.value)};`);
  return [
    ...exports,
    "pi",
    "--provider",
    shellSquote(`${PI_PROVIDER_PREFIX}${groupKey}`),
  ].join(" ");
}
