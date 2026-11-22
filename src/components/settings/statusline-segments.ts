// ─── Statusline segment model + data tables (pure, no React) ───
// Extracted from statusline-gen.ts (arch-redesign phase 6 S6): the large
// constant tables (SEGMENT_DEFS + derived) and their type definitions live
// here; statusline-gen.ts keeps the pure generation functions and re-exports
// every symbol below so consumers import paths stay unchanged.
//
// Pure data only — no React/Tauri imports — safe to load in a Node test harness.
// Prefix defaults follow the UI language via i18n (same keys as Rust statusline.rs default_options).

import i18n from "../../locales";

const PREFIX = {
  balance: () => i18n.t("statusline.prefix.balance", "Balance "),
  windowCost: () => i18n.t("statusline.prefix.windowCost", "≈$"),
  cache: () => i18n.t("statusline.prefix.cache", "Cache "),
};

/**
 * 未改动的语言化默认前缀（等于 defaults.prefix 且由 PREFIX 管）不存进 options，
 * 显示时按当前语言取；用户改过（含清空成空串）原样保留。
 */
export function omitLocalizedDefaultPrefix(
  opts: Record<string, any>,
  defaults: Record<string, any>,
): Record<string, any> {
  const p = opts.prefix;
  if (p === undefined || p !== defaults.prefix || !Object.values(PREFIX).some((f) => f() === p)) return opts;
  const { prefix: _drop, ...rest } = opts;
  return rest;
}

export type RowAlign = "left" | "center" | "right";

export type SegmentType =
  | "model"          // Model display name
  | "context-bar"    // Context window progress bar
  | "context-pct"    // Context window percentage
  | "git"            // Git branch + repo
  | "cost"           // API cost + duration
  | "rate-limits"    // Rate limit usage
  | "effort"         // Effort level
  | "vim"            // Vim mode
  | "separator"      // Visual separator (· or |)
  // ── Atomic segments (one per raw statusline input field) ──
  // Cost / execution
  | "cost-usd"               // cost.total_cost_usd
  | "session-duration"       // cost.total_duration_ms
  | "api-duration"           // cost.total_api_duration_ms
  | "lines-changed"          // cost.total_lines_added / removed
  // Context window
  | "context-tokens"         // context_window.total_input/output_tokens
  | "context-max"            // context_window.context_window_size
  | "context-remaining"      // context_window.remaining_percentage
  | "context-cache"          // context_window.current_usage.cache_*_tokens
  // Rate limits (per window)
  | "rate-limit-5h"          // rate_limits.five_hour
  | "rate-limit-7d"          // rate_limits.seven_day
  | "spend-limit"            // rate_limits.spend_limit
  // Git
  | "git-branch"             // git -C <cwd> branch --show-current（脚本内跑 git）
  | "git-host"               // workspace.repo.host
  | "git-owner"              // workspace.repo.owner
  | "git-repo"               // workspace.repo.name
  | "git-repo-full"          // owner/name
  | "git-worktree"           // workspace.git_worktree
  // Directory / session
  | "cwd"                    // workspace.current_dir
  | "project-dir"            // workspace.project_dir
  | "added-dirs"             // workspace.added_dirs
  | "session-id"             // session_id
  | "session-name"           // session_name
  | "transcript-path"        // transcript_path
  // Worktree
  | "worktree-name"          // worktree.name
  | "worktree-branch"        // worktree.branch
  | "worktree-original-branch" // worktree.original_branch
  | "worktree-path"          // worktree.path
  | "worktree-original-cwd"  // worktree.original_cwd
  // PR
  | "pr-number"              // pr.number
  | "pr-url"                 // pr.url
  | "pr-state"               // pr.review_state
  | "pr-kind"                // pr.kind
  // Other single fields
  | "version"                // version
  | "output-style"           // output_style.name
  | "thinking"               // thinking.enabled
  | "token-warn"             // exceeds_200k_tokens
  | "agent"                  // agent.name
  | "agent-badge"            // 子代理徽章 [type·status·model]（动态符号/色）
  | "prompt-cache"           // prompt_cache.*
  | "fast-mode"              // fast_mode
  | "agent-type"             // 子代理 task.agentType
  | "task-description"       // 子代理 task.description
  | "task-effort"            // 子代理 task.effort
  // aidog group segments
  | "group-balance"  // aidog group: 预估余额
  | "group-spent"    // aidog group: 累计预估花费
  | "group-window-cost" // aidog group: 本配额周期折算花费（coding plan，0 隐藏）
  | "group-coding"   // aidog group: coding plan 利用率
  | "group-requests" // aidog group: 请求数 · 成功率
  | "group-cache"    // aidog group: 缓存命中率
  | "group-tokens"   // aidog group: 已使用总 tokens
  | "group-route"    // aidog group: 组名/最近命中平台名（glm/GLM-自用）
  | "custom";        // Custom jq expression

export interface StatusLineSegment {
  id: string;
  type: SegmentType;
  enabled: boolean;
  newline: boolean; // insert line break before this segment (row leader when true)
  options: Record<string, any>;
  color?: string;      // fixed hex foreground color, e.g. "#4A9EFF"
  autoColor?: boolean; // value-class segments: derive color from value via thresholds
  align?: RowAlign;    // row alignment — only meaningful on the row-leading segment
}

export interface SegmentDef {
  type: SegmentType;
  name: string;
  icon: string;
  desc: string;
  defaultOptions: Record<string, any>;
  /** Render preview text (static mock) */
  toPreview: (opts: Record<string, any>) => string;
  /** Editable fields for the modal */
  fields: { key: string; label: string; type: "string" | "number" | "select"; options?: string[]; placeholder?: string }[];
}

/** Segment types whose value can drive automatic semantic coloring. */
export const VALUE_COLORABLE: Set<SegmentType> = new Set([
  "context-pct", "context-bar", "cost", "rate-limits",
  // Atomic value-class segments
  "cost-usd", "context-remaining", "rate-limit-5h", "rate-limit-7d",
  "session-duration", "api-duration", "spend-limit",
]);

/** Segment types that consume the shared aidog group-info endpoint. */
export const GROUP_SEG_TYPES = new Set<SegmentType>([
  "group-balance", "group-spent", "group-window-cost", "group-coding",
  "group-requests", "group-cache", "group-tokens", "group-route",
]);

export const SEGMENT_DEFS: SegmentDef[] = [
  {
    type: "model",
    name: "Model Name",
    icon: "core",
    desc: "Current model display name",
    defaultOptions: { format: "short" },
    toPreview: (o) => o.format === "full" ? "claude-sonnet-4-6" : "Opus",
    fields: [
      { key: "format", label: "Format", type: "select", options: ["short", "full"] },
    ],
  },
  {
    type: "context-bar",
    name: "Context Bar",
    icon: "status",
    desc: "10-char progress bar + percentage",
    defaultOptions: { width: 10, filled: "▓", empty: "░" },
    toPreview: (o) => {
      const w = o.width || 10;
      const pct = 65;
      const filled = Math.round(pct * w / 100);
      return (o.filled || "▓").repeat(filled) + (o.empty || "░").repeat(w - filled) + ` ${pct}%`;
    },
    fields: [
      { key: "width", label: "Width", type: "number", placeholder: "10" },
      { key: "filled", label: "Filled Char", type: "string", placeholder: "▓" },
      { key: "empty", label: "Empty Char", type: "string", placeholder: "░" },
    ],
  },
  {
    type: "context-pct",
    name: "Context Percentage",
    icon: "status",
    desc: "Percentage number only",
    defaultOptions: { suffix: "%" },
    // `degradeZero` (subagent default): emit nothing when ctx% is absent/0 so the
    // `affixPre` separator also drops — mirrors ccplugin which omits ctx for tasks
    // with no real context data. Main statusline omits the flag → always `0%`.
    toPreview: () => "65%",
    fields: [],
  },
  {
    type: "git",
    name: "Git Status",
    icon: "folder",
    desc: "Branch name + repo name",
    defaultOptions: { showRepo: false },
    toPreview: (o) => o.showRepo ? "anthropics/claude-code" : "claude-code",
    fields: [
      { key: "showRepo", label: "Show full path (owner/name)", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "cost",
    name: "Cost Tracking",
    icon: "bolt",
    desc: "API cost + duration",
    defaultOptions: { showDuration: true },
    toPreview: (o) => o.showDuration ? "$0.12 · 155s" : "$0.12",
    fields: [
      { key: "showDuration", label: "Show duration", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "rate-limits",
    name: "Rate Limits",
    icon: "permissions",
    desc: "5h / 7d limit usage percentage",
    defaultOptions: { windows: "both" },
    toPreview: (o) => o.windows === "5h" ? "5h:23%" : o.windows === "7d" ? "7d:41%" : "5h:23% 7d:41%",
    fields: [
      { key: "windows", label: "Window", type: "select", options: ["both", "5h", "7d"] },
    ],
  },
  {
    type: "effort",
    name: "Effort Level",
    icon: "behavior",
    desc: "Reasoning effort level",
    defaultOptions: {},
    toPreview: () => "high",
    fields: [],
  },
  {
    type: "vim",
    name: "Vim Mode",
    icon: "ui",
    desc: "Current vim mode",
    defaultOptions: {},
    toPreview: () => "NORMAL",
    fields: [],
  },
  {
    type: "separator",
    name: "Separator",
    icon: "advanced",
    desc: "Visual separator",
    defaultOptions: { char: "·" },
    toPreview: (o) => (typeof o.char === "string" ? o.char : "·"),
    fields: [
      { key: "char", label: "Separator Char", type: "string", placeholder: "·" },
    ],
  },
  {
    type: "group-balance",
    name: "Group Balance",
    icon: "bolt",
    desc: "Estimated remaining balance of the group's sole enabled platform (single-enabled-platform groups only)",
    get defaultOptions() { return { prefix: PREFIX.balance(), dynamicColor: false }; },
    toPreview: (o) => `${o.prefix ?? PREFIX.balance()}48.20`,
    fields: [
      { key: "prefix", label: "Prefix", type: "string", get placeholder() { return PREFIX.balance(); } },
      { key: "dynamicColor", label: "Dynamic color (by days left)", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "group-spent",
    name: "Group Spent",
    icon: "bolt",
    desc: "Cumulative estimated spend of the current group (single-enabled-platform groups only)",
    defaultOptions: { prefix: "$" },
    toPreview: (o) => `${o.prefix ?? "$"}1.23`,
    fields: [
      { key: "prefix", label: "Prefix", type: "string", placeholder: "$" },
    ],
  },
  {
    type: "group-window-cost",
    name: "Window cost",
    icon: "bolt",
    desc: "Current group's estimated spend $ in this quota window (coding plan platforms accumulate est_cost since the window start; hidden when 0 or non-coding)",
    get defaultOptions() { return { prefix: PREFIX.windowCost() }; },
    toPreview: (o) => `${o.prefix ?? PREFIX.windowCost()}4.56`,
    fields: [
      { key: "prefix", label: "Prefix", type: "string", get placeholder() { return PREFIX.windowCost(); } },
    ],
  },
  {
    type: "group-route",
    name: "Group · Platform",
    icon: "bolt",
    desc: "Current group name / platform of the most recent successful hit (works for multi-platform groups too)",
    defaultOptions: {},
    toPreview: () => "glm/GLM-main",
    fields: [],
  },
  {
    type: "group-coding",
    name: "Coding Plan",
    icon: "permissions",
    desc: "Coding Plan tier utilization (single-enabled-platform groups only)",
    defaultOptions: { dynamicColor: false },
    toPreview: () => "5h 23%·7d 41%",
    fields: [
      { key: "dynamicColor", label: "Dynamic color (by pace)", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "group-requests",
    name: "Requests · Success",
    icon: "status",
    desc: "Group request count · success rate (single-enabled-platform groups only)",
    defaultOptions: {},
    toPreview: () => "128·99%",
    fields: [],
  },
  {
    type: "group-cache",
    name: "Cache Rate",
    icon: "status",
    desc: "Group cache hit rate (single-enabled-platform groups only)",
    get defaultOptions() { return { prefix: PREFIX.cache() }; },
    toPreview: (o) => `${o.prefix ?? PREFIX.cache()}37%`,
    fields: [
      { key: "prefix", label: "Prefix", type: "string", get placeholder() { return PREFIX.cache(); } },
    ],
  },
  {
    type: "group-tokens",
    name: "Total Tokens",
    icon: "core",
    desc: "Total tokens used by the current group (single-enabled-platform groups only)",
    defaultOptions: { prefix: "" },
    toPreview: (o) => `${o.prefix ?? ""}1.2M`,
    fields: [
      { key: "prefix", label: "Prefix", type: "string", placeholder: "" },
    ],
  },
  // ── Atomic segments: one per raw statusline input field ──
  // Cost / execution
  {
    type: "cost-usd",
    name: "Cost ($)",
    icon: "bolt",
    desc: "cost.total_cost_usd — estimated session cost",
    defaultOptions: { prefix: "$" },
    toPreview: (o) => `${o.prefix ?? "$"}0.12`,
    fields: [
      { key: "prefix", label: "Prefix", type: "string", placeholder: "$" },
    ],
  },
  {
    type: "session-duration",
    name: "Session Time",
    icon: "status",
    desc: "cost.total_duration_ms — total session duration",
    defaultOptions: { format: "human" },
    toPreview: (o) => o.format === "ms" ? "285000ms" : "4m45s",
    fields: [
      { key: "format", label: "Format", type: "select", options: ["human", "ms"] },
    ],
  },
  {
    type: "api-duration",
    name: "API Time",
    icon: "status",
    desc: "cost.total_api_duration_ms — API wait time",
    defaultOptions: { format: "human" },
    toPreview: (o) => o.format === "ms" ? "15300ms" : "15s",
    fields: [
      { key: "format", label: "Format", type: "select", options: ["human", "ms"] },
    ],
  },
  {
    type: "lines-changed",
    name: "Lines Changed",
    icon: "core",
    desc: "cost.total_lines_added / removed — lines added/removed",
    defaultOptions: {},
    toPreview: () => "+412 -87",
    fields: [],
  },
  // Context window
  {
    type: "context-tokens",
    name: "Context Tokens",
    icon: "core",
    desc: "context_window.total_input/output_tokens — input/output tokens",
    defaultOptions: { abbrev: true, mode: "split" },
    toPreview: (o) => o.mode === "sum"
      ? (o.abbrev ? "101.9K" : "101900")
      : (o.abbrev ? "89.5K/12.4K" : "89500/12400"),
    fields: [
      { key: "mode", label: "Mode", type: "select", options: ["split", "sum"] },
      { key: "abbrev", label: "Abbreviate (K/M)", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "context-max",
    name: "Context Max",
    icon: "status",
    desc: "context_window.context_window_size — max window size",
    defaultOptions: { abbrev: true },
    toPreview: (o) => o.abbrev ? "200K" : "200000",
    fields: [
      { key: "abbrev", label: "Abbreviate (K/M)", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "context-remaining",
    name: "Context Left",
    icon: "status",
    desc: "context_window.remaining_percentage — remaining percentage",
    defaultOptions: {},
    toPreview: () => "49%",
    fields: [],
  },
  {
    type: "context-cache",
    name: "Cache Tokens",
    icon: "core",
    desc: "current_usage.cache_creation/read_input_tokens — cache write/read",
    get defaultOptions() { return { abbrev: true, mode: "tokens", prefix: PREFIX.cache() }; },
    toPreview: (o) => o.mode === "hitrate"
      ? `${o.prefix ?? PREFIX.cache()}13.3578%`
      : (o.abbrev ? "w20K r12.1K" : "w20000 r12100"),
    fields: [
      { key: "mode", label: "Mode", type: "select", options: ["tokens", "hitrate"] },
      { key: "abbrev", label: "Abbreviate (K/M)", type: "select", options: ["true", "false"] },
      { key: "prefix", label: "Hit-rate prefix", type: "string", get placeholder() { return PREFIX.cache(); } },
    ],
  },
  // Rate limits (per window)
  {
    type: "rate-limit-5h",
    name: "Limit 5h",
    icon: "permissions",
    desc: "rate_limits.five_hour — 5-hour window usage",
    defaultOptions: { showReset: false },
    toPreview: (o) => o.showReset ? "5h:34% (128m)" : "5h:34%",
    fields: [
      { key: "showReset", label: "Show time until reset", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "rate-limit-7d",
    name: "Limit 7d",
    icon: "permissions",
    desc: "rate_limits.seven_day — 7-day window usage",
    defaultOptions: { showReset: false },
    toPreview: (o) => o.showReset ? "7d:62% (40h)" : "7d:62%",
    fields: [
      { key: "showReset", label: "Show time until reset", type: "select", options: ["false", "true"] },
    ],
  },
  // Git
  {
    type: "git-branch",
    name: "Git Branch",
    icon: "folder",
    desc: "Runs git branch --show-current in-script (empty when not a git repo / no branch)",
    defaultOptions: {},
    // cwd 取自 workspace.current_dir，回退 .cwd，再回退当前目录；非 git 仓库 / 游离 HEAD → 空输出降级。
    toPreview: () => "main",
    fields: [],
  },
  {
    type: "git-host",
    name: "Git Host",
    icon: "folder",
    desc: "workspace.repo.host — git repository host",
    defaultOptions: {},
    toPreview: () => "github.com",
    fields: [],
  },
  {
    type: "git-owner",
    name: "Git Owner",
    icon: "folder",
    desc: "workspace.repo.owner — repository owner",
    defaultOptions: {},
    toPreview: () => "anthropics",
    fields: [],
  },
  {
    type: "git-repo",
    name: "Git Repo",
    icon: "folder",
    desc: "workspace.repo.name — repository name",
    defaultOptions: {},
    toPreview: () => "claude-code",
    fields: [],
  },
  {
    type: "git-repo-full",
    name: "Git Full Name",
    icon: "folder",
    desc: "owner/name — full repository identifier",
    defaultOptions: {},
    toPreview: () => "anthropics/claude-code",
    fields: [],
  },
  {
    type: "git-worktree",
    name: "Git Worktree",
    icon: "folder",
    desc: "workspace.git_worktree — git worktree name",
    defaultOptions: {},
    toPreview: () => "feature-xyz",
    fields: [],
  },
  // Directory / session
  {
    type: "cwd",
    name: "Working Dir",
    icon: "folder",
    desc: "workspace.current_dir — current working directory",
    defaultOptions: { format: "basename" },
    toPreview: (o) => o.format === "full" ? "/Users/luoxin/persons/aidog" : "aidog",
    fields: [
      { key: "format", label: "Format", type: "select", options: ["basename", "full"] },
    ],
  },
  {
    type: "project-dir",
    name: "Project Dir",
    icon: "folder",
    desc: "workspace.project_dir — project launch directory",
    defaultOptions: { format: "basename" },
    toPreview: (o) => o.format === "full" ? "/Users/luoxin/persons/aidog" : "aidog",
    fields: [
      { key: "format", label: "Format", type: "select", options: ["basename", "full"] },
    ],
  },
  {
    type: "added-dirs",
    name: "Added Dirs",
    icon: "folder",
    desc: "workspace.added_dirs — directories added via /add-dir",
    defaultOptions: {},
    toPreview: () => "shared,web",
    fields: [],
  },
  {
    type: "session-id",
    name: "Session ID",
    icon: "core",
    desc: "session_id — session identifier",
    defaultOptions: { truncate: true },
    toPreview: (o) => o.truncate ? "abc123xy" : "abc123xyz789",
    fields: [
      { key: "truncate", label: "Truncate (first 8 chars)", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "session-name",
    name: "Session Name",
    icon: "core",
    desc: "session_name — custom session name (hidden if unset)",
    defaultOptions: {},
    toPreview: () => "statusline-atoms",
    fields: [],
  },
  {
    type: "transcript-path",
    name: "Transcript Path",
    icon: "folder",
    desc: "transcript_path — session transcript file",
    defaultOptions: { format: "basename" },
    toPreview: (o) => o.format === "full" ? "/Users/luoxin/.claude/session.jsonl" : "session.jsonl",
    fields: [
      { key: "format", label: "Format", type: "select", options: ["basename", "full"] },
    ],
  },
  // Worktree
  {
    type: "worktree-name",
    name: "Worktree Name",
    icon: "folder",
    desc: "worktree.name — worktree identifier",
    defaultOptions: {},
    toPreview: () => "feature-xyz",
    fields: [],
  },
  {
    type: "worktree-branch",
    name: "Worktree Branch",
    icon: "folder",
    desc: "worktree.branch — current working branch",
    defaultOptions: {},
    toPreview: () => "feat/atoms",
    fields: [],
  },
  {
    type: "worktree-original-branch",
    name: "Worktree Origin",
    icon: "folder",
    desc: "worktree.original_branch — branch before entering worktree",
    defaultOptions: {},
    toPreview: () => "main",
    fields: [],
  },
  // PR
  {
    type: "pr-number",
    name: "PR Number",
    icon: "status",
    desc: "pr.number — open PR number",
    defaultOptions: { prefix: "#" },
    toPreview: (o) => `${o.prefix ?? "#"}123`,
    fields: [
      { key: "prefix", label: "Prefix", type: "string", placeholder: "#" },
    ],
  },
  {
    type: "pr-url",
    name: "PR URL",
    icon: "status",
    desc: "pr.url — PR link",
    defaultOptions: {},
    toPreview: () => "https://github.com/o/r/pull/123",
    fields: [],
  },
  {
    type: "pr-state",
    name: "PR State",
    icon: "status",
    desc: "pr.review_state — PR review state",
    defaultOptions: {},
    toPreview: () => "approved",
    fields: [],
  },
  // Other single fields
  {
    type: "version",
    name: "CC Version",
    icon: "core",
    desc: "version — Claude Code version",
    defaultOptions: { prefix: "v" },
    toPreview: (o) => `${o.prefix ?? "v"}2.1.90`,
    fields: [
      { key: "prefix", label: "Prefix", type: "string", placeholder: "v" },
    ],
  },
  {
    type: "output-style",
    name: "Output Style",
    icon: "ui",
    desc: "output_style.name — current output style",
    defaultOptions: {},
    toPreview: () => "default",
    fields: [],
  },
  {
    type: "thinking",
    name: "Thinking",
    icon: "behavior",
    desc: "thinking.enabled — shown when extended thinking is on",
    defaultOptions: { label: "thinking" },
    toPreview: (o) => o.label ?? "thinking",
    fields: [
      { key: "label", label: "Text", type: "string", placeholder: "thinking" },
    ],
  },
  {
    type: "token-warn",
    name: "Token Warning",
    icon: "permissions",
    desc: "exceeds_200k_tokens — warns when exceeding 200k",
    defaultOptions: { label: "⚠200k" },
    toPreview: (o) => o.label ?? "⚠200k",
    fields: [
      { key: "label", label: "Text", type: "string", placeholder: "⚠200k" },
    ],
  },
  {
    type: "agent",
    name: "Agent Name",
    icon: "team",
    desc: "agent.name — agent name (hidden if unset)",
    defaultOptions: {},
    toPreview: () => "reviewer",
    fields: [],
  },
  {
    type: "agent-badge",
    name: "Subagent Badge",
    icon: "team",
    desc: "[type·status·model] — subagent task badge (hidden when type empty; status symbol/color dynamic)",
    defaultOptions: {},
    // Self-colors via embedded catppuccin truecolor (multi-color + dynamic
    // status色), so leave the segment `color` empty — do not wrap in fixedColorBash.
    toPreview: () => "[Agent·●·Opus]",
    fields: [],
  },
  {
    type: "spend-limit",
    name: "Spend Limit",
    icon: "permissions",
    desc: "rate_limits.spend_limit — gateway spend limit usage (hidden when unset)",
    defaultOptions: { showReset: false, showAmount: false },
    toPreview: (o) => `spend:63%${o.showReset ? " (2h)" : ""}${o.showAmount ? " $314.12/$500 monthly" : ""}`,
    fields: [
      { key: "showReset", label: "Show reset countdown", type: "select", options: ["true", "false"] },
      { key: "showAmount", label: "Show amount", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "prompt-cache",
    name: "Prompt Cache",
    icon: "status",
    desc: "prompt_cache — hit ratio / warm / expiry countdown / misses / cold rebuild / miss cause",
    get defaultOptions() { return { mode: "hitrate", prefix: PREFIX.cache() }; },
    toPreview: (o) => {
      const body: Record<string, string> = {
        hitrate: "91%", warm: "warm", ttl: "90m", misses: "2/14 miss",
        recache: "cold→45K", cause: "tools_changed",
      };
      return (o.prefix ?? PREFIX.cache()) + (body[o.mode ?? "hitrate"] ?? "91%");
    },
    fields: [
      { key: "mode", label: "Mode", type: "select", options: ["hitrate", "warm", "ttl", "misses", "recache", "cause"] },
      { key: "prefix", label: "Prefix", type: "string", get placeholder() { return PREFIX.cache(); } },
    ],
  },
  {
    type: "fast-mode",
    name: "Fast Mode",
    icon: "behavior",
    desc: "fast_mode — shown when fast mode is on",
    defaultOptions: { label: "fast" },
    toPreview: (o) => o.label ?? "fast",
    fields: [
      { key: "label", label: "Text", type: "string", placeholder: "fast" },
    ],
  },
  {
    type: "worktree-path",
    name: "Worktree Path",
    icon: "folder",
    desc: "worktree.path — worktree directory",
    defaultOptions: {},
    toPreview: () => "/tmp/wt/feature-xyz",
    fields: [],
  },
  {
    type: "worktree-original-cwd",
    name: "Worktree Origin Dir",
    icon: "folder",
    desc: "worktree.original_cwd — directory before entering worktree",
    defaultOptions: {},
    toPreview: () => "/tmp/proj",
    fields: [],
  },
  {
    type: "pr-kind",
    name: "PR Kind",
    icon: "status",
    desc: "pr.kind — mr means GitLab merge request (hidden for GitHub PRs)",
    defaultOptions: {},
    toPreview: () => "mr",
    fields: [],
  },
  {
    type: "agent-type",
    name: "Subagent Type",
    icon: "team",
    desc: "task.agentType — subagent type (e.g. Explore)",
    defaultOptions: {},
    toPreview: () => "Explore",
    fields: [],
  },
  {
    type: "task-description",
    name: "Subagent Description",
    icon: "team",
    desc: "task.description — subagent task description",
    defaultOptions: {},
    toPreview: () => "review code",
    fields: [],
  },
  {
    type: "task-effort",
    name: "Subagent Effort",
    icon: "team",
    desc: "task.effort — subagent reasoning effort (level or token budget)",
    defaultOptions: {},
    toPreview: () => "high",
    fields: [],
  },
  {
    type: "custom",
    name: "Custom",
    icon: "bolt",
    desc: "Custom jq expression",
    defaultOptions: { expr: ".model.display_name" },
    toPreview: (o) => `<${o.expr || ".model.display_name"}>`,
    fields: [
      { key: "expr", label: "jq Expression", type: "string", placeholder: ".model.display_name" },
    ],
  },
];

export const SEGMENT_DEF_MAP = new Map(SEGMENT_DEFS.map(d => [d.type, d]));

/**
 * Ordered segment categories for the add-segment picker. Each entry lists the
 * segment types under that group; the picker renders a labeled header per group.
 * i18n: `statusline.segCat.<id>`.
 */
export const SEGMENT_CATEGORIES: { id: string; label: string; types: SegmentType[] }[] = [
  { id: "common", label: "Common", types: ["model", "context-bar", "context-pct", "git", "cost", "rate-limits", "effort", "vim", "separator"] },
  { id: "cost", label: "Cost / Execution", types: ["cost-usd", "session-duration", "api-duration", "lines-changed"] },
  { id: "context", label: "Context", types: ["context-tokens", "context-max", "context-remaining", "context-cache", "prompt-cache"] },
  { id: "rate", label: "Rate limits", types: ["rate-limit-5h", "rate-limit-7d", "spend-limit"] },
  { id: "git", label: "Git", types: ["git-branch", "git-host", "git-owner", "git-repo", "git-repo-full", "git-worktree"] },
  { id: "session", label: "Directory / Session", types: ["cwd", "project-dir", "added-dirs", "session-id", "session-name", "transcript-path"] },
  { id: "worktree", label: "Worktree", types: ["worktree-name", "worktree-branch", "worktree-original-branch", "worktree-path", "worktree-original-cwd"] },
  { id: "pr", label: "Pull Request", types: ["pr-number", "pr-url", "pr-state", "pr-kind"] },
  { id: "other", label: "Other", types: ["version", "output-style", "thinking", "token-warn", "agent", "agent-badge", "agent-type", "task-description", "task-effort", "fast-mode", "custom"] },
];

/**
 * Built-in default 3-line layout (PRD). Applied only when no `segments` exist
 * (first run) or on explicit reset — existing user layouts are never overwritten.
 *
 * Separators are now explicit `separator` segments inserted between stable items
 * (`·` row1/3). Conditional separators that must vanish when their neighbour
 * degrades to empty (`[cost]·`, `·worktree`, `coding · `/`balance · ` group
 * segments, `|pwd`) stay on per-segment reserved affix options (`affixPre` /
 * `affixSuf`) so an empty body leaves no orphaned separator char.
 *
 * Colors are fixed hex per PRD: model 蓝 / tokens 紫 / cost 灰 / ctx·cache 绿 /
 * branch 黄 / version 灰. Row 3 coding/balance self-color dynamically (no fixed
 * `color`) via group*DynBash. Separator segments inherit no color (terminal default).
 */
export const DEFAULT_SEGMENTS: StatusLineSegment[] = [
  // ── Row 1: model · tokens[cost]·ctx%·缓存 X% ──
  { id: "d-model", type: "model", enabled: true, newline: false, color: "#4A9EFF",
    options: { format: "short" } },
  { id: "d-sep1", type: "separator", enabled: true, newline: false,
    options: { char: " · " } },
  { id: "d-tokens", type: "context-tokens", enabled: true, newline: false, color: "#BF5AF2",
    options: { mode: "sum", abbrev: true } },
  // cost hugs brackets and trails its own `·` so it disappears cleanly when empty.
  { id: "d-cost", type: "cost-usd", enabled: true, newline: false, color: "#8E8E93",
    options: { prefix: "$", affixPre: "[", affixSuf: "]·" } },
  { id: "d-ctx", type: "context-pct", enabled: true, newline: false, color: "#34C759",
    options: {} },
  { id: "d-cache", type: "context-cache", enabled: true, newline: false, color: "#34C759",
    options: { mode: "hitrate", affixPre: "·" } },
  // ── Row 2: branch[·worktree]|pwd ──
  { id: "d-branch", type: "git-branch", enabled: true, newline: true, color: "#FFD60A",
    options: {} },
  { id: "d-worktree", type: "worktree-name", enabled: true, newline: false,
    options: { affixPre: "·" } },
  { id: "d-cwd", type: "cwd", enabled: true, newline: false,
    options: { format: "full", affixPre: "|" } },
  // ── Row 3: coding-or-balance · version ──
  // version carries its own leading ` · ` affix; coding/balance are mutually
  // exclusive and concatenate directly. When both are empty the ` · ` still
  // prefixes version as a decorative bullet.
  { id: "d-coding", type: "group-coding", enabled: true, newline: true,
    options: { dynamicColor: true } },
  { id: "d-balance", type: "group-balance", enabled: true, newline: false,
    options: { dynamicColor: true, prefix: "$", affixPre: "·" } },
  { id: "d-wcost", type: "group-window-cost", enabled: true, newline: false,
    options: { prefix: "$", affixPre: "·" } },
  { id: "d-route", type: "group-route", enabled: true, newline: false,
    options: { affixPre: "·" } },
  { id: "d-version", type: "version", enabled: true, newline: false, color: "#8E8E93",
    options: { prefix: "v", affixPre: " · " } },
];

/**
 * Built-in default SubagentStatusLine layout. Subagent now shares the exact same
 * segment editor as the main statusline (no templates) — this is its first-run /
 * reset default. Renders a single line:
 *
 *   [<type>·<状态符号>·<model>]<子代理名>·<ctx%>·<tokens>·<时长>
 *   e.g. [Agent·●·Opus]reviewer·48%·96K·6m40s
 *
 * The leading badge is now the dynamic `agent-badge` segment (移植自 ccplugin
 * subagent_statusline.py)：type_label(`local_agent→Agent`) + status 符号/色(`_STATUS_MAP`)
 * + model(task 级优先回退顶层)；`.type` 为空时整段隐藏。它取代了旧的字面量
 * `[Agent·●]` 分隔符，故首段随 type/status/model 变化而非恒定。徽章自带 catppuccin
 * 颜色（无 `color` 字段）。其后 name 仍走 `.agent.name → .session_name → "subagent"`
 * 兜底，剩余指标 `·`-分隔且字段缺失时降级为空。
 */
export const DEFAULT_SUBAGENT_SEGMENTS: StatusLineSegment[] = [
  { id: "sa-badge", type: "agent-badge", enabled: true, newline: false,
    options: {} },
  { id: "sa-name", type: "custom", enabled: true, newline: false, color: "#4A9EFF",
    options: { expr: ".label // .name // .id // \"?\"" } },
  // Metric segments carry their own leading `·` via `affixPre` instead of
  // standalone `separator` segments: an empty metric (no token / no duration /
  // no ctx) then degrades to nothing AND drops its separator — matching
  // ccplugin subagent_statusline.py which omits zero/absent metrics rather than
  // emitting `0%` / `0` with orphaned `·` separators.
  { id: "sa-ctx", type: "context-pct", enabled: true, newline: false, color: "#34C759",
    options: { suffix: "%", degradeZero: true, affixPre: "·" } },
  { id: "sa-tokens", type: "context-tokens", enabled: true, newline: false, color: "#BF5AF2",
    options: { mode: "sum", abbrev: true, affixPre: "·" } },
  { id: "sa-dur", type: "session-duration", enabled: true, newline: false, color: "#8E8E93",
    options: { format: "human", affixPre: "·" } },
];

// ── Available data fields reference ──

export const STATUSLINE_DATA_FIELDS = [
  { id: "model", group: "Model", fields: [
    { key: "model.id", desc: "Model identifier" },
    { key: "model.display_name", desc: "Model display name" },
  ]},
  { id: "workspace", group: "Workspace", fields: [
    { key: "workspace.current_dir", desc: "Current working directory" },
    { key: "workspace.project_dir", desc: "Project launch directory" },
    { key: "workspace.repo.owner/name", desc: "Git repository identifier" },
  ]},
  { id: "cost", group: "Cost", fields: [
    { key: "cost.total_cost_usd", desc: "Cumulative estimated cost ($)" },
    { key: "cost.total_duration_ms", desc: "Total duration (ms)" },
    { key: "cost.total_api_duration_ms", desc: "API wait time (ms)" },
  ]},
  { id: "contextWindow", group: "Context Window", fields: [
    { key: "context_window.used_percentage", desc: "Used percentage" },
    { key: "context_window.context_window_size", desc: "Maximum window size" },
  ]},
  { id: "rateLimits", group: "Rate Limits", fields: [
    { key: "rate_limits.five_hour.used_percentage", desc: "5-hour window usage %" },
    { key: "rate_limits.seven_day.used_percentage", desc: "7-day window usage %" },
  ]},
  { id: "other", group: "Other", fields: [
    { key: "effort.level", desc: "Reasoning effort" },
    { key: "vim.mode", desc: "Vim mode" },
    { key: "session_id", desc: "Session ID" },
    { key: "version", desc: "Claude Code version" },
  ]},
  { id: "subagent", group: "Subagent Tasks", fields: [
    { key: "type", desc: "Task type (e.g. local_agent; badge hidden when empty)" },
    { key: "status", desc: "Task status (running/pending/completed/failed/cancelled)" },
    { key: "agent.name", desc: "Subagent name" },
  ]},
];
