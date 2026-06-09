// Claude Code settings schema — aligned with https://code.claude.com/docs/zh-CN/settings
// Organized by section for GUI rendering

export interface SettingField {
  key: string;
  /** English label — used as i18n fallback; primary label is i18n key `settings.f_${key}` */
  label: string;
  type: "string" | "boolean" | "select" | "json" | "string[]" | "kv";
  options?: string[];
  placeholder?: string;
  description?: string;
  /** When set, renders a path picker button alongside the text input */
  pathType?: "file" | "directory";
  /** When true, skip default FieldRenderer — section handles this field via custom UI */
  skipGui?: boolean;
}

export interface SettingSection {
  id: string;
  labelKey: string; // i18n key for section heading
  fields: SettingField[];
}

// ── Sections ──

export const SECTIONS: SettingSection[] = [
  {
    id: "core",
    labelKey: "settings.sectionCore",
    fields: [
      { key: "model", label: "Model", type: "string", placeholder: "e.g. claude-sonnet-4-6, sonnet" },
      { key: "effortLevel", label: "Effort Level", type: "select", options: ["low", "medium", "high", "xhigh"] },
      { key: "outputStyle", label: "Output Style", type: "string", placeholder: "Explanatory, Concise..." },
      { key: "language", label: "Language", type: "string", placeholder: "zh-CN, en-US, ja-JP...", options: ["zh-CN", "en-US", "ja-JP", "ko-KR", "fr-FR", "de-DE", "es-ES", "pt-BR", "it-IT", "ru-RU", "ar-SA", "hi-IN", "th-TH", "vi-VN"] },
      { key: "agent", label: "Agent", type: "string", description: "将主线程作为命名 subagent 运行" },
      { key: "apiKeyHelper", label: "API Key Helper", type: "string", placeholder: "/bin/generate_temp_api_key.sh", pathType: "file" },
      { key: "modelOverrides", label: "Model Overrides", type: "kv", description: "模型 ID 映射，如 Bedrock ARN" },
    ],
  },
  {
    id: "behavior",
    labelKey: "settings.sectionBehavior",
    fields: [
      { key: "alwaysThinkingEnabled", label: "Always Thinking", type: "boolean" },
      { key: "autoMemoryEnabled", label: "Auto Memory", type: "boolean" },
      { key: "prefersReducedMotion", label: "Reduced Motion", type: "boolean" },
      { key: "skipDangerousModePermissionPrompt", label: "Skip Dangerous Mode Prompt", type: "boolean" },
      { key: "feedbackSurveyRate", label: "Survey Rate (0-1)", type: "string", placeholder: "0" },
      { key: "cleanupPeriodDays", label: "Cleanup Period (days)", type: "string", placeholder: "30" },
      { key: "disableAllHooks", label: "Disable All Hooks", type: "boolean" },
      { key: "disableWorkflows", label: "Disable Workflows", type: "boolean" },
      { key: "disableDeepLinkRegistration", label: "Disable Deep Link", type: "select", options: ["disable", ""] },
      { key: "disableAutoMode", label: "Disable Auto Mode", type: "select", options: ["disable", ""] },
      { key: "disableAgentView", label: "Disable Agent View", type: "boolean" },
      { key: "syntaxHighlightingDisabled", label: "Disable Syntax Highlight", type: "boolean" },
    ],
  },
  {
    id: "ui",
    labelKey: "settings.sectionUI",
    fields: [
      { key: "tui", label: "TUI Renderer", type: "select", options: ["fullscreen", "default"] },
      { key: "editorMode", label: "Editor Mode", type: "select", options: ["normal", "vim"] },
      { key: "defaultShell", label: "Default Shell", type: "select", options: ["bash", "powershell"] },
      { key: "viewMode", label: "View Mode", type: "select", options: ["default", "verbose", "focus"] },
      { key: "showThinkingSummaries", label: "Show Thinking Summaries", type: "boolean" },
      { key: "showTurnDuration", label: "Show Turn Duration", type: "boolean" },
      { key: "spinnerTipsEnabled", label: "Spinner Tips", type: "boolean" },
      { key: "autoScrollEnabled", label: "Auto Scroll", type: "boolean" },
      { key: "terminalProgressBarEnabled", label: "Terminal Progress Bar", type: "boolean" },
      { key: "awaySummaryEnabled", label: "Away Summary", type: "boolean" },
    ],
  },
  {
    id: "team",
    labelKey: "settings.sectionTeam",
    fields: [
      { key: "teammateMode", label: "Teammate Mode", type: "select", options: ["auto", "in-process", "tmux"] },
      { key: "fastModePerSessionOptIn", label: "Fast Mode Per-Session", type: "boolean" },
      { key: "autoMode", label: "Auto Mode", type: "json", description: "自动模式分类器规则" },
    ],
  },
  {
    id: "permissions",
    labelKey: "settings.sectionPermissions",
    fields: [
      { key: "permissions", label: "Permissions", type: "json", description: "{ allow:[], ask:[], deny:[], defaultMode, ... }" },
    ],
  },
  {
    id: "env",
    labelKey: "settings.sectionEnv",
    fields: [
      { key: "env", label: "Environment Variables", type: "json", description: "KEY-VALUE 环境变量" },
    ],
  },
  {
    id: "hooks",
    labelKey: "settings.sectionHooks",
    fields: [
      { key: "hooks", label: "Hooks", type: "json", description: "生命周期事件钩子配置", skipGui: true },
    ],
  },
  {
    id: "plugins",
    labelKey: "settings.sectionPlugins",
    fields: [
      { key: "enabledPlugins", label: "Enabled Plugins", type: "kv", description: "插件@市场 → true/false" },
      { key: "extraKnownMarketplaces", label: "Extra Marketplaces", type: "string[]", description: "额外插件市场源" },
      { key: "skillOverrides", label: "Skill Overrides", type: "kv", description: "按 skill 名称的可见性覆盖" },
    ],
  },
  {
    id: "sandbox",
    labelKey: "settings.sectionSandbox",
    fields: [
      { key: "sandbox", label: "Sandbox", type: "json", description: "沙箱配置（文件系统/网络隔离）", skipGui: true },
    ],
  },
  {
    id: "status",
    labelKey: "settings.sectionStatus",
    fields: [
      { key: "statusLine", label: "Status Line", type: "string", description: "自定义状态行模板" },
      { key: "subagentStatusLine", label: "Subagent Status Line", type: "string", description: "子代理状态行模板" },
      { key: "fileSuggestion", label: "File Suggestion", type: "string", description: "自定义文件建议脚本路径", pathType: "file" },
    ],
  },
  {
    id: "network",
    labelKey: "settings.sectionNetwork",
    fields: [
      { key: "autoUpdatesChannel", label: "Auto Updates Channel", type: "select", options: ["latest", "stable"] },
      { key: "minimumVersion", label: "Minimum Version", type: "string", placeholder: "e.g. 2.1.100" },
      { key: "skipWebFetchPreflight", label: "Skip WebFetch Preflight", type: "boolean" },
      { key: "allowedHttpHookUrls", label: "Allowed HTTP Hook URLs", type: "string[]", description: "HTTP hook URL 白名单" },
      { key: "httpHookAllowedEnvVars", label: "HTTP Hook Env Vars", type: "string[]", description: "HTTP hook 环境变量白名单" },
    ],
  },
  {
    id: "memory",
    labelKey: "settings.sectionMemory",
    fields: [
      { key: "claudeMdExcludes", label: "CLAUDE.md Excludes", type: "string[]", description: "跳过的 CLAUDE.md glob 模式" },
      { key: "autoMemoryDirectory", label: "Auto Memory Directory", type: "string", placeholder: "~/my-memory-dir", pathType: "directory" },
      { key: "plansDirectory", label: "Plans Directory", type: "string", placeholder: "~/.claude/plans", pathType: "directory" },
    ],
  },
  {
    id: "worktree",
    labelKey: "settings.sectionWorktree",
    fields: [
      { key: "worktree", label: "Worktree Config", type: "json", description: "{ baseRef, sparsePaths, symlinkDirectories, bgIsolation }" },
    ],
  },
  {
    id: "advanced",
    labelKey: "settings.sectionAdvanced",
    fields: [
      { key: "attribution", label: "Attribution", type: "kv", description: "commit / pr 等署名字段" },
      { key: "companyAnnouncements", label: "Company Announcements", type: "json" },
      { key: "maxSkillDescriptionChars", label: "Max Skill Description", type: "string", placeholder: "1536" },
      { key: "skillListingBudgetFraction", label: "Skill Listing Budget", type: "string", placeholder: "0.01" },
      { key: "preferredNotifChannel", label: "Notification Channel", type: "select", options: ["auto", "terminal_bell", "iterm2", "iterm2_with_bell", "kitty", "ghostty", "notifications_disabled"] },
    ],
  },
];

// All known top-level keys from Claude Code settings.json
export const ALL_SETTING_KEYS = SECTIONS.flatMap(s => s.fields.map(f => f.key));

// ── Recommended config (from settings.glm.json, sanitized) ──

/** Detect system language and map to locale code */
function detectLanguage(): string {
  const nav = typeof navigator !== "undefined" ? navigator : null;
  const lang = nav?.language ?? "en-US";
  return lang;
}

export const RECOMMENDED_CONFIG: Record<string, any> = {
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "language": detectLanguage(),
  "alwaysThinkingEnabled": true,
  "autoMemoryEnabled": true,
  "prefersReducedMotion": true,
  "skipDangerousModePermissionPrompt": true,
  "showThinkingSummaries": true,
  "showTurnDuration": true,
  "autoScrollEnabled": true,
  "terminalProgressBarEnabled": true,
  "feedbackSurveyRate": 0,
  "teammateMode": "auto",
  "attribution": {
    "commit": "",
    "pr": "",
  },
  "permissions": {
    "defaultMode": "bypassPermissions",
  },
  "env": {
    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1",
    "CLAUDE_CODE_ENABLE_TELEMETRY": "0",
    "CLAUDE_CODE_EFFORT_LEVEL": "medium",
    "BASH_MAX_OUTPUT_LENGTH": "10240",
    "CLAUDE_AUTOCOMPACT_PCT_OVERRIDE": "80",
    "CLAUDE_CODE_FILE_READ_MAX_OUTPUT_TOKENS": "10240",
    "CLAUDE_CODE_PLAN_MODE_REQUIRED": "true",
    "ENABLE_PROMPT_CACHING_1H": "1",
  },
};
