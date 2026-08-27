// ─── settings 键 ↔ 环境变量 配对同步 ───────────────────────────
//
// Claude Code 里有一批配置存在两种写法：settings.json 的键（如 `fastMode`）和
// 环境变量（如 `CLAUDE_CODE_DISABLE_FAST_MODE`）。只写一边时，另一边残留的旧值会
// 按 Claude Code 自身的优先级（环境变量高于 settings）反压回来 —— 界面上改了却不
// 生效，是最难自查的一类问题。
//
// 策略（用户 2026-08-27 决策）：**谁后改谁为准**。不设主从，改哪一边就把另一边写成
// 等价值；删哪一边就把另一边一起删。同步是单趟的，不递归，不会来回打架。
//
// 配对只收「文档上明确等价」的项，不靠名字相近猜。语义相反的（`fastMode` 开 ↔
// `..._DISABLE_FAST_MODE` 关）标 polarity: "inverted"。

/** 一对等价配置：settings 键与环境变量。 */
export interface SettingEnvPair {
  /** settings.json 里的键名。 */
  settingKey: string;
  /** 等价的环境变量名。 */
  envKey: string;
  /** boolean 两边都是开关；value 两边都是标量（字符串/数字）。 */
  kind: "boolean" | "value";
  /** 仅 boolean 用：inverted 表示两边语义相反（一个叫「开」一个叫「禁用」）。 */
  polarity?: "same" | "inverted";
}

export const SETTING_ENV_PAIRS: SettingEnvPair[] = [
  { settingKey: "fastMode", envKey: "CLAUDE_CODE_DISABLE_FAST_MODE", kind: "boolean", polarity: "inverted" },
  { settingKey: "autoCompactEnabled", envKey: "DISABLE_AUTO_COMPACT", kind: "boolean", polarity: "inverted" },
  { settingKey: "autoMemoryEnabled", envKey: "CLAUDE_CODE_DISABLE_AUTO_MEMORY", kind: "boolean", polarity: "inverted" },
  { settingKey: "fileCheckpointingEnabled", envKey: "CLAUDE_CODE_DISABLE_FILE_CHECKPOINTING", kind: "boolean", polarity: "inverted" },
  { settingKey: "includeGitInstructions", envKey: "CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS", kind: "boolean", polarity: "inverted" },
  { settingKey: "syntaxHighlightingDisabled", envKey: "CLAUDE_CODE_SYNTAX_HIGHLIGHT", kind: "boolean", polarity: "inverted" },
  { settingKey: "autoConnectIde", envKey: "CLAUDE_CODE_AUTO_CONNECT_IDE", kind: "boolean", polarity: "same" },
  { settingKey: "disableArtifact", envKey: "CLAUDE_CODE_DISABLE_ARTIFACT", kind: "boolean", polarity: "same" },
  { settingKey: "disableWorkflows", envKey: "CLAUDE_CODE_DISABLE_WORKFLOWS", kind: "boolean", polarity: "same" },
  { settingKey: "disableBundledSkills", envKey: "CLAUDE_CODE_DISABLE_BUNDLED_SKILLS", kind: "boolean", polarity: "same" },
  { settingKey: "disableAgentView", envKey: "CLAUDE_CODE_DISABLE_AGENT_VIEW", kind: "boolean", polarity: "same" },
  { settingKey: "effortLevel", envKey: "CLAUDE_CODE_EFFORT_LEVEL", kind: "value" },
  { settingKey: "autoCompactWindow", envKey: "CLAUDE_CODE_AUTO_COMPACT_WINDOW", kind: "value" },
  { settingKey: "model", envKey: "ANTHROPIC_MODEL", kind: "value" },
];

const PAIR_BY_SETTING = new Map(SETTING_ENV_PAIRS.map((p) => [p.settingKey, p]));
const PAIR_BY_ENV = new Map(SETTING_ENV_PAIRS.map((p) => [p.envKey, p]));

/** 环境变量的真值解析，与 EnvEditor.envBool 同规则。 */
function envBool(v: string | undefined): boolean {
  if (!v) return false;
  return ["1", "true", "yes", "on"].includes(v.toLowerCase());
}

/** settings 值 → 环境变量字符串。 */
function envFromSetting(pair: SettingEnvPair, value: unknown): string {
  if (pair.kind === "boolean") {
    const on = value === true;
    return (pair.polarity === "inverted" ? !on : on) ? "1" : "0";
  }
  return String(value);
}

/** 环境变量字符串 → settings 值。 */
function settingFromEnv(pair: SettingEnvPair, value: string): unknown {
  if (pair.kind === "boolean") {
    const on = envBool(value);
    return pair.polarity === "inverted" ? !on : on;
  }
  return value;
}

/** 与 Settings.tsx 原有语义一致：undefined / null / 空串视为「未设置」，false 与 0 保留。 */
function isUnset(value: unknown): boolean {
  return value === undefined || value === null || value === "";
}

/**
 * 在配置对象上写入一个字段，并把有配对关系的另一侧同步成等价值。
 *
 * - `field` 是 settings 键 → 顺带写 / 删 `env` 里对应的环境变量
 * - `field === "env"` → 逐个比对变动的环境变量，顺带写 / 删对应的 settings 键
 * - 未配对的字段：行为与同步前完全一致
 *
 * 纯函数，返回新对象，不改入参。
 */
export function updateConfigField(
  prev: Record<string, any>,
  field: string,
  value: unknown,
): Record<string, any> {
  const next: Record<string, any> = { ...prev };
  if (isUnset(value)) delete next[field];
  else next[field] = value;

  if (field === "env") {
    const prevEnv = (prev.env ?? {}) as Record<string, string>;
    const nextEnv = (value ?? {}) as Record<string, string>;
    // 只看真正变动的环境变量：没动过的不去覆盖 settings 侧，
    // 否则用户单独调整某个 settings 键的动作会在下一次 env 编辑里被抹掉。
    const touched = new Set([...Object.keys(prevEnv), ...Object.keys(nextEnv)]);
    for (const envKey of touched) {
      const pair = PAIR_BY_ENV.get(envKey);
      if (!pair || prevEnv[envKey] === nextEnv[envKey]) continue;
      const after = nextEnv[envKey];
      if (isUnset(after)) delete next[pair.settingKey];
      else next[pair.settingKey] = settingFromEnv(pair, after);
    }
    return next;
  }

  const pair = PAIR_BY_SETTING.get(field);
  if (!pair) return next;

  const env: Record<string, string> = { ...((prev.env ?? {}) as Record<string, string>) };
  if (isUnset(value)) delete env[pair.envKey];
  else env[pair.envKey] = envFromSetting(pair, value);
  if (Object.keys(env).length > 0) next.env = env;
  else delete next.env;
  return next;
}
