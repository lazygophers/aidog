// ─── Statusline script generator (pure, no React) ──────────
// Extracted from editors.tsx: segment model + bash→Python script generation.
// Pure functions only — no React/Tauri imports — so a Node test harness can
// import these to build golden-output regression fixtures.

/** A single display segment in the statusline */
export type RowAlign = "left" | "center" | "right";

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

/** Segment types whose value can drive automatic semantic coloring. */
export const VALUE_COLORABLE: Set<SegmentType> = new Set([
  "context-pct", "context-bar", "cost", "rate-limits",
  // Atomic value-class segments
  "cost-usd", "context-remaining", "rate-limit-5h", "rate-limit-7d",
  "session-duration", "api-duration",
]);

/** Parse "#RRGGBB" / "#RGB" → [r,g,b] (0–255) or null when invalid. */
export function hexToRgb(hex?: string): [number, number, number] | null {
  if (!hex) return null;
  let h = hex.trim().replace(/^#/, "");
  if (h.length === 3) h = h.split("").map(c => c + c).join("");
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return null;
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

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
  // PR
  | "pr-number"              // pr.number
  | "pr-url"                 // pr.url
  | "pr-state"               // pr.review_state
  // Other single fields
  | "version"                // version
  | "output-style"           // output_style.name
  | "thinking"               // thinking.enabled
  | "token-warn"             // exceeds_200k_tokens
  | "agent"                  // agent.name
  | "agent-badge"            // 子代理徽章 [type·status·model]（动态符号/色）
  // aidog group segments
  | "group-balance"  // aidog group: 预估余额
  | "group-spent"    // aidog group: 累计预估花费
  | "group-coding"   // aidog group: coding plan 利用率
  | "group-requests" // aidog group: 请求数 · 成功率
  | "group-cache"    // aidog group: 缓存命中率
  | "group-tokens"   // aidog group: 已使用总 tokens
  | "custom";        // Custom jq expression

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

/** Segment types that consume the shared aidog group-info endpoint. */
export const GROUP_SEG_TYPES = new Set<SegmentType>([
  "group-balance", "group-spent", "group-coding",
  "group-requests", "group-cache", "group-tokens",
]);


export const SEGMENT_DEFS: SegmentDef[] = [
  {
    type: "model",
    name: "模型名称",
    icon: "core",
    desc: "当前模型显示名称",
    defaultOptions: { format: "short" },
    toPreview: (o) => o.format === "full" ? "claude-sonnet-4-6" : "Opus",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["short", "full"] },
    ],
  },
  {
    type: "context-bar",
    name: "上下文进度条",
    icon: "status",
    desc: "10 字符进度条 + 百分比",
    defaultOptions: { width: 10, filled: "▓", empty: "░" },
    toPreview: (o) => {
      const w = o.width || 10;
      const pct = 65;
      const filled = Math.round(pct * w / 100);
      return (o.filled || "▓").repeat(filled) + (o.empty || "░").repeat(w - filled) + ` ${pct}%`;
    },
    fields: [
      { key: "width", label: "宽度", type: "number", placeholder: "10" },
      { key: "filled", label: "填充字符", type: "string", placeholder: "▓" },
      { key: "empty", label: "空字符", type: "string", placeholder: "░" },
    ],
  },
  {
    type: "context-pct",
    name: "上下文百分比",
    icon: "status",
    desc: "仅百分比数字",
    defaultOptions: { suffix: "%" },
    // `degradeZero` (subagent default): emit nothing when ctx% is absent/0 so the
    // `affixPre` separator also drops — mirrors ccplugin which omits ctx for tasks
    // with no real context data. Main statusline omits the flag → always `0%`.
    toPreview: () => "65%",
    fields: [],
  },
  {
    type: "git",
    name: "Git 状态",
    icon: "folder",
    desc: "分支名 + 仓库名",
    defaultOptions: { showRepo: false },
    toPreview: (o) => o.showRepo ? "anthropics/claude-code" : "claude-code",
    fields: [
      { key: "showRepo", label: "显示完整路径 (owner/name)", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "cost",
    name: "成本追踪",
    icon: "bolt",
    desc: "API 成本 + 持续时间",
    defaultOptions: { showDuration: true },
    toPreview: (o) => o.showDuration ? "$0.12 · 155s" : "$0.12",
    fields: [
      { key: "showDuration", label: "显示持续时间", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "rate-limits",
    name: "速率限制",
    icon: "permissions",
    desc: "5h / 7d 限制使用百分比",
    defaultOptions: { windows: "both" },
    toPreview: (o) => o.windows === "5h" ? "5h:23%" : o.windows === "7d" ? "7d:41%" : "5h:23% 7d:41%",
    fields: [
      { key: "windows", label: "窗口", type: "select", options: ["both", "5h", "7d"] },
    ],
  },
  {
    type: "effort",
    name: "Effort Level",
    icon: "behavior",
    desc: "推理工作量等级",
    defaultOptions: {},
    toPreview: () => "high",
    fields: [],
  },
  {
    type: "vim",
    name: "Vim 模式",
    icon: "ui",
    desc: "当前 vim 模式",
    defaultOptions: {},
    toPreview: () => "NORMAL",
    fields: [],
  },
  {
    type: "separator",
    name: "分隔符",
    icon: "advanced",
    desc: "视觉分隔符（可插入到任意段之间）",
    defaultOptions: { char: "·" },
    toPreview: (o) => (typeof o.char === "string" ? o.char : "·"),
    fields: [
      { key: "char", label: "分隔符字符", type: "string", placeholder: "·" },
    ],
  },
  {
    type: "group-balance",
    name: "分组余额",
    icon: "bolt",
    desc: "当前分组单平台预估剩余余额（动态色：<1天红 / <3天黄 / 否则绿）",
    defaultOptions: { prefix: "余额 ", dynamicColor: false },
    toPreview: (o) => `${o.prefix ?? "余额 "}48.20`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "余额 " },
      { key: "dynamicColor", label: "动态色 (按可用天数)", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "group-spent",
    name: "分组花费",
    icon: "bolt",
    desc: "当前分组累计预估花费（仅单平台分组）",
    defaultOptions: { prefix: "$" },
    toPreview: (o) => `${o.prefix ?? "$"}1.23`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "$" },
    ],
  },
  {
    type: "group-coding",
    name: "Coding Plan",
    icon: "permissions",
    desc: "Coding Plan 各档利用率（动态色：fast红 / normal黄 / busy绿，红时显重置）",
    defaultOptions: { dynamicColor: false },
    toPreview: () => "5h 23%·7d 41%",
    fields: [
      { key: "dynamicColor", label: "动态色 (按 pace)", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "group-requests",
    name: "请求·成功率",
    icon: "status",
    desc: "当前分组请求数 · 成功率（仅单平台分组）",
    defaultOptions: {},
    toPreview: () => "128·99%",
    fields: [],
  },
  {
    type: "group-cache",
    name: "缓存率",
    icon: "status",
    desc: "当前分组缓存命中率（仅单平台分组）",
    defaultOptions: { prefix: "缓存 " },
    toPreview: (o) => `${o.prefix ?? "缓存 "}37%`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "缓存 " },
    ],
  },
  {
    type: "group-tokens",
    name: "总 Tokens",
    icon: "core",
    desc: "当前分组已使用总 tokens（仅单平台分组）",
    defaultOptions: { prefix: "" },
    toPreview: (o) => `${o.prefix ?? ""}1.2M`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "" },
    ],
  },
  // ── Atomic segments: one per raw statusline input field ──
  // Cost / execution
  {
    type: "cost-usd",
    name: "成本 ($)",
    icon: "bolt",
    desc: "cost.total_cost_usd — 累计预估成本",
    defaultOptions: { prefix: "$" },
    toPreview: (o) => `${o.prefix ?? "$"}0.12`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "$" },
    ],
  },
  {
    type: "session-duration",
    name: "会话耗时",
    icon: "status",
    desc: "cost.total_duration_ms — 会话总耗时",
    defaultOptions: { format: "human" },
    toPreview: (o) => o.format === "ms" ? "285000ms" : "4m45s",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["human", "ms"] },
    ],
  },
  {
    type: "api-duration",
    name: "API 耗时",
    icon: "status",
    desc: "cost.total_api_duration_ms — API 等待时间",
    defaultOptions: { format: "human" },
    toPreview: (o) => o.format === "ms" ? "15300ms" : "15s",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["human", "ms"] },
    ],
  },
  {
    type: "lines-changed",
    name: "代码变更",
    icon: "core",
    desc: "cost.total_lines_added / removed — 新增/删除行",
    defaultOptions: {},
    toPreview: () => "+412 -87",
    fields: [],
  },
  // Context window
  {
    type: "context-tokens",
    name: "上下文 Tokens",
    icon: "core",
    desc: "输入/输出 token，或 session 合计（total_input + total_output）",
    defaultOptions: { abbrev: true, mode: "split" },
    toPreview: (o) => o.mode === "sum"
      ? (o.abbrev ? "101.9K" : "101900")
      : (o.abbrev ? "89.5K/12.4K" : "89500/12400"),
    fields: [
      { key: "mode", label: "模式", type: "select", options: ["split", "sum"] },
      { key: "abbrev", label: "缩写 (K/M)", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "context-max",
    name: "上下文容量",
    icon: "status",
    desc: "context_window.context_window_size — 最大窗口",
    defaultOptions: { abbrev: true },
    toPreview: (o) => o.abbrev ? "200K" : "200000",
    fields: [
      { key: "abbrev", label: "缩写 (K/M)", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "context-remaining",
    name: "上下文剩余",
    icon: "status",
    desc: "context_window.remaining_percentage — 剩余百分比",
    defaultOptions: {},
    toPreview: () => "49%",
    fields: [],
  },
  {
    type: "context-cache",
    name: "缓存率",
    icon: "core",
    desc: "缓存写入/读取 token，或缓存命中率 %（≤4 位小数）",
    defaultOptions: { abbrev: true, mode: "tokens", prefix: "缓存 " },
    toPreview: (o) => o.mode === "hitrate"
      ? `${o.prefix ?? "缓存 "}13.3578%`
      : (o.abbrev ? "w20K r12.1K" : "w20000 r12100"),
    fields: [
      { key: "mode", label: "模式", type: "select", options: ["tokens", "hitrate"] },
      { key: "abbrev", label: "缩写 (K/M)", type: "select", options: ["true", "false"] },
      { key: "prefix", label: "命中率前缀", type: "string", placeholder: "缓存 " },
    ],
  },
  // Rate limits (per window)
  {
    type: "rate-limit-5h",
    name: "限制 5h",
    icon: "permissions",
    desc: "rate_limits.five_hour — 5 小时窗口使用率",
    defaultOptions: { showReset: false },
    toPreview: (o) => o.showReset ? "5h:34% (128m)" : "5h:34%",
    fields: [
      { key: "showReset", label: "显示剩余重置时间", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "rate-limit-7d",
    name: "限制 7d",
    icon: "permissions",
    desc: "rate_limits.seven_day — 7 天窗口使用率",
    defaultOptions: { showReset: false },
    toPreview: (o) => o.showReset ? "7d:62% (40h)" : "7d:62%",
    fields: [
      { key: "showReset", label: "显示剩余重置时间", type: "select", options: ["false", "true"] },
    ],
  },
  // Git
  {
    type: "git-branch",
    name: "Git 分支",
    icon: "folder",
    desc: "脚本内 git branch --show-current（非 git / 无分支降级空）",
    defaultOptions: {},
    // cwd 取自 workspace.current_dir，回退 .cwd，再回退当前目录；非 git 仓库 / 游离 HEAD → 空输出降级。
    toPreview: () => "main",
    fields: [],
  },
  {
    type: "git-host",
    name: "Git 主机",
    icon: "folder",
    desc: "workspace.repo.host — Git 仓库主机",
    defaultOptions: {},
    toPreview: () => "github.com",
    fields: [],
  },
  {
    type: "git-owner",
    name: "Git 所有者",
    icon: "folder",
    desc: "workspace.repo.owner — 仓库所有者",
    defaultOptions: {},
    toPreview: () => "anthropics",
    fields: [],
  },
  {
    type: "git-repo",
    name: "Git 仓库",
    icon: "folder",
    desc: "workspace.repo.name — 仓库名",
    defaultOptions: {},
    toPreview: () => "claude-code",
    fields: [],
  },
  {
    type: "git-repo-full",
    name: "Git 全名",
    icon: "folder",
    desc: "owner/name — 仓库完整标识",
    defaultOptions: {},
    toPreview: () => "anthropics/claude-code",
    fields: [],
  },
  {
    type: "git-worktree",
    name: "Git Worktree",
    icon: "folder",
    desc: "workspace.git_worktree — Git worktree 名称",
    defaultOptions: {},
    toPreview: () => "feature-xyz",
    fields: [],
  },
  // Directory / session
  {
    type: "cwd",
    name: "工作目录",
    icon: "folder",
    desc: "workspace.current_dir — 当前工作目录",
    defaultOptions: { format: "basename" },
    toPreview: (o) => o.format === "full" ? "/Users/luoxin/persons/aidog" : "aidog",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["basename", "full"] },
    ],
  },
  {
    type: "project-dir",
    name: "项目目录",
    icon: "folder",
    desc: "workspace.project_dir — 项目启动目录",
    defaultOptions: { format: "basename" },
    toPreview: (o) => o.format === "full" ? "/Users/luoxin/persons/aidog" : "aidog",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["basename", "full"] },
    ],
  },
  {
    type: "added-dirs",
    name: "附加目录",
    icon: "folder",
    desc: "workspace.added_dirs — /add-dir 添加的目录",
    defaultOptions: {},
    toPreview: () => "shared,web",
    fields: [],
  },
  {
    type: "session-id",
    name: "会话 ID",
    icon: "core",
    desc: "session_id — 会话标识符",
    defaultOptions: { truncate: true },
    toPreview: (o) => o.truncate ? "abc123xy" : "abc123xyz789",
    fields: [
      { key: "truncate", label: "截断 (前8位)", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "session-name",
    name: "会话名称",
    icon: "core",
    desc: "session_name — 自定义会话名（未设置时隐藏）",
    defaultOptions: {},
    toPreview: () => "statusline-atoms",
    fields: [],
  },
  {
    type: "transcript-path",
    name: "记录路径",
    icon: "folder",
    desc: "transcript_path — 会话记录文件",
    defaultOptions: { format: "basename" },
    toPreview: (o) => o.format === "full" ? "/Users/luoxin/.claude/session.jsonl" : "session.jsonl",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["basename", "full"] },
    ],
  },
  // Worktree
  {
    type: "worktree-name",
    name: "Worktree 名",
    icon: "folder",
    desc: "worktree.name — Worktree 标识",
    defaultOptions: {},
    toPreview: () => "feature-xyz",
    fields: [],
  },
  {
    type: "worktree-branch",
    name: "Worktree 分支",
    icon: "folder",
    desc: "worktree.branch — 当前工作分支",
    defaultOptions: {},
    toPreview: () => "feat/atoms",
    fields: [],
  },
  {
    type: "worktree-original-branch",
    name: "Worktree 源分支",
    icon: "folder",
    desc: "worktree.original_branch — 回源分支",
    defaultOptions: {},
    toPreview: () => "main",
    fields: [],
  },
  // PR
  {
    type: "pr-number",
    name: "PR 编号",
    icon: "status",
    desc: "pr.number — 开放 PR 编号",
    defaultOptions: { prefix: "#" },
    toPreview: (o) => `${o.prefix ?? "#"}123`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "#" },
    ],
  },
  {
    type: "pr-url",
    name: "PR 链接",
    icon: "status",
    desc: "pr.url — PR 链接",
    defaultOptions: {},
    toPreview: () => "https://github.com/o/r/pull/123",
    fields: [],
  },
  {
    type: "pr-state",
    name: "PR 状态",
    icon: "status",
    desc: "pr.review_state — PR 审查状态",
    defaultOptions: {},
    toPreview: () => "approved",
    fields: [],
  },
  // Other single fields
  {
    type: "version",
    name: "CC 版本",
    icon: "core",
    desc: "version — Claude Code 版本",
    defaultOptions: { prefix: "v" },
    toPreview: (o) => `${o.prefix ?? "v"}2.1.90`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "v" },
    ],
  },
  {
    type: "output-style",
    name: "输出风格",
    icon: "ui",
    desc: "output_style.name — 当前输出风格",
    defaultOptions: {},
    toPreview: () => "default",
    fields: [],
  },
  {
    type: "thinking",
    name: "思考模式",
    icon: "behavior",
    desc: "thinking.enabled — 扩展思考开启时显示",
    defaultOptions: { label: "thinking" },
    toPreview: (o) => o.label ?? "thinking",
    fields: [
      { key: "label", label: "文案", type: "string", placeholder: "thinking" },
    ],
  },
  {
    type: "token-warn",
    name: "Token 警示",
    icon: "permissions",
    desc: "exceeds_200k_tokens — 超 200k 时警示",
    defaultOptions: { label: "⚠200k" },
    toPreview: (o) => o.label ?? "⚠200k",
    fields: [
      { key: "label", label: "文案", type: "string", placeholder: "⚠200k" },
    ],
  },
  {
    type: "agent",
    name: "Agent 名称",
    icon: "team",
    desc: "agent.name — agent 名称（未配置时隐藏）",
    defaultOptions: {},
    toPreview: () => "reviewer",
    fields: [],
  },
  {
    type: "agent-badge",
    name: "子代理徽章",
    icon: "team",
    desc: "[type·状态·模型] — 子代理任务徽章（type 空时隐藏，状态符号/色动态）",
    defaultOptions: {},
    // Self-colors via embedded catppuccin truecolor (multi-color + dynamic
    // status色), so leave the segment `color` empty — do not wrap in fixedColorBash.
    toPreview: () => "[Agent·●·Opus]",
    fields: [],
  },
  {
    type: "custom",
    name: "自定义",
    icon: "bolt",
    desc: "自定义 jq 表达式",
    defaultOptions: { expr: ".model.display_name" },
    toPreview: (o) => `<${o.expr || ".model.display_name"}>`,
    fields: [
      { key: "expr", label: "jq 表达式", type: "string", placeholder: ".model.display_name" },
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
  { id: "common", label: "常用", types: ["model", "context-bar", "context-pct", "git", "cost", "rate-limits", "effort", "vim", "separator"] },
  { id: "cost", label: "成本 / 执行", types: ["cost-usd", "session-duration", "api-duration", "lines-changed"] },
  { id: "context", label: "上下文", types: ["context-tokens", "context-max", "context-remaining", "context-cache"] },
  { id: "rate", label: "速率限制", types: ["rate-limit-5h", "rate-limit-7d"] },
  { id: "git", label: "Git", types: ["git-branch", "git-host", "git-owner", "git-repo", "git-repo-full", "git-worktree"] },
  { id: "session", label: "目录 / 会话", types: ["cwd", "project-dir", "added-dirs", "session-id", "session-name", "transcript-path"] },
  { id: "worktree", label: "Worktree", types: ["worktree-name", "worktree-branch", "worktree-original-branch"] },
  { id: "pr", label: "Pull Request", types: ["pr-number", "pr-url", "pr-state"] },
  { id: "other", label: "其他", types: ["version", "output-style", "thinking", "token-warn", "agent", "agent-badge", "custom"] },
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
    options: { mode: "hitrate", prefix: "缓存 ", affixPre: "·" } },
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
  { id: "model", group: "模型", fields: [
    { key: "model.id", desc: "模型标识符" },
    { key: "model.display_name", desc: "模型显示名称" },
  ]},
  { id: "workspace", group: "工作区", fields: [
    { key: "workspace.current_dir", desc: "当前工作目录" },
    { key: "workspace.project_dir", desc: "项目启动目录" },
    { key: "workspace.repo.owner/name", desc: "Git 仓库标识" },
  ]},
  { id: "cost", group: "成本", fields: [
    { key: "cost.total_cost_usd", desc: "累计预估成本 ($)" },
    { key: "cost.total_duration_ms", desc: "总持续时间 (ms)" },
    { key: "cost.total_api_duration_ms", desc: "API 等待时间 (ms)" },
  ]},
  { id: "contextWindow", group: "上下文窗口", fields: [
    { key: "context_window.used_percentage", desc: "已使用百分比" },
    { key: "context_window.context_window_size", desc: "最大窗口大小" },
  ]},
  { id: "rateLimits", group: "速率限制", fields: [
    { key: "rate_limits.five_hour.used_percentage", desc: "5小时窗口使用 %" },
    { key: "rate_limits.seven_day.used_percentage", desc: "7天窗口使用 %" },
  ]},
  { id: "other", group: "其他", fields: [
    { key: "effort.level", desc: "推理工作量" },
    { key: "vim.mode", desc: "Vim 模式" },
    { key: "session_id", desc: "会话 ID" },
    { key: "version", desc: "Claude Code 版本" },
  ]},
  { id: "subagent", group: "子代理任务", fields: [
    { key: "type", desc: "任务类型（如 local_agent；空则隐藏徽章）" },
    { key: "status", desc: "任务状态（running/pending/completed/failed/cancelled）" },
    { key: "agent.name", desc: "子代理名称" },
  ]},
];

// ── Script generation from segments ──

/** Group active segments into rows (split on `newline`). Returns rows with align. */
export function groupRows(segments: StatusLineSegment[]): { align: RowAlign; segs: StatusLineSegment[] }[] {
  const rows: { align: RowAlign; segs: StatusLineSegment[] }[] = [];
  let cur: StatusLineSegment[] | null = null;
  for (const seg of segments) {
    if (cur === null || (seg.newline && cur.length > 0)) {
      cur = [];
      rows.push({ align: seg.align ?? "left", segs: cur });
    }
    cur.push(seg);
  }
  return rows;
}

/**
 * Re-derive `newline` flags so the row model stays self-consistent after any
 * structural mutation (drag-reorder, delete, enable-toggle).
 *
 * The row model is *derived* from `newline`: a row break is any segment with
 * `newline === true`, plus the implicit break before the first segment. Drag
 * reordering moves items in the flat array without touching `newline`, which can
 * leave the new first segment carrying `newline: true` (a redundant leading
 * break) or strand a row break inside the array in a way that silently merges
 * rows. Both make "this row" ambiguous and break per-row delete.
 *
 * Invariant enforced here: the first segment never carries `newline: true`
 * (its row break is implicit). All other `newline` flags are preserved, so the
 * visible row count and membership are stable across reorders.
 */
export function normalizeSegments(segments: StatusLineSegment[]): StatusLineSegment[] {
  if (segments.length === 0) return segments;
  return segments.map((s, i) =>
    i === 0 ? (s.newline ? { ...s, newline: false } : s) : s,
  );
}

/** True when the segment starts a row (first active segment, or newline=true). */
export function isRowLeaderSeg(segments: StatusLineSegment[], id: string): boolean {
  const active = segments.filter(s => s.enabled);
  const idx = active.findIndex(s => s.id === id);
  if (idx < 0) {
    // disabled segment — leads if it has explicit newline
    return !!segments.find(s => s.id === id)?.newline;
  }
  return idx === 0 || !!active[idx].newline;
}

// ── Python script generation from segments ──
//
// The generated statusline / subagent scripts are Python (PEP723, stdlib only)
// executed by `uv run --script` or `python3` (see ScriptInvoker). Output is
// byte-for-byte identical to the former jq/printf/awk/sed bash implementation,
// guaranteed by the golden-output regression in scripts/statusline-golden/.
//
// Each script embeds the shared rendering engine (ENGINE_PY, auto-generated from
// scripts/statusline-golden/engine.py) plus a JSON segment config and a small
// entry point. The engine reads the config and renders at runtime, so all the
// number-formatting / ANSI / truncation parity logic lives in one place.

import { ENGINE_PY } from "./statusline-runtime";

/** PEP723 inline-metadata header (stdlib only; shebang is the python3 fallback). */
const PEP723_HEADER =
  `#!/usr/bin/env python3\n` +
  `# /// script\n` +
  `# requires-python = ">=3.8"\n` +
  `# dependencies = []\n` +
  `# ///\n`;

/** A serializable per-segment spec consumed by the runtime engine. */
interface SegmentSpec {
  type: SegmentType;
  opts: Record<string, any>;
  /** Fixed-color "r;g;b" triple, or null. */
  rgb: string | null;
  /** Value-driven auto color. */
  autoColor: boolean;
}

/**
 * Base64-encode a (possibly non-ASCII) JSON config string for safe embedding in
 * the generated Python source — avoids all quoting hazards (the config carries
 * arbitrary user prefixes / separators incl. quotes, backslashes, multibyte).
 * The script decodes it via `base64.b64decode(...).decode("utf-8")`.
 */
function b64(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  // btoa exists in the Tauri webview; Node test harness provides it too (>=16).
  return btoa(bin);
}

/** Resolve a segment's runtime spec (merged opts + color/autoColor). */
function segSpec(seg: StatusLineSegment): SegmentSpec {
  const def = SEGMENT_DEF_MAP.get(seg.type);
  const opts = { ...(def?.defaultOptions ?? {}), ...seg.options };
  const useAuto = !!seg.autoColor && VALUE_COLORABLE.has(seg.type);
  const rgbArr = useAuto ? null : hexToRgb(seg.color);
  return {
    type: seg.type,
    opts,
    rgb: rgbArr ? rgbArr.join(";") : null,
    autoColor: useAuto,
  };
}

/**
 * Generate the main statusLine Python script. Output (stdout) is byte-identical
 * to the former bash generator for every fixture in the golden regression set.
 */
export function generateStatusLineScript(segments: StatusLineSegment[]): string {
  const active = segments.filter(s => s.enabled);
  if (active.length === 0) {
    return PEP723_HEADER + "print('')\n";
  }
  const rows = groupRows(active).map(r => ({
    align: r.align,
    segs: r.segs.map(segSpec),
  }));
  const needGroup = active.some(s => GROUP_SEG_TYPES.has(s.type));
  const config = b64(JSON.stringify(rows));
  return [
    PEP723_HEADER,
    "# Generated by aidog — do not edit manually",
    ENGINE_PY,
    "",
    "import base64",
    `ROWS = json.loads(base64.b64decode("${config}").decode("utf-8"))`,
    `NEED_GROUP = ${needGroup ? "True" : "False"}`,
    "",
    "def main():",
    "    payload = json.loads(sys.stdin.read() or '{}')",
    "    gi = fetch_group_info() if NEED_GROUP else None",
    "    for line in render(payload, ROWS, gi):",
    "        sys.stdout.write(line + '\\n')",
    "",
    "main()",
    "",
  ].join("\n");
}

/**
 * Generate the SubagentStatusLine Python script — one JSONL `{"id","content"}`
 * line per task. Byte-identical to the former bash subagent generator.
 */
export function generateSubagentStatusLineScript(segments: StatusLineSegment[]): string {
  const active = segments.filter(s => s.enabled);
  if (active.length === 0) {
    return PEP723_HEADER + "pass\n";
  }
  // Subagent rows are single-line; all active segments render on one row.
  const config = b64(JSON.stringify(active.map(segSpec)));
  return [
    PEP723_HEADER,
    "# Generated by aidog — do not edit manually (SubagentStatusLine)",
    ENGINE_PY,
    "",
    "import base64",
    `SEGS = json.loads(base64.b64decode("${config}").decode("utf-8"))`,
    "",
    "def main():",
    "    payload = json.loads(sys.stdin.read() or '{}')",
    "    now = _now_epoch()",
    "    for line in render_subagent(payload, SEGS, now):",
    "        sys.stdout.write(line + '\\n')",
    "",
    "main()",
    "",
  ].join("\n");
}

/**
 * Resolved materialization for a statusLine / subagentStatusLine config block.
 * `scriptContent` is the bash script body to write (builtin mode) or `null`
 * (custom mode / disabled — nothing to generate). `customCommand` is the
 * user-supplied native command (custom mode only). `padding` is carried so the
 * caller can assemble the native field.
 */
export interface StatuslineMaterialization {
  enabled: boolean;
  mode: "builtin" | "custom";
  scriptContent: string | null;
  customCommand: string;
}

/**
 * Pure resolver: given a stored `_aidog_statusline` / `_aidog_subagent_statusline`
 * block and its scriptType, derive everything needed to materialize the native
 * `statusLine` / `subagentStatusLine` field — applying all default logic
 * (segments → DEFAULT_SEGMENTS, subagent template selection)
 * in one authoritative place. No side effects; the caller persists the result.
 *
 * Mirrors StatusLinePanel's in-component derivations so the on-save materializer
 * and the live UI agree byte-for-byte.
 */
export function materializeStatusline(
  stored: Record<string, any> | undefined,
  scriptType: "statusline" | "subagent",
): StatuslineMaterialization {
  const s = (stored ?? {}) as Record<string, any>;
  const isMain = scriptType === "statusline";
  const enabled = !!s.enabled;
  const mode: "builtin" | "custom" = s.mode === "custom" ? "custom" : "builtin";
    const customCommand = typeof s.customCommand === "string" ? s.customCommand : "";

  let scriptContent: string | null = null;
  if (enabled && mode === "builtin") {
    if (!isMain) {
      // Subagent statusline — native bash generator emitting per-task JSONL
      //   stdin:  {tasks: [{id, name, type, status, …}]}
      //   stdout: {"id":"…","content":"…"} per task
      // (no external dependency; the old python delegation was a non-distributable
      //  dev-machine path).
      const segments: StatusLineSegment[] =
        (s.segments as StatusLineSegment[] | undefined) ?? DEFAULT_SUBAGENT_SEGMENTS.map(seg => ({ ...seg }));
      return {
        enabled: true,
        mode: "builtin",
        scriptContent: generateSubagentStatusLineScript(segments),
        customCommand,
      };
    }
    // main statusline — segment-based bash generator.
    const segments: StatusLineSegment[] =
      (s.segments as StatusLineSegment[] | undefined) ?? DEFAULT_SEGMENTS.map(seg => ({ ...seg }));
    scriptContent = generateStatusLineScript(segments);
  }

  return { enabled, mode, scriptContent, customCommand };
}

/** Mock metric values used to drive autoColor preview (matches bash thresholds). */
export const PREVIEW_METRIC: Record<string, number> = {
  "context-pct": 65,
  "context-bar": 65,
  "cost": 12,          // cents
  "cost-usd": 12,      // cents
  "rate-limits": 41,
  "rate-limit-5h": 34,
  "rate-limit-7d": 62,
  "context-remaining": 49,
  "session-duration": 285, // seconds
  "api-duration": 15,      // seconds
};
