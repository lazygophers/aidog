// Representative segment-config set for golden-output regression.
// `null` segments → use the built-in default for that kind.
// Each config exercises a slice of segment types / options / colors / alignment.

const seg = (id, type, opts = {}, extra = {}) => ({
  id, type, enabled: true, newline: false, options: opts, ...extra,
});

export const CONFIGS = {
  // Built-in 3-row main default (mixed fixed colors + dynamic group rows).
  "main-default": { segments: null },
  // Built-in subagent default (agent-badge + name + metrics).
  "subagent-default": { segments: null },

  // Every fixed-color / plain atomic segment, one row, each with its own affix
  // separator so empties drop cleanly. Covers number formatting + truncation.
  "atoms-fixed": {
    segments: [
      seg("a-model", "model", { format: "full" }, { color: "#4A9EFF" }),
      seg("a-costusd", "cost-usd", { prefix: "$", affixPre: "·" }),
      seg("a-sdur", "session-duration", { format: "human", affixPre: "·" }),
      seg("a-sdur-ms", "session-duration", { format: "ms", affixPre: "·" }),
      seg("a-adur", "api-duration", { format: "human", affixPre: "·" }),
      seg("a-lines", "lines-changed", { affixPre: "·" }),
      seg("a-ctok-split", "context-tokens", { mode: "split", abbrev: true, affixPre: "·" }),
      seg("a-ctok-sum", "context-tokens", { mode: "sum", abbrev: false, affixPre: "·" }),
      seg("a-cmax", "context-max", { abbrev: true, affixPre: "·" }),
      seg("a-crem", "context-remaining", { affixPre: "·" }),
      seg("a-ccache-tok", "context-cache", { mode: "tokens", abbrev: true, affixPre: "·" }),
      seg("a-ccache-hit", "context-cache", { mode: "hitrate", prefix: "缓存 ", affixPre: "·" }),
      seg("a-r5", "rate-limit-5h", { showReset: true, affixPre: "·" }),
      seg("a-r7", "rate-limit-7d", { showReset: true, affixPre: "·" }),
      seg("a-r5n", "rate-limit-5h", { showReset: false, affixPre: "·" }),
    ],
  },

  // Git/dir/session/worktree/pr/other atomic fields (no git subprocess except branch).
  "atoms-fields": {
    segments: [
      seg("f-host", "git-host", { affixPre: "·" }),
      seg("f-owner", "git-owner", { affixPre: "·" }),
      seg("f-repo", "git-repo", { affixPre: "·" }),
      seg("f-repofull", "git-repo-full", { affixPre: "·" }),
      seg("f-wt", "git-worktree", { affixPre: "·" }),
      seg("f-cwd-b", "cwd", { format: "basename", affixPre: "·" }),
      seg("f-cwd-f", "cwd", { format: "full", affixPre: "·" }),
      seg("f-pdir", "project-dir", { format: "basename", affixPre: "·" }),
      seg("f-added", "added-dirs", { affixPre: "·" }),
      seg("f-sid", "session-id", { truncate: true, affixPre: "·" }),
      seg("f-sidf", "session-id", { truncate: false, affixPre: "·" }),
      seg("f-sname", "session-name", { affixPre: "·" }),
      seg("f-tpath", "transcript-path", { format: "basename", affixPre: "·" }),
      seg("f-wtn", "worktree-name", { affixPre: "·" }),
      seg("f-wtb", "worktree-branch", { affixPre: "·" }),
      seg("f-wtob", "worktree-original-branch", { affixPre: "·" }),
      seg("f-prn", "pr-number", { prefix: "#", affixPre: "·" }),
      seg("f-pru", "pr-url", { affixPre: "·" }),
      seg("f-prs", "pr-state", { affixPre: "·" }),
      seg("f-ver", "version", { prefix: "v", affixPre: "·" }),
      seg("f-ostyle", "output-style", { affixPre: "·" }),
      seg("f-think", "thinking", { label: "thinking", affixPre: "·" }),
      seg("f-twarn", "token-warn", { label: "⚠200k", affixPre: "·" }),
      seg("f-agent", "agent", { affixPre: "·" }),
      seg("f-effort", "effort", { affixPre: "·" }),
      seg("f-vim", "vim", { affixPre: "·" }),
      seg("f-custom", "custom", { expr: ".model.display_name", affixPre: "·" }),
    ],
  },

  // autoColor value segments (threshold-driven ANSI) + context-bar widths.
  "atoms-autocolor": {
    segments: [
      seg("c-ctxpct", "context-pct", {}, { autoColor: true }),
      seg("c-ctxbar", "context-bar", { width: 12 }, { autoColor: true, affixPre: "·" }),
      seg("c-cost", "cost-usd", { prefix: "$", affixPre: "·" }, { autoColor: true }),
      seg("c-crem", "context-remaining", { affixPre: "·" }, { autoColor: true }),
      seg("c-r5", "rate-limit-5h", { affixPre: "·" }, { autoColor: true }),
      seg("c-r7", "rate-limit-7d", { affixPre: "·" }, { autoColor: true }),
      seg("c-sdur", "session-duration", { format: "human", affixPre: "·" }, { autoColor: true }),
      seg("c-rl", "rate-limits", { windows: "both", affixPre: "·" }, { autoColor: true }),
      seg("c-cost-leg", "cost", { showDuration: true, affixPre: "·" }, { autoColor: true }),
      seg("c-git", "git", { showRepo: true, affixPre: "·" }),
      seg("c-ctxpct-leg", "context-pct", { affixPre: "·" }),
    ],
  },

  // Multi-row alignment: left / center / right rows with width-based padding.
  "align-rows": {
    segments: [
      seg("al-m", "model", { format: "short" }, { align: "left", color: "#4A9EFF" }),
      seg("al-c", "cost-usd", { prefix: "$" }, { newline: true, align: "center", color: "#8E8E93" }),
      seg("al-r", "version", { prefix: "v" }, { newline: true, align: "right", color: "#34C759" }),
    ],
  },

  // aidog group segments (static + dynamic color) — needs ANTHROPIC_BASE_URL.
  "group-static": {
    segments: [
      seg("g-bal", "group-balance", { prefix: "余额 ", dynamicColor: false }),
      seg("g-spent", "group-spent", { prefix: "$", affixPre: " · " }),
      seg("g-coding", "group-coding", { dynamicColor: false }, { newline: true }),
      seg("g-req", "group-requests", { affixPre: " · " }),
      seg("g-cache", "group-cache", { prefix: "缓存 ", affixPre: " · " }),
      seg("g-tok", "group-tokens", { prefix: "", affixPre: " · " }),
    ],
  },
  "group-dynamic": {
    segments: [
      seg("gd-coding", "group-coding", { dynamicColor: true }),
      seg("gd-bal", "group-balance", { prefix: "$", dynamicColor: true, affixPre: "·" }),
    ],
  },

  // Subagent variants: agent-badge status/color permutations exercised via inputs.
  "subagent-default-2": { segments: null },
};
