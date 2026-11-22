// Claude Code settings schema — aligned with https://code.claude.com/docs/zh-Hans/settings
// Organized by section for GUI rendering

export interface SettingField {
  key: string;
  /** English label — used as i18n fallback; primary label is i18n key `settings.f_${key}` */
  label: string;
  type: "string" | "boolean" | "select" | "json" | "string[]" | "kv" | "object" | "kv-select";
  options?: string[];
  placeholder?: string;
  description?: string;
  /** When set, renders a path picker button alongside the text input */
  pathType?: "file" | "directory";
  /** When true, skip default FieldRenderer — section handles this field via custom UI */
  skipGui?: boolean;
  /** type "object": fixed sub-fields rendered inline (nested objects stay JSON) */
  objectFields?: ObjectSubField[];
  /** type "kv-select": allowed values for every entry's value side */
  valueOptions?: string[];
  /** type "kv" / "kv-select": placeholder for the key input */
  keyPlaceholder?: string;
}

/** One sub-field of a `type: "object"` setting (voice, spinnerVerbs, ...). */
export interface ObjectSubField {
  key: string;
  label: string;
  type: "string" | "boolean" | "select" | "string[]";
  options?: string[];
  placeholder?: string;
}

export interface SettingSection {
  id: string;
  labelKey: string; // i18n key for section heading
  fields: SettingField[];
}

// ── Sections ──

/** Claude Code `language` 字段可选值（settings.json 顶层 language key）。
 *  单一事实源：CLI 集成 tab 与 claudeTab 的 language 字段共用，禁复制。
 *  注意：最终落盘值由 Claude Code CLI 消费，CLI 不识别的 code 会被忽略回落英文，
 *  新增 code 前最好核对 CLI 实际支持集。 */

/** 两级语族分组：第一级语族（中文/English/...），第二级变体（中文 → 简体/繁体各变种）。
 *  Coding 设置 language select 用 optgroup 渲染；LANGUAGE_OPTIONS 由其派生保持单源。 */
export const LANGUAGE_GROUPS: { family: string; options: { value: string; label: string }[] }[] = [
  { family: "中文", options: [
    { value: "zh-Hans", label: "简体（通用）" },
    { value: "zh-CN", label: "简体（中国）" },
    { value: "zh-SG", label: "简体（新加坡）" },
    { value: "zh-TW", label: "繁体（台湾）" },
    { value: "zh-HK", label: "繁体（香港）" },
  ]},
  { family: "English", options: [{ value: "en-US", label: "English (US)" }] },
  { family: "日本語", options: [{ value: "ja-JP", label: "日本語" }] },
  { family: "한국어", options: [{ value: "ko-KR", label: "한국어" }] },
  { family: "Français", options: [{ value: "fr-FR", label: "Français" }] },
  { family: "Deutsch", options: [{ value: "de-DE", label: "Deutsch" }] },
  { family: "Español", options: [{ value: "es-ES", label: "Español" }] },
  { family: "Português", options: [{ value: "pt-BR", label: "Português (Brasil)" }] },
  { family: "Italiano", options: [{ value: "it-IT", label: "Italiano" }] },
  { family: "Русский", options: [{ value: "ru-RU", label: "Русский" }] },
  { family: "العربية", options: [{ value: "ar-SA", label: "العربية" }] },
  { family: "हिन्दी", options: [{ value: "hi-IN", label: "हिन्दी" }] },
  { family: "ไทย", options: [{ value: "th-TH", label: "ไทย" }] },
  { family: "Tiếng Việt", options: [{ value: "vi-VN", label: "Tiếng Việt" }] },
  { family: "Nederlands", options: [{ value: "nl-NL", label: "Nederlands" }] },
  { family: "Polski", options: [{ value: "pl-PL", label: "Polski" }] },
  { family: "Türkçe", options: [{ value: "tr-TR", label: "Türkçe" }] },
  { family: "Bahasa Indonesia", options: [{ value: "id-ID", label: "Bahasa Indonesia" }] },
  { family: "Bahasa Melayu", options: [{ value: "ms-MY", label: "Bahasa Melayu" }] },
  { family: "বাংলা", options: [{ value: "bn-BD", label: "বাংলা" }] },
  { family: "فارسی", options: [{ value: "fa-IR", label: "فارسی" }] },
  { family: "עברית", options: [{ value: "he-IL", label: "עברית" }] },
  { family: "Čeština", options: [{ value: "cs-CZ", label: "Čeština" }] },
  { family: "Dansk", options: [{ value: "da-DK", label: "Dansk" }] },
  { family: "Suomi", options: [{ value: "fi-FI", label: "Suomi" }] },
  { family: "Ελληνικά", options: [{ value: "el-GR", label: "Ελληνικά" }] },
  { family: "Magyar", options: [{ value: "hu-HU", label: "Magyar" }] },
  { family: "Norsk", options: [{ value: "no-NO", label: "Norsk" }] },
  { family: "Română", options: [{ value: "ro-RO", label: "Română" }] },
  { family: "Slovenčina", options: [{ value: "sk-SK", label: "Slovenčina" }] },
  { family: "Svenska", options: [{ value: "sv-SE", label: "Svenska" }] },
  { family: "Українська", options: [{ value: "uk-UA", label: "Українська" }] },
];

/** 内置 output style（官方 docs.claude.com output-styles：值区分大小写，default = 不选风格）。 */
export const OUTPUT_STYLE_BUILTINS = ["default", "Proactive", "Concise", "Explanatory", "Learning"];

export const LANGUAGE_OPTIONS: string[] = LANGUAGE_GROUPS.flatMap((g) => g.options.map((o) => o.value));

export const SECTIONS: SettingSection[] = [
  {
    id: "core",
    labelKey: "settings.sectionCore",
    fields: [
      // ── 原 core ──
      { key: "model", label: "Model", type: "string", placeholder: "e.g. claude-sonnet-4-6, sonnet" },
      { key: "effortLevel", label: "Effort Level", type: "select", options: ["low", "medium", "high", "xhigh"] },
      { key: "outputStyle", label: "Output Style", type: "string", placeholder: "Explanatory, Concise...", options: OUTPUT_STYLE_BUILTINS },
      { key: "language", label: "Language", type: "string", placeholder: "zh-Hans, en-US, ja-JP...", options: LANGUAGE_OPTIONS },
      { key: "agent", label: "Agent", type: "string", description: "Run the main thread as a named subagent" },
      { key: "apiKeyHelper", label: "API Key Helper", type: "string", placeholder: "/bin/generate_temp_api_key.sh", pathType: "file" },
      { key: "modelOverrides", label: "Model Overrides", type: "kv", description: "Model ID mapping, e.g. Bedrock ARNs" },
      { key: "advisorModel", label: "Advisor Model", type: "string", placeholder: "fable / opus / sonnet", description: "Model used by the server-side advisor tool" },
      { key: "availableModels", label: "Available Models", type: "string[]", description: "Restrict the selectable models (main session / subagent / skill / advisor)" },
      { key: "fallbackModel", label: "Fallback Models", type: "string[]", description: "Model chain to fall back to, in order, when the main model is overloaded" },
      { key: "teammateDefaultModel", label: "Teammate Default Model", type: "string", placeholder: "sonnet", description: "Default model for agent team teammates; empty inherits the lead" },
      { key: "switchModelsOnFlag", label: "Switch Model On Flag", type: "boolean", description: "Automatically switch to the fallback model when the safety classifier flags a request (on by default)" },
      { key: "fastMode", label: "Fast Mode", type: "boolean", description: "Enable fast mode in supported sessions" },
      { key: "skillOverrides", label: "Skill Overrides", type: "kv-select", valueOptions: ["on", "name-only", "user-invocable-only", "off"], keyPlaceholder: "skill-name", description: "Override visibility per skill name without editing SKILL.md" },
      { key: "skillListingMaxDescChars", label: "Skill Listing Max Desc Chars", type: "string", placeholder: "1536", description: "Character limit for description + when_to_use in the skill listing" },
      { key: "disableBundledSkills", label: "Disable Bundled Skills", type: "boolean", description: "Disable Claude Code's built-in skills and workflows" },
      { key: "disableSkillShellExecution", label: "Disable Skill Shell Execution", type: "boolean", description: "Block inline shell execution in skills / custom commands" },
      { key: "workflowSizeGuideline", label: "Workflow Size Guideline", type: "select", options: ["unrestricted", "small", "medium", "large"], description: "Suggested agent scale for dynamic workflows (default medium)" },
      { key: "workflowKeywordTriggerEnabled", label: "Workflow Keyword Trigger", type: "boolean", description: "Whether the ultracode keyword in a prompt triggers a dynamic workflow (on by default)" },
      { key: "autoCompactEnabled", label: "Auto Compact", type: "boolean", description: "Automatically compact when the context nears its limit (on by default)" },
      { key: "autoCompactWindow", label: "Auto Compact Window", type: "string", placeholder: "180000", description: "Context token count that triggers auto-compaction (100000–1000000)" },
      { key: "fileCheckpointingEnabled", label: "File Checkpointing", type: "boolean", description: "Snapshot files before each edit so /rewind can roll back (on by default)" },
      { key: "useAutoModeDuringPlan", label: "Auto Mode During Plan", type: "boolean", description: "Whether plan mode keeps auto mode semantics (on by default)" },
      { key: "askUserQuestionTimeout", label: "AskUserQuestion Timeout", type: "select", options: ["60s", "5m", "10m", "never"], description: "How long AskUserQuestion waits before continuing without an answer (default never)" },
      { key: "dialogExpiry", label: "Dialog Expiry", type: "select", options: ["60s", "5m", "10m", "never"], description: "Response deadline for dialogs forwarded to remote clients (default 5m)" },
      { key: "crossSessionInbound", label: "Cross-Session Inbound", type: "select", options: ["accept", "hold", "refuse"], description: "How to handle cross-session messages from other sessions" },
      { key: "isolatePeerMachines", label: "Isolate Peer Machines", type: "boolean", description: "Require explicit approval before SendMessage delivers across machines" },
      { key: "respondToBashCommands", label: "Respond To Bash Commands", type: "boolean", description: "Whether Claude responds after a ! command runs in the input box (on by default)" },
      { key: "respectGitignore", label: "Respect .gitignore", type: "boolean", description: "Whether the @ file picker respects .gitignore (on by default)" },
      { key: "includeGitInstructions", label: "Include Git Instructions", type: "boolean", description: "Whether the system prompt includes the built-in commit / PR workflow instructions (on by default)" },
      // ── 原 behavior ──
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
      // ── 原 team ──
      { key: "teammateMode", label: "Teammate Mode", type: "select", options: ["auto", "in-process", "tmux"] },
      { key: "fastModePerSessionOptIn", label: "Fast Mode Per-Session", type: "boolean" },
      { key: "autoMode", label: "Auto Mode", type: "json", description: "Auto mode classifier rules" },
      // ── 原 memory ──
      { key: "claudeMdExcludes", label: "CLAUDE.md Excludes", type: "string[]", description: "CLAUDE.md glob patterns to skip" },
      { key: "autoMemoryDirectory", label: "Auto Memory Directory", type: "string", placeholder: "~/my-memory-dir", pathType: "directory" },
      { key: "plansDirectory", label: "Plans Directory", type: "string", placeholder: "~/.claude/plans", pathType: "directory" },
    ],
  },
  {
    id: "ui",
    labelKey: "settings.sectionUI",
    fields: [
      { key: "theme", label: "Theme", type: "string", options: ["auto", "dark", "light", "dark-daltonized", "light-daltonized", "dark-ansi", "light-ansi"], placeholder: "dark", description: "UI color theme; custom:<slug> is also accepted" },
      { key: "tui", label: "TUI Renderer", type: "select", options: ["fullscreen", "default"] },
      { key: "editorMode", label: "Editor Mode", type: "select", options: ["normal", "vim"] },
      { key: "vimInsertModeRemaps", label: "Vim Insert Remaps", type: "kv", keyPlaceholder: "jk", description: "Map two-key sequences to Escape in vim mode; the value is always <Esc>" },
      { key: "defaultShell", label: "Default Shell", type: "select", options: ["bash", "powershell"] },
      { key: "viewMode", label: "View Mode", type: "select", options: ["default", "verbose", "focus"] },
      { key: "verbose", label: "Verbose Output", type: "boolean", description: "Show full tool output instead of truncated summaries" },
      { key: "showThinkingSummaries", label: "Show Thinking Summaries", type: "boolean" },
      { key: "showTurnDuration", label: "Show Turn Duration", type: "boolean" },
      { key: "showClearContextOnPlanAccept", label: "Clear Context On Plan Accept", type: "boolean", description: "Whether the plan acceptance screen shows the \"clear context\" option" },
      { key: "spinnerTipsEnabled", label: "Spinner Tips", type: "boolean" },
      { key: "spinnerTipsOverride", label: "Spinner Tips Override", type: "object", description: "Custom spinner tips", objectFields: [
        { key: "tips", label: "Tips", type: "string[]" },
        { key: "excludeDefault", label: "Exclude Default", type: "boolean" },
      ] },
      { key: "spinnerVerbs", label: "Spinner Verbs", type: "object", description: "Custom action verbs shown while a turn is in progress", objectFields: [
        { key: "mode", label: "Mode", type: "select", options: ["replace", "append"] },
        { key: "verbs", label: "Verbs", type: "string[]" },
      ] },
      { key: "autoScrollEnabled", label: "Auto Scroll", type: "boolean" },
      { key: "wheelScrollAccelerationEnabled", label: "Wheel Scroll Acceleration", type: "boolean", description: "Accelerate fast mouse-wheel scrolling in fullscreen rendering (on by default)" },
      { key: "terminalProgressBarEnabled", label: "Terminal Progress Bar", type: "boolean" },
      { key: "awaySummaryEnabled", label: "Away Summary", type: "boolean" },
      { key: "axScreenReader", label: "Screen Reader Mode", type: "boolean", description: "Plain text output without decorative borders or animation (screen-reader friendly)" },
      { key: "emojiCompletionEnabled", label: "Emoji Completion", type: "boolean", description: "Suggest and replace :shortcode with emoji while typing (on by default)" },
      { key: "promptSuggestionEnabled", label: "Prompt Suggestions", type: "boolean", description: "Gray predictive suggestions in the input box (on by default)" },
      { key: "permissionExplainerEnabled", label: "Permission Explainer", type: "boolean", description: "Press Ctrl+E on a permission prompt to explain the command (on by default)" },
      { key: "spellcheck", label: "Spellcheck", type: "boolean", description: "Underline spelling mistakes while typing (requires a spell checker you install)" },
      { key: "voice", label: "Voice Dictation", type: "object", description: "Voice dictation settings; /voice writes them automatically", objectFields: [
        { key: "enabled", label: "Enabled", type: "boolean" },
        { key: "mode", label: "Mode", type: "select", options: ["hold", "tap"] },
        { key: "autoSubmit", label: "Auto Submit", type: "boolean" },
      ] },
      { key: "voiceEnabled", label: "Voice Enabled (legacy)", type: "boolean", description: "Legacy alias of voice.enabled; prefer the voice object" },
      { key: "externalEditorContext", label: "External Editor Context", type: "boolean", description: "Include the previous reply as comments when Ctrl+G opens an external editor" },
      { key: "diffTool", label: "Diff Tool", type: "select", options: ["auto", "terminal"], description: "Where diffs are shown when connected to an IDE" },
      { key: "autoConnectIde", label: "Auto Connect IDE", type: "boolean", description: "Automatically connect to a running IDE when launched from an external terminal" },
      { key: "autoInstallIdeExtension", label: "Auto Install IDE Extension", type: "boolean", description: "Automatically install the IDE extension when running in a VS Code terminal (on by default)" },
      { key: "footerLinksRegexes", label: "Footer Link Badges", type: "json", description: "Render clickable badges in the footer when a regex matches turn output: [{pattern,url,label}]" },
      { key: "prUrlTemplate", label: "PR URL Template", type: "string", placeholder: "https://review.example.com/{owner}/{repo}/{number}", description: "URL template for the PR badge in the footer" },
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
      { key: "env", label: "Environment Variables", type: "json", description: "KEY-VALUE environment variables" },
    ],
  },
  {
    id: "hooks",
    labelKey: "settings.sectionHooks",
    fields: [
      { key: "hooks", label: "Hooks", type: "json", description: "Lifecycle event hook configuration", skipGui: true },
    ],
  },
  {
    id: "mcp",
    labelKey: "settings.sectionMcp",
    fields: [
      { key: "enableAllProjectMcpServers", label: "Enable All Project MCP Servers", type: "boolean", description: "Automatically approve all MCP servers defined in the project's .mcp.json" },
      { key: "enabledMcpjsonServers", label: "Enabled .mcp.json Servers", type: "string[]", description: ".mcp.json servers approved by name" },
      { key: "disabledMcpjsonServers", label: "Disabled .mcp.json Servers", type: "string[]", description: ".mcp.json servers rejected by name" },
      { key: "disableClaudeAiConnectors", label: "Disable claude.ai Connectors", type: "boolean", description: "Block automatically fetching and connecting claude.ai MCP connectors" },
    ],
  },
  {
    id: "remote",
    labelKey: "settings.sectionRemote",
    fields: [
      { key: "disableRemoteControl", label: "Disable Remote Control", type: "boolean", description: "Disable remote control (command, flag, auto-start and in-session toggle)" },
      { key: "remoteControlAtStartup", label: "Remote Control At Startup", type: "boolean", description: "Automatically connect remote control when each interactive session starts" },
      { key: "agentPushNotifEnabled", label: "Push When Claude Decides", type: "boolean", description: "Allow proactive push notifications to your phone while remote control is connected" },
      { key: "inputNeededNotifEnabled", label: "Push When Input Needed", type: "boolean", description: "Push to your phone when waiting for a permission confirmation or a question" },
      { key: "enableArtifact", label: "Enable Artifact", type: "boolean", description: "Enable the Artifact tool (publish session output as a private claude.ai page)" },
      { key: "disableArtifact", label: "Disable Artifact", type: "boolean", description: "Disable the Artifact tool; same as CLAUDE_CODE_DISABLE_ARTIFACT=1" },
    ],
  },
  {
    id: "plugins",
    labelKey: "settings.sectionPlugins",
    fields: [
      { key: "enabledPlugins", label: "Enabled Plugins", type: "kv", description: "plugin@marketplace → true/false", skipGui: true },
      { key: "extraKnownMarketplaces", label: "Extra Marketplaces", type: "kv", description: "Named marketplace source definitions", skipGui: true },
      { key: "pluginConfigs", label: "Plugin Configs", type: "json", description: "Configure MCP servers and plugin options by plugin ID (plugin@marketplace)", skipGui: true },
      { key: "skippedPlugins", label: "Skipped Plugins", type: "string[]", description: "Plugin IDs the user chose not to install", skipGui: true },
      { key: "skippedMarketplaces", label: "Skipped Marketplaces", type: "string[]", description: "Marketplace names the user chose not to install", skipGui: true },
    ],
  },
  {
    id: "sandbox",
    labelKey: "settings.sectionSandbox",
    fields: [
      { key: "sandbox", label: "Sandbox", type: "json", description: "Sandbox configuration (filesystem / network isolation)", skipGui: true },
    ],
  },
  {
    id: "status",
    labelKey: "settings.sectionStatus",
    fields: [
      { key: "statusLine", label: "Status Line", type: "json", description: "Custom status line configuration", skipGui: true },
      { key: "subagentStatusLine", label: "Subagent Status Line", type: "json", description: "Subagent status line configuration", skipGui: true },
      { key: "fileSuggestion", label: "File Suggestion", type: "string", description: "Path to a custom file suggestion script", pathType: "file" },
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
      // ── 原 advanced ──
      { key: "attribution", label: "Attribution", type: "json", description: "commit / PR attribution fields", skipGui: true },
      { key: "companyAnnouncements", label: "Company Announcements", type: "string[]", description: "List of company announcements" },
      { key: "includeCoAuthoredBy", label: "Include Co-Authored-By", type: "boolean", description: "Deprecated, use attribution instead: whether commits / PRs carry the Claude co-author line" },
      { key: "skillListingBudgetFraction", label: "Skill Listing Budget", type: "string", placeholder: "0.01" },
      { key: "preferredNotifChannel", label: "Notification Channel", type: "select", options: ["auto", "terminal_bell", "iterm2", "iterm2_with_bell", "kitty", "ghostty", "notifications_disabled"] },
      // ── 原 network ──
      { key: "autoUpdatesChannel", label: "Auto Updates Channel", type: "select", options: ["latest", "stable"] },
      { key: "minimumVersion", label: "Minimum Version", type: "string", placeholder: "e.g. 2.1.100" },
      { key: "skipWebFetchPreflight", label: "Skip WebFetch Preflight", type: "boolean" },
      { key: "allowedHttpHookUrls", label: "Allowed HTTP Hook URLs", type: "string[]", description: "Allowlist of HTTP hook URLs" },
      { key: "httpHookAllowedEnvVars", label: "HTTP Hook Env Vars", type: "string[]", description: "Allowlist of environment variables for HTTP hooks" },
      { key: "awsAuthRefresh", label: "AWS Auth Refresh", type: "string", placeholder: "aws sso login --profile myprofile", description: "Custom script that refreshes the .aws directory" },
      { key: "awsCredentialExport", label: "AWS Credential Export", type: "string", placeholder: "/bin/generate_aws_grant.sh", pathType: "file", description: "Custom script that outputs AWS credentials as JSON" },
      { key: "gcpAuthRefresh", label: "GCP Auth Refresh", type: "string", placeholder: "gcloud auth application-default login", description: "Custom script that refreshes GCP Application Default Credentials" },
      { key: "otelHeadersHelper", label: "OTel Headers Helper", type: "string", placeholder: "/bin/generate_otel_headers.sh", pathType: "file", description: "Script that generates dynamic OpenTelemetry request headers" },
      { key: "sshConfigs", label: "SSH Configs", type: "json", description: "SSH connections in the desktop environment dropdown: [{id,name,sshHost,sshPort?,sshIdentityFile?,startDirectory?}]" },
    ],
  },
];

// ── Env Var Definitions ──────────────────────────────────────────
// Known environment variables with dedicated UI controls.
// Values in config.env are always strings; UI converts to/from typed controls.

export type EnvVarType = "boolean" | "select" | "number" | "string" | "password";

export interface EnvVarDef {
  key: string;
  label: string;
  description?: string;
  type: EnvVarType;
  options?: string[];
  placeholder?: string;
  min?: number;
  max?: number;
  group: string;
}

export const ENV_VAR_GROUP_ORDER = [
  "performance", "toggles", "network", "provider", "model",
  "otel", "mcp", "plugins", "ide", "auth", "session", "shell", "debug",
] as const;

export const ENV_VAR_GROUP_LABEL_KEYS: Record<string, string> = {
  performance: "env.group.performance",
  toggles: "env.group.toggles",
  network: "env.group.network",
  provider: "env.group.provider",
  model: "env.group.model",
  otel: "env.group.otel",
  mcp: "env.group.mcp",
  plugins: "env.group.plugins",
  ide: "env.group.ide",
  auth: "env.group.auth",
  session: "env.group.session",
  shell: "env.group.shell",
  debug: "env.group.debug",
};

export const ENV_VAR_DEFS: EnvVarDef[] = [
  // ── Performance & Limits ──
  { key: "CLAUDE_CODE_EFFORT_LEVEL", label: "Effort Level", type: "select", options: ["low", "medium", "high", "xhigh", "max", "auto"], group: "performance" },
  { key: "CLAUDE_AUTOCOMPACT_PCT_OVERRIDE", label: "Auto Compact %", description: "Context capacity percentage that triggers auto-compaction (1-100)", type: "number", min: 1, max: 100, placeholder: "95", group: "performance" },
  { key: "CLAUDE_CODE_MAX_OUTPUT_TOKENS", label: "Max Output Tokens", type: "number", placeholder: "16384", group: "performance" },
  { key: "MAX_THINKING_TOKENS", label: "Max Thinking Tokens", description: "Extended thinking token budget, 0 disables thinking", type: "number", placeholder: "0", group: "performance" },
  { key: "API_TIMEOUT_MS", label: "API Timeout (ms)", type: "number", placeholder: "600000", group: "performance" },
  { key: "BASH_DEFAULT_TIMEOUT_MS", label: "Bash Timeout (ms)", type: "number", placeholder: "120000", group: "performance" },
  { key: "BASH_MAX_OUTPUT_LENGTH", label: "Bash Max Output", description: "Maximum characters of bash output", type: "number", placeholder: "10240", group: "performance" },
  { key: "BASH_MAX_TIMEOUT_MS", label: "Bash Max Timeout (ms)", type: "number", placeholder: "600000", group: "performance" },
  { key: "CLAUDE_CODE_FILE_READ_MAX_OUTPUT_TOKENS", label: "File Read Token Limit", type: "number", placeholder: "10240", group: "performance" },
  { key: "TASK_MAX_OUTPUT_LENGTH", label: "Task Max Output", description: "Maximum characters of subagent output", type: "number", placeholder: "32000", group: "performance" },
  { key: "CLAUDE_CODE_MAX_CONTEXT_TOKENS", label: "Max Context Tokens", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_MAX_RETRIES", label: "Max Retries", type: "number", placeholder: "10", group: "performance" },
  { key: "CLAUDE_CODE_MAX_TOOL_USE_CONCURRENCY", label: "Max Tool Concurrency", type: "number", placeholder: "10", group: "performance" },
  { key: "CLAUDE_CODE_MAX_TURNS", label: "Max Turns", description: "Limit number of agent turns", type: "number", group: "performance" },
  { key: "MAX_MCP_OUTPUT_TOKENS", label: "MCP Output Tokens", description: "Maximum tokens of MCP tool response", type: "number", placeholder: "25000", group: "performance" },
  { key: "MAX_STRUCTURED_OUTPUT_RETRIES", label: "Structured Output Retries", description: "Structured output validation retry count", type: "number", placeholder: "5", group: "performance" },
  { key: "CLAUDE_STREAM_IDLE_TIMEOUT_MS", label: "Stream Idle Timeout (ms)", description: "Stream idle timeout duration", type: "number", placeholder: "300000", group: "performance" },

  // ── Feature Toggles ──
  { key: "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", label: "Disable Nonessential Traffic", description: "Disable auto-update, feedback, error reporting, telemetry", type: "boolean", group: "toggles" },
  { key: "DISABLE_TELEMETRY", label: "Disable Telemetry", description: "Opt out of telemetry", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_TELEMETRY", label: "Enable OpenTelemetry", description: "Enable OTEL data collection", type: "boolean", group: "toggles" },
  { key: "DISABLE_ERROR_REPORTING", label: "Disable Error Reporting", type: "boolean", group: "toggles" },
  { key: "DISABLE_AUTOUPDATER", label: "Disable Auto Updater", type: "boolean", group: "toggles" },
  { key: "DISABLE_UPDATES", label: "Disable All Updates", description: "Block all updates (including manual)", type: "boolean", group: "toggles" },
  { key: "ENABLE_PROMPT_CACHING_1H", label: "Prompt Caching 1H", description: "1-hour prompt cache TTL", type: "boolean", group: "toggles" },
  { key: "DISABLE_PROMPT_CACHING", label: "Disable Prompt Caching", type: "boolean", group: "toggles" },
  { key: "DISABLE_COST_WARNINGS", label: "Disable Cost Warnings", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_FAST_MODE", label: "Disable Fast Mode", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_THINKING", label: "Disable Thinking", description: "Force disable extended thinking", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING", label: "Disable Adaptive Thinking", description: "Fall back to fixed thinking budget", type: "boolean", group: "toggles" },
  { key: "DISABLE_INTERLEAVED_THINKING", label: "Disable Interleaved Thinking", type: "boolean", group: "toggles" },
  { key: "DISABLE_AUTO_COMPACT", label: "Disable Auto Compact", type: "boolean", group: "toggles" },
  { key: "DISABLE_COMPACT", label: "Disable All Compact", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_FILE_CHECKPOINTING", label: "Disable File Checkpointing", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_AUTO_MEMORY", label: "Disable Auto Memory", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_ATTACHMENTS", label: "Disable Attachments", description: "Disable @ file attachment handling", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS", label: "Disable Git Instructions", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_TERMINAL_TITLE", label: "Disable Terminal Title", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", label: "Disable Background Tasks", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_1M_CONTEXT", label: "Disable 1M Context", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_ALTERNATE_SCREEN", label: "Disable Alternate Screen", description: "Use classic main screen renderer", type: "boolean", group: "toggles" },
  { key: "ENABLE_TOOL_SEARCH", label: "Enable Tool Search", description: "MCP tool search lazy loading", type: "select", options: ["true", "auto", "false"], group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_CRON", label: "Disable Cron", description: "Disable scheduled tasks", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_WORKFLOWS", label: "Disable Workflows", type: "boolean", group: "toggles" },
  { key: "DISABLE_LOGIN_COMMAND", label: "Disable Login Command", type: "boolean", group: "toggles" },
  { key: "DISABLE_LOGOUT_COMMAND", label: "Disable Logout Command", type: "boolean", group: "toggles" },
  { key: "DEBUG", label: "Debug Mode", type: "boolean", group: "toggles" },

  // ── Network & Proxy ──
  { key: "ANTHROPIC_BASE_URL", label: "Base URL", description: "Override API endpoint", type: "string", placeholder: "https://api.anthropic.com", group: "network" },
  { key: "ANTHROPIC_API_KEY", label: "API Key", type: "password", group: "network" },
  { key: "ANTHROPIC_AUTH_TOKEN", label: "Auth Token", description: "Custom Authorization header value", type: "password", group: "network" },
  { key: "ANTHROPIC_CUSTOM_HEADERS", label: "Custom Headers", description: "Name: Value format, multiple separated by newline", type: "string", group: "network" },
  { key: "ANTHROPIC_BETAS", label: "Beta Headers", description: "Comma-separated anthropic-beta header values", type: "string", group: "network" },
  { key: "HTTP_PROXY", label: "HTTP Proxy", type: "string", group: "network" },
  { key: "HTTPS_PROXY", label: "HTTPS Proxy", type: "string", group: "network" },
  { key: "NO_PROXY", label: "No Proxy", description: "Domain list bypassing proxy", type: "string", group: "network" },

  // ── Provider Routing ──
  { key: "CLAUDE_CODE_USE_BEDROCK", label: "Use Bedrock", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_USE_VERTEX", label: "Use Vertex AI", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_USE_FOUNDRY", label: "Use Microsoft Foundry", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_USE_ANTHROPIC_AWS", label: "Use Anthropic AWS", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_USE_MANTLE", label: "Use Bedrock Mantle", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_BEDROCK_AUTH", label: "Skip Bedrock Auth", description: "Skip AWS authentication (when using LLM gateway)", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_VERTEX_AUTH", label: "Skip Vertex Auth", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_FOUNDRY_AUTH", label: "Skip Foundry Auth", type: "boolean", group: "provider" },
  { key: "ANTHROPIC_AWS_API_KEY", label: "AWS API Key", description: "Claude Platform on AWS workspace key", type: "password", group: "provider" },
  { key: "ANTHROPIC_AWS_BASE_URL", label: "AWS Base URL", type: "string", group: "provider" },
  { key: "ANTHROPIC_AWS_WORKSPACE_ID", label: "AWS Workspace ID", type: "string", group: "provider" },
  { key: "ANTHROPIC_FOUNDRY_RESOURCE", label: "Foundry Resource", type: "string", group: "provider" },
  { key: "ANTHROPIC_FOUNDRY_BASE_URL", label: "Foundry Base URL", type: "string", group: "provider" },
  { key: "ANTHROPIC_FOUNDRY_API_KEY", label: "Foundry API Key", type: "password", group: "provider" },
  { key: "ANTHROPIC_VERTEX_BASE_URL", label: "Vertex Base URL", type: "string", group: "provider" },
  { key: "ANTHROPIC_VERTEX_PROJECT_ID", label: "Vertex Project ID", type: "string", group: "provider" },
  { key: "ANTHROPIC_BEDROCK_BASE_URL", label: "Bedrock Base URL", type: "string", group: "provider" },
  { key: "ANTHROPIC_BEDROCK_SERVICE_TIER", label: "Bedrock Service Tier", description: "default / flex / priority", type: "select", options: ["default", "flex", "priority"], group: "provider" },

  // ── Model Config ──
  { key: "ANTHROPIC_MODEL", label: "Model Override", description: "Override the model used", type: "string", placeholder: "claude-sonnet-4-6", group: "model" },
  { key: "CLAUDE_CODE_SUBAGENT_MODEL", label: "Subagent Model", type: "string", group: "model" },
  { key: "ANTHROPIC_CUSTOM_MODEL_OPTION", label: "Custom Model Option", description: "Add custom entry in /model selector", type: "string", group: "model" },
  { key: "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME", label: "Custom Model Name", description: "Custom model display name", type: "string", group: "model" },
  { key: "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION", label: "Custom Model Description", description: "Custom model display description", type: "string", group: "model" },

  // ── Misc / Undocumented ──
  { key: "CLAUDE_AUTO_BACKGROUND_TASKS", label: "Auto Background Tasks", description: "Automatically move long-running subagents to background", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ATTRIBUTION_HEADER", label: "Attribution Header", description: "Omit attribution block from system prompt, improves agent caching", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_SUPPRESS_SESSION_ATTRIBUTION", label: "Suppress Session Attribution", description: "Do not attach attribution info to sessions", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_GATEWAY_HINT_HEADERS", label: "Gateway Hint Headers", description: "Send request-class / agent-type hint headers to a custom gateway", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", label: "Agent Teams (Experimental)", description: "Enable agent team collaboration", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_AUTO_COMPACT_WINDOW", label: "Auto Compact Window", description: "Context tokens used for autocompact calculation", type: "number", placeholder: "180000", group: "performance" },
  { key: "CLAUDE_CODE_PLAN_MODE_REQUIRED", label: "Plan Mode Required", type: "boolean", group: "toggles" },
  { key: "FORCE_AUTOUPDATE_PLUGINS", label: "Force Autoupdate Plugins", type: "boolean", group: "plugins" },

  // ── Model Config（续，官方 env-vars 全量） ──
  { key: "ANTHROPIC_DEFAULT_MODEL", label: "Default Model", description: "Model used by default for new sessions", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_OPUS_MODEL", label: "Default Opus Model", description: "Model ID the opus alias resolves to (also used by opusplan)", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME", label: "Opus Display Name", description: "Display name pinned for Opus in the /model picker", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_OPUS_MODEL_DESCRIPTION", label: "Opus Display Description", description: "Display description pinned for Opus in the /model picker", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES", label: "Opus Capabilities", description: "Comma-separated capability list, e.g. effort,thinking", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_SONNET_MODEL", label: "Default Sonnet Model", description: "Model ID the sonnet alias resolves to", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME", label: "Sonnet Display Name", description: "Display name pinned for Sonnet in the /model picker", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_SONNET_MODEL_DESCRIPTION", label: "Sonnet Display Description", description: "Display description pinned for Sonnet in the /model picker", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES", label: "Sonnet Capabilities", description: "Comma-separated capability list, e.g. effort,thinking", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_HAIKU_MODEL", label: "Default Haiku Model", description: "Model ID the haiku alias resolves to; also used for background tasks", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME", label: "Haiku Display Name", description: "Display name pinned for Haiku in the /model picker", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_HAIKU_MODEL_DESCRIPTION", label: "Haiku Display Description", description: "Display description pinned for Haiku in the /model picker", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES", label: "Haiku Capabilities", description: "Comma-separated capability list, e.g. effort,thinking", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_FABLE_MODEL", label: "Default Fable Model", description: "Model ID the fable alias resolves to", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_FABLE_MODEL_NAME", label: "Fable Display Name", description: "Display name pinned for Fable in the /model picker", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_FABLE_MODEL_DESCRIPTION", label: "Fable Display Description", description: "Display description pinned for Fable in the /model picker", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_FABLE_MODEL_SUPPORTED_CAPABILITIES", label: "Fable Capabilities", description: "Comma-separated capability list, e.g. effort,thinking", type: "string", group: "model" },
  { key: "ANTHROPIC_CUSTOM_MODEL_OPTION_SUPPORTED_CAPABILITIES", label: "Custom Model Capabilities", description: "Comma-separated capabilities supported by the custom model entry", type: "string", group: "model" },
  { key: "ANTHROPIC_SMALL_FAST_MODEL", label: "Small Fast Model (deprecated)", description: "Deprecated: Haiku-class model name used for background tasks", type: "string", group: "model" },
  { key: "ANTHROPIC_SMALL_FAST_MODEL_AWS_REGION", label: "Small Fast Model AWS Region", description: "Region override for the Haiku-class model on Bedrock", type: "string", group: "model" },
  { key: "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY", label: "Gateway Model Discovery", description: "Populate the /model picker from the gateway's /v1/models endpoint", type: "boolean", group: "model" },
  { key: "FALLBACK_FOR_ALL_PRIMARY_MODELS", label: "Fallback For All Primary Models", description: "Without a fallback model configured, stop retrying and error out when any model is overloaded", type: "boolean", group: "model" },

  // ── Provider Routing（续） ──
  { key: "ANTHROPIC_BEDROCK_MANTLE_BASE_URL", label: "Bedrock Mantle Base URL", description: "Override the Bedrock Mantle endpoint URL", type: "string", group: "provider" },
  { key: "ANTHROPIC_BEDROCK_REGION_PREFIX", label: "Bedrock Region Prefix", description: "Cross-region inference profile prefix: us / eu / apac / jp / au / global", type: "select", options: ["us", "eu", "apac", "jp", "au", "global"], group: "provider" },
  { key: "ANTHROPIC_FOUNDRY_AUTH_TOKEN", label: "Foundry Auth Token", description: "Bearer token for Microsoft Foundry, such as an Entra access token", type: "password", group: "provider" },
  { key: "ANTHROPIC_ORGANIZATION_ID", label: "Organization ID", description: "Organization ID for workload identity federation, paired with the federation rule ID", type: "string", group: "provider" },
  { key: "ANTHROPIC_WORKSPACE_ID", label: "Workspace ID", description: "Workspace ID to use when the federation rule spans multiple workspaces", type: "string", group: "provider" },
  { key: "ANTHROPIC_FEDERATION_RULE_ID", label: "Federation Rule ID", description: "Rule ID for workload identity federation", type: "string", group: "provider" },
  { key: "ANTHROPIC_PROFILE", label: "Anthropic Profile", description: "Authenticate with a named Anthropic profile, taking precedence over /login credentials", type: "string", group: "provider" },
  { key: "AWS_BEARER_TOKEN_BEDROCK", label: "Bedrock API Key", description: "Amazon Bedrock API key", type: "password", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_MANTLE_AUTH", label: "Skip Mantle Auth", description: "Skip AWS authentication for Bedrock Mantle when routing through a gateway", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_ANTHROPIC_AWS_AUTH", label: "Skip Anthropic AWS Auth", description: "Skip client-side authentication for Claude Platform on AWS", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_AWS_CRED_CACHE", label: "Skip AWS Cred Cache", description: "Disable in-process caching of the AWS credential chain and resolve it per request", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_AWS_CHAIN_RESOLVE_TIMEOUT_MS", label: "AWS Chain Resolve Timeout (ms)", description: "How long to wait for the AWS default credential chain to return credentials", type: "number", group: "provider" },
  { key: "CLAUDE_CODE_DISABLE_BEDROCK_CONTENT_TYPE_GUARD", label: "Disable Bedrock Content-Type Guard", description: "Skip content-type validation on Bedrock streaming responses", type: "boolean", group: "provider" },
  { key: "CLAUDE_ENABLE_BYTE_WATCHDOG_BEDROCK", label: "Byte Watchdog (Bedrock)", description: "Enable a byte-level idle watchdog on Bedrock eventstream responses", type: "boolean", group: "provider" },
  { key: "ENABLE_PROMPT_CACHING_1H_BEDROCK", label: "Prompt Caching 1H (Bedrock, deprecated)", description: "Deprecated; use ENABLE_PROMPT_CACHING_1H instead", type: "boolean", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_5_OPUS", label: "Vertex Region — Opus 5", description: "Region override for Claude Opus 5 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_5_SONNET", label: "Vertex Region — Sonnet 5", description: "Region override for Claude Sonnet 5 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_FABLE_5", label: "Vertex Region — Fable 5", description: "Region override for Claude Fable 5 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_8_OPUS", label: "Vertex Region — Opus 4.8", description: "Region override for Claude Opus 4.8 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_7_OPUS", label: "Vertex Region — Opus 4.7", description: "Region override for Claude Opus 4.7 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_6_OPUS", label: "Vertex Region — Opus 4.6", description: "Region override for Claude Opus 4.6 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_6_SONNET", label: "Vertex Region — Sonnet 4.6", description: "Region override for Claude Sonnet 4.6 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_5_OPUS", label: "Vertex Region — Opus 4.5", description: "Region override for Claude Opus 4.5 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_5_SONNET", label: "Vertex Region — Sonnet 4.5", description: "Region override for Claude Sonnet 4.5 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_HAIKU_4_5", label: "Vertex Region — Haiku 4.5", description: "Region override for Claude Haiku 4.5 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_1_OPUS", label: "Vertex Region — Opus 4.1", description: "Region override for Claude Opus 4.1 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_0_OPUS", label: "Vertex Region — Opus 4.0", description: "Region override for Claude Opus 4.0 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_0_SONNET", label: "Vertex Region — Sonnet 4.0", description: "Region override for Claude Sonnet 4.0 on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_3_7_SONNET", label: "Vertex Region — Sonnet 3.7", description: "Region override for Claude 3.7 Sonnet on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_3_5_SONNET", label: "Vertex Region — Sonnet 3.5", description: "Region override for Claude 3.5 Sonnet on Vertex", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_3_5_HAIKU", label: "Vertex Region — Haiku 3.5", description: "Region override for Claude 3.5 Haiku on Vertex", type: "string", group: "provider" },

  // ── OpenTelemetry / 遥测 ──
  { key: "OTEL_METRICS_EXPORTER", label: "Metrics Exporter", description: "Metrics exporter: otlp / prometheus / console", type: "string", placeholder: "otlp", group: "otel" },
  { key: "OTEL_LOGS_EXPORTER", label: "Logs Exporter", description: "Logs exporter: otlp / console", type: "string", placeholder: "otlp", group: "otel" },
  { key: "OTEL_EXPORTER_OTLP_PROTOCOL", label: "OTLP Protocol", description: "OTLP transport protocol: grpc / http/protobuf / http/json", type: "select", options: ["grpc", "http/protobuf", "http/json"], group: "otel" },
  { key: "OTEL_EXPORTER_OTLP_ENDPOINT", label: "OTLP Endpoint", description: "OTLP collector address", type: "string", placeholder: "http://localhost:4317", group: "otel" },
  { key: "OTEL_EXPORTER_OTLP_HEADERS", label: "OTLP Headers", description: "OTLP request headers in the form key=value,key2=value2", type: "string", group: "otel" },
  { key: "OTEL_LOG_USER_PROMPTS", label: "Log User Prompts", description: "Include raw user prompt text in telemetry (redacted by default)", type: "boolean", group: "otel" },
  { key: "OTEL_LOG_ASSISTANT_RESPONSES", label: "Log Assistant Responses", description: "Include model response text in assistant_response events", type: "boolean", group: "otel" },
  { key: "OTEL_LOG_TOOL_CONTENT", label: "Log Tool Content", description: "Include tool input and output content in span events (off by default)", type: "boolean", group: "otel" },
  { key: "OTEL_LOG_TOOL_DETAILS", label: "Log Tool Details", description: "Include details such as tool parameters, MCP server names, and raw error strings", type: "boolean", group: "otel" },
  { key: "OTEL_LOG_RAW_API_BODIES", label: "Log Raw API Bodies", description: "Export Messages API request and response JSON as log events", type: "boolean", group: "otel" },
  { key: "OTEL_METRICS_INCLUDE_SESSION_ID", label: "Metrics Include Session ID", description: "Include the session ID in metric attributes (included by default; set false to exclude)", type: "boolean", group: "otel" },
  { key: "OTEL_METRICS_INCLUDE_ACCOUNT_UUID", label: "Metrics Include Account UUID", description: "Include the account UUID in metric attributes (included by default)", type: "boolean", group: "otel" },
  { key: "OTEL_METRICS_INCLUDE_VERSION", label: "Metrics Include Version", description: "Include the Claude Code version in metric attributes (off by default)", type: "boolean", group: "otel" },
  { key: "OTEL_METRICS_INCLUDE_ENTRYPOINT", label: "Metrics Include Entrypoint", description: "Include the session entrypoint in metric attributes (off by default)", type: "boolean", group: "otel" },
  { key: "OTEL_METRICS_INCLUDE_RESOURCE_ATTRIBUTES", label: "Metrics Include Resource Attrs", description: "Attach OTEL_RESOURCE_ATTRIBUTES to metric labels (included by default)", type: "boolean", group: "otel" },
  { key: "OTEL_ATTRIBUTE_VALUE_LENGTH_LIMIT", label: "Attribute Value Length Limit", description: "Maximum attribute value length in the OTel SDK", type: "number", group: "otel" },
  { key: "CLAUDE_CODE_OTEL_CONTENT_MAX_LENGTH", label: "OTel Content Max Length", description: "Maximum length of content-bearing telemetry attributes, including the truncation marker", type: "number", group: "otel" },
  { key: "CLAUDE_CODE_OTEL_FLUSH_TIMEOUT_MS", label: "OTel Flush Timeout (ms)", description: "Timeout for flushing pending spans (default 5000)", type: "number", group: "otel" },
  { key: "CLAUDE_CODE_OTEL_SHUTDOWN_TIMEOUT_MS", label: "OTel Shutdown Timeout (ms)", description: "Timeout for exporter shutdown on exit (default 2000)", type: "number", group: "otel" },
  { key: "CLAUDE_CODE_OTEL_HEADERS_HELPER_DEBOUNCE_MS", label: "OTel Headers Refresh (ms)", description: "Refresh interval for dynamic OTel headers (default 1740000)", type: "number", group: "otel" },
  { key: "CLAUDE_CODE_OTEL_DIAG_STDERR", label: "OTel Diagnostics To Stderr", description: "Write exporter diagnostic errors to stderr", type: "boolean", group: "otel" },
  { key: "CLAUDE_CODE_ENABLE_FEEDBACK_SURVEY_FOR_OTEL", label: "Feedback Survey Via OTel", description: "Send the session quality survey to your own OTel collector instead", type: "boolean", group: "otel" },
  { key: "DO_NOT_TRACK", label: "Do Not Track", description: "Opt out of telemetry; equivalent to DISABLE_TELEMETRY", type: "boolean", group: "otel" },

  // ── MCP ──
  { key: "MCP_TIMEOUT", label: "MCP Startup Timeout (ms)", description: "MCP server startup timeout (default 30000)", type: "number", group: "mcp" },
  { key: "MCP_TOOL_TIMEOUT", label: "MCP Tool Timeout (ms)", description: "Timeout for MCP tool execution", type: "number", group: "mcp" },
  { key: "MCP_CONNECT_TIMEOUT_MS", label: "MCP Connect Timeout (ms)", description: "Timeout waiting for a connection batch during blocking startup (default 5000)", type: "number", group: "mcp" },
  { key: "MCP_CONNECTION_NONBLOCKING", label: "MCP Nonblocking Startup", description: "Whether startup waits for MCP servers to connect (nonblocking by default)", type: "boolean", group: "mcp" },
  { key: "MCP_SERVER_CONNECTION_BATCH_SIZE", label: "MCP Local Batch Size", description: "Number of local stdio servers connected in parallel at startup (default 3)", type: "number", group: "mcp" },
  { key: "MCP_REMOTE_SERVER_CONNECTION_BATCH_SIZE", label: "MCP Remote Batch Size", description: "Number of remote HTTP/SSE servers connected in parallel at startup (default 20)", type: "number", group: "mcp" },
  { key: "MCP_DISCOVERY_CACHE", label: "MCP Discovery Cache", description: "Set to 0 to disable the cross-process MCP discovery cache", type: "boolean", group: "mcp" },
  { key: "MCP_SDK_GENERATION", label: "MCP SDK Generation", description: "Pin the MCP client runtime generation to use", type: "string", group: "mcp" },
  { key: "MCP_OAUTH_CALLBACK_PORT", label: "MCP OAuth Callback Port", description: "Fixed port used for the OAuth callback", type: "number", group: "mcp" },
  { key: "MCP_CLIENT_SECRET", label: "MCP Client Secret", description: "OAuth client secret for MCP servers that require preprovisioned credentials", type: "password", group: "mcp" },
  { key: "CLAUDE_CODE_MCP_ALLOWLIST_ENV", label: "MCP Env Allowlist", description: "stdio MCP servers inherit only a safe baseline environment plus their own env", type: "boolean", group: "mcp" },
  { key: "CLAUDE_CODE_MCP_AUTO_BACKGROUND_MS", label: "MCP Auto Background (ms)", description: "How long before an MCP tool call moves to a background task (default 120000; 0 disables)", type: "number", group: "mcp" },
  { key: "CLAUDE_CODE_MCP_TOOL_IDLE_TIMEOUT", label: "MCP Tool Idle Timeout (ms)", description: "Idle timeout for MCP tool calls", type: "number", group: "mcp" },
  { key: "CLAUDE_AGENT_SDK_MCP_NO_PREFIX", label: "SDK MCP No Prefix", description: "Drop the mcp__<server>__ prefix from MCP tool names created by the SDK", type: "boolean", group: "mcp" },
  { key: "ENABLE_CLAUDEAI_MCP_SERVERS", label: "Enable claude.ai MCP Servers", description: "Enable claude.ai MCP servers (on by default for logged-in users)", type: "boolean", group: "mcp" },

  // ── Plugins / Skills ──
  { key: "CLAUDE_CODE_PLUGIN_CACHE_DIR", label: "Plugin Root Dir", description: "Override the plugin root directory, which holds marketplace and plugin caches", type: "string", group: "plugins" },
  { key: "CLAUDE_CODE_PLUGIN_SEED_DIR", label: "Plugin Seed Dir", description: "Read-only plugin seed directories, separated by : or ;", type: "string", group: "plugins" },
  { key: "CLAUDE_CODE_PLUGIN_GIT_TIMEOUT_MS", label: "Plugin Git Timeout (ms)", description: "Timeout for git operations when installing or updating plugins (default 120000)", type: "number", group: "plugins" },
  { key: "CLAUDE_CODE_PLUGIN_PREFER_HTTPS", label: "Plugin Prefer HTTPS", description: "Clone owner/repo shorthand over HTTPS instead of SSH", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_PLUGIN_KEEP_MARKETPLACE_ON_FAILURE", label: "Keep Marketplace On Failure", description: "Keep the existing marketplace cache instead of re-cloning when git pull fails", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL", label: "Disable Official Marketplace Autoinstall", description: "Do not auto-register the official plugin marketplace", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_ENABLE_BACKGROUND_PLUGIN_REFRESH", label: "Background Plugin Refresh", description: "In non-interactive mode, refresh plugin state at turn boundaries after background installs finish", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_SYNC_PLUGIN_INSTALL", label: "Sync Plugin Install", description: "Wait for plugin installation to finish before the first query in non-interactive mode", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_SYNC_PLUGIN_INSTALL_TIMEOUT_MS", label: "Sync Plugin Install Timeout (ms)", description: "Timeout for synchronous plugin installation; continues without plugins when exceeded", type: "number", group: "plugins" },
  { key: "CLAUDE_CODE_SYNC_SKILLS", label: "Sync claude.ai Skills", description: "Sync skills enabled on claude.ai into ~/.claude/skills/synced/", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_SYNC_SKILLS_INSTALL_TIMEOUT_MS", label: "Sync Skills Install Timeout (ms)", description: "Timeout for re-syncing skills during a session (default 30000)", type: "number", group: "plugins" },
  { key: "CLAUDE_CODE_SYNC_SKILLS_WAIT_TIMEOUT_MS", label: "Sync Skills Wait Timeout (ms)", description: "How long the first query waits for the initial skill list (default 5000)", type: "number", group: "plugins" },
  { key: "CLAUDE_CODE_DISABLE_BUNDLED_SKILLS", label: "Disable Bundled Skills", description: "Disable the skills and workflows bundled with Claude Code", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_DISABLE_POLICY_SKILLS", label: "Disable Policy Skills", description: "Do not load the system-level managed skills directory", type: "boolean", group: "plugins" },
  { key: "SLASH_COMMAND_TOOL_CHAR_BUDGET", label: "Slash Command Char Budget", description: "Character budget for skill metadata visible to the Skill tool", type: "number", group: "plugins" },

  // ── IDE / 终端集成 ──
  { key: "CLAUDE_CODE_AUTO_CONNECT_IDE", label: "Auto Connect IDE", description: "Override the IDE auto-connect behaviour", type: "boolean", group: "ide" },
  { key: "CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL", label: "Skip IDE Extension Install", description: "Skip automatic installation of the IDE extension", type: "boolean", group: "ide" },
  { key: "CLAUDE_CODE_IDE_SKIP_VALID_CHECK", label: "Skip IDE Lockfile Check", description: "Skip IDE lockfile validation when connecting", type: "boolean", group: "ide" },
  { key: "CLAUDE_CODE_IDE_HOST_OVERRIDE", label: "IDE Host Override", description: "Override the host address used to connect to the IDE extension", type: "string", group: "ide" },
  { key: "CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST", label: "Provider Managed By Host", description: "Let the host platform own model provider routing (set by the host)", type: "boolean", group: "ide" },

  // ── Network / TLS（续） ──
  { key: "CLAUDE_CODE_CERT_STORE", label: "CA Cert Store", description: "CA certificate sources for TLS, comma-separated: bundled / system", type: "string", placeholder: "bundled,system", group: "network" },
  { key: "CLAUDE_CODE_CLIENT_CERT", label: "Client Certificate", description: "File path to the mTLS client certificate", type: "string", group: "network" },
  { key: "CLAUDE_CODE_CLIENT_KEY", label: "Client Key", description: "File path to the mTLS client private key", type: "string", group: "network" },
  { key: "CLAUDE_CODE_CLIENT_KEY_PASSPHRASE", label: "Client Key Passphrase", description: "Passphrase for the encrypted client private key", type: "password", group: "network" },
  { key: "CLAUDE_CODE_DISABLE_MTLS_RELOAD_ON_STALE_CONNECTION", label: "Disable mTLS Reload", description: "Do not reload the mTLS certificate and key on connection-level errors", type: "boolean", group: "network" },
  { key: "CLAUDE_CODE_PROXY_RESOLVES_HOSTS", label: "Proxy Resolves Hosts", description: "Let the proxy perform DNS resolution instead of resolving locally", type: "boolean", group: "network" },
  { key: "CLAUDE_CODE_PROPAGATE_TRACEPARENT", label: "Propagate Traceparent", description: "Propagate W3C traceparent to downstream", type: "boolean", group: "network" },
  { key: "CLAUDE_CODE_EXTRA_BODY", label: "Extra Request Body", description: "JSON object merged into the top level of every API request body", type: "string", group: "network" },

  // ── Performance & Limits（续） ──
  { key: "API_FORCE_IDLE_TIMEOUT", label: "API Idle Timeout (ms)", description: "Override the 5-minute streaming idle abort; 0 disables it", type: "number", group: "performance" },
  { key: "CLAUDE_BYTE_STREAM_IDLE_TIMEOUT_MS", label: "Byte Stream Idle Timeout (ms)", description: "Timeout of the byte-level streaming idle watchdog", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS", label: "Max Concurrent Subagents", description: "Maximum subagents running concurrently in one session (default 20)", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_SUBAGENT_HARD_CAP", label: "Subagent Hard Cap (hook)", description: "Hard concurrency cap enforced by hook, covers resumed finished agents and agent teams (default 2)", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH", label: "Max Subagent Spawn Depth", description: "Maximum subagent nesting depth (default 3)", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_MAX_SUBAGENTS_PER_SESSION", label: "Max Subagents Per Session (no-op)", description: "No-op since v2.1.224; formerly capped total subagents per session", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_MAX_WEB_SEARCHES_PER_SESSION", label: "Max Web Searches", description: "Maximum WebSearch calls per session (default 200)", type: "number", group: "performance" },
  { key: "CLAUDE_ASYNC_AGENT_STALL_TIMEOUT_MS", label: "Async Agent Stall Timeout (ms)", description: "Stall timeout for background subagents (default 600000)", type: "number", group: "performance" },
  { key: "CLAUDE_SUBAGENT_BG_SHELL_MAX_MS", label: "Subagent Bg Shell Max (ms)", description: "Maximum lifetime of background shell commands started by subagents (default 3600000)", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS", label: "Print Mode Bg Wait (ms)", description: "Upper bound on waiting for background tasks after the final turn in -p mode", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_TEAM_TEARDOWN_PARK_TIMEOUT_MS", label: "Team Teardown Timeout (ms)", description: "How long to wait for agent team teardown on exit (1000–60000)", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_SESSIONEND_HOOKS_TIMEOUT_MS", label: "SessionEnd Hooks Timeout (ms)", description: "Time budget for SessionEnd hooks", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_STOP_HOOK_BLOCK_CAP", label: "Stop Hook Block Cap", description: "Maximum consecutive turns Stop/SubagentStop hooks may block", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_GLOB_TIMEOUT_SECONDS", label: "Glob Timeout (s)", description: "File discovery timeout for the Glob tool (default 20s, 60s on WSL)", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_WEBFETCH_CACHE_TTL_MS", label: "WebFetch Cache TTL (ms)", description: "How long WebFetch results stay cached (default 900000)", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_TOOL_MEMORY_LIMIT", label: "Tool Memory Limit", description: "Limit memory for Bash/PowerShell commands on Linux/WSL, e.g. 4G", type: "string", placeholder: "4G", group: "performance" },
  { key: "CLAUDE_CODE_SCRIPT_CAPS", label: "Script Invocation Caps", description: "JSON object capping per-session invocations of specific scripts", type: "string", group: "performance" },
  { key: "CLAUDE_CODE_SCROLL_SPEED", label: "Scroll Speed", description: "Wheel multiplier in fullscreen rendering (max 20; values below 1 allowed)", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_GOAL_CHECKIN_MINUTES", label: "Goal Check-in (min)", description: "How long background work may block a goal before a check-in fires (default 30; 0 disables)", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_WORKFLOW_PREFIX_STAGGER_MS", label: "Workflow Stagger (ms)", description: "Maximum stagger applied to the first request of workflow agents sharing a prefix", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_RESUME_INTERRUPTED_TURN_MAX_AGE_MS", label: "Resume Turn Max Age (ms)", description: "Maximum age of the last message that still allows resuming an interrupted turn", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_EXIT_AFTER_STOP_DELAY", label: "Exit After Stop Delay (ms)", description: "How long to wait before exiting once the query loop goes idle", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_USER_DIALOG_TIMEOUT_MS", label: "User Dialog Timeout (ms)", description: "Deadline for answering dialogs forwarded to a remote client", type: "number", group: "performance" },
  { key: "CLAUDE_AFK_TIMEOUT_MS", label: "AFK Timeout (ms)", description: "Idle time before AskUserQuestion auto-continues with no answer", type: "number", group: "performance" },
  { key: "CLAUDE_AFK_COUNTDOWN_MS", label: "AFK Countdown (ms)", description: "How far ahead the countdown appears before auto-continuing (default 20000)", type: "number", group: "performance" },
  { key: "CLAUDE_AX_PREPARK_MS", label: "Screen Reader Prepark (ms)", description: "Wait time before writing a new line in screen reader mode", type: "number", group: "performance" },
  { key: "CLAUDE_AX_STARTUP_QUIET_MS", label: "Screen Reader Startup Quiet (ms)", description: "How long the first screen render is delayed in screen reader mode", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_CONNECT_TIMEOUT_MS", label: "Connect Timeout (ms, no-op)", description: "No-op since v2.1.186", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT", label: "Skip Unknown Model Window Enforcement", description: "Skip proactive auto-compaction when the model ID is unrecognized", type: "boolean", group: "performance" },
  { key: "CLAUDE_CODE_API_KEY_HELPER_TTL_MS", label: "API Key Helper TTL (ms)", description: "Refresh interval for credentials from apiKeyHelper", type: "number", group: "performance" },

  // ── Feature Toggles（续） ──
  { key: "CLAUDE_CODE_DISABLE_ADVISOR_TOOL", label: "Disable Advisor Tool", description: "Disable the advisor tool and the /advisor command", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_AGENT_VIEW", label: "Disable Agent View", description: "Turn off background agents and the agent view", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_ARTIFACT", label: "Disable Artifact", description: "Disable the Artifact tool", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_CLAUDE_MDS", label: "Disable CLAUDE.md", description: "Do not load any CLAUDE.md memory files", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_EXPLORE_PLAN_AGENTS", label: "Disable Explore/Plan Agents", description: "Disable the built-in Explore and Plan subagents", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS", label: "Disable Experimental Betas", description: "Strip the anthropic-beta header and beta tool fields", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY", label: "Disable Feedback Survey", description: "Turn off periodic feedback surveys", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_LEGACY_MODEL_REMAP", label: "Disable Legacy Model Remap", description: "Do not remap Opus 4.0/4.1 onto the current Opus automatically", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK", label: "Disable Non-Streaming Fallback", description: "Do not fall back to a non-streaming request when streaming fails midway", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_NOTIFICATION_PRESENCE_CHECK", label: "Disable Notification Presence Check", description: "Send PushNotification desktop notifications even when the terminal is focused", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_PERMISSION_PROMPT_NOTIFY_HOOKS", label: "Disable Permission Notify Hooks", description: "Permission prompts forwarded remotely no longer trigger Notification hooks", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_MOUSE", label: "Disable Mouse", description: "Disable mouse tracking in fullscreen rendering", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_MOUSE_CLICKS", label: "Disable Mouse Clicks", description: "Disable clicks, drags, and hovers while keeping the scroll wheel", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_VIRTUAL_SCROLL", label: "Disable Virtual Scroll", description: "Render the full transcript in fullscreen mode instead of virtualizing scroll", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_BG_EXIT_HANDOFF", label: "Disable Bg Exit Handoff", description: "Stop a background session's tasks when that session exits", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_BG_SHELL_PRESSURE_REAP", label: "Disable Bg Shell Reap", description: "Do not kill background shell commands under memory pressure", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_ADMIN_ENV_UNION", label: "Disable Admin Env Union", description: "Do not merge env blocks key-by-key across managed sources", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ALWAYS_ENABLE_EFFORT", label: "Always Send Effort", description: "Send the effort parameter for every model", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_APPEND_SUBAGENT_PROMPT", label: "Append Subagent Prompt", description: "Allow appending text to the system prompt of non-fork subagents", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_AUTO_MODE", label: "Enable Auto Mode (no-op)", description: "Kept for compatibility and now a no-op: auto mode is available by default", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_AWAY_SUMMARY", label: "Away Summary Override", description: "Force the away summary on or off", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING", label: "Fine-Grained Tool Streaming", description: "Stream tool input parameters as they are generated", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION", label: "Prompt Suggestions", description: "Toggle the greyed-out predictive suggestion in the input box", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_TASKS", label: "Enable Task Tools", description: "Choose which task-tracking tool set is exposed", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_TODO_TOOLS", label: "Enable Todo Tools", description: "Enable task-tracking tools on models that do not include them by default", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_OPUS_4_7_FAST_MODE", label: "Opus 4.7 Fast Mode (removed)", description: "Removed in v2.1.142", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_OPUS_4_6_FAST_MODE_OVERRIDE", label: "Opus 4.6 Fast Mode (removed)", description: "Removed in v2.1.160; now a no-op", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_FORCE_SESSION_PERSISTENCE", label: "Force Session Persistence", description: "Persist the transcript and prompt history even for nested launches", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_FORCE_STRIKETHROUGH", label: "Force Strikethrough", description: "Force rendering of ~~strikethrough~~ text", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_FORCE_SYNC_OUTPUT", label: "Force Synchronized Output", description: "Force DEC 2026 synchronized output on", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_PERFORCE_MODE", label: "Perforce Mode", description: "Enable Perforce write protection: prompt for p4 edit on read-only files", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_USE_NATIVE_FILE_SEARCH", label: "Native File Search", description: "Use Node file APIs instead of ripgrep for file discovery", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_USE_POWERSHELL_TOOL", label: "PowerShell Tool", description: "Control whether the PowerShell tool is enabled", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_FORK_SUBAGENT", label: "Fork Subagent", description: "Allow Claude to spawn fork subagents on its own", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_FORWARD_SUBAGENT_TEXT", label: "Forward Subagent Text", description: "Include subagent text and thinking blocks in stream-json output", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_RESUME_INTERRUPTED_TURN", label: "Resume Interrupted Turn", description: "Automatically resume when the previous session ended mid-turn", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_RETRY_WATCHDOG", label: "Retry Watchdog", description: "Retry 429/529 capacity errors indefinitely for unattended sessions", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_PACKAGE_MANAGER_AUTO_UPDATE", label: "Package Manager Auto Update", description: "Allow package manager upgrade commands to run in the background", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", label: "Subprocess Env Scrub", description: "Strip Anthropic and cloud provider credentials from subprocess environments", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_SKIP_FAST_MODE_ORG_CHECK", label: "Skip Fast Mode Org Check", description: "Skip the client-side fast mode availability check", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_SKIP_FAST_MODE_NETWORK_ERRORS", label: "Skip Fast Mode Network Errors", description: "Treat fast mode as available when the availability check fails", type: "boolean", group: "toggles" },
  { key: "CLAUDE_DISABLE_ADOPT", label: "Disable Adopt", description: "Stop in-flight work instead of adopting it when moving to the background", type: "boolean", group: "toggles" },
  { key: "CLAUDE_ENABLE_BYTE_WATCHDOG", label: "Byte Watchdog", description: "Force the byte-level streaming idle watchdog on or off", type: "boolean", group: "toggles" },
  { key: "CLAUDE_ENABLE_STREAM_WATCHDOG", label: "Stream Watchdog", description: "Force the event-level streaming idle watchdog on or off", type: "boolean", group: "toggles" },
  { key: "CLAUDE_AGENT_SDK_DISABLE_BUILTIN_AGENTS", label: "SDK Disable Builtin Agents", description: "Disable built-in subagents such as Explore and Plan in non-interactive mode", type: "boolean", group: "toggles" },
  { key: "CCR_FORCE_BUNDLE", label: "Force Cloud Bundle", description: "Make claude --cloud bundle and upload the local repository", type: "boolean", group: "toggles" },
  { key: "DISABLE_DOCTOR_COMMAND", label: "Disable /doctor", description: "Hide the /doctor and /checkup commands", type: "boolean", group: "toggles" },
  { key: "DISABLE_FEEDBACK_COMMAND", label: "Disable /feedback", description: "Disable /feedback, including /bug and /share", type: "boolean", group: "toggles" },
  { key: "DISABLE_UPGRADE_COMMAND", label: "Disable /upgrade", description: "Hide the /upgrade command", type: "boolean", group: "toggles" },
  { key: "DISABLE_EXTRA_USAGE_COMMAND", label: "Disable /usage-credits", description: "Hide the command for purchasing extra usage", type: "boolean", group: "toggles" },
  { key: "DISABLE_INSTALL_GITHUB_APP_COMMAND", label: "Disable /install-github-app", description: "Hide the /install-github-app command", type: "boolean", group: "toggles" },
  { key: "DISABLE_PROMPT_CACHING_OPUS", label: "Disable Prompt Caching (Opus)", description: "Disable prompt caching for the Opus family", type: "boolean", group: "toggles" },
  { key: "DISABLE_PROMPT_CACHING_SONNET", label: "Disable Prompt Caching (Sonnet)", description: "Disable prompt caching for the Sonnet family", type: "boolean", group: "toggles" },
  { key: "DISABLE_PROMPT_CACHING_HAIKU", label: "Disable Prompt Caching (Haiku)", description: "Disable prompt caching for the Haiku family", type: "boolean", group: "toggles" },
  { key: "DISABLE_PROMPT_CACHING_FABLE", label: "Disable Prompt Caching (Fable)", description: "Disable prompt caching for the Fable family", type: "boolean", group: "toggles" },
  { key: "FORCE_PROMPT_CACHING_5M", label: "Force 5m Prompt Cache", description: "Force a 5-minute cache TTL, overriding the 1-hour setting", type: "boolean", group: "toggles" },
  { key: "FORCE_HYPERLINK", label: "Force Hyperlinks", description: "Force OSC 8 clickable links on or off", type: "boolean", group: "toggles" },
  { key: "USE_BUILTIN_RIPGREP", label: "Use Builtin Ripgrep", description: "Set to 0 to use the system-installed rg instead", type: "boolean", group: "toggles" },

  // ── Auth / 凭证 ──
  { key: "CLAUDE_CODE_OAUTH_TOKEN", label: "OAuth Token", description: "claude.ai OAuth access token, usable instead of /login", type: "password", group: "auth" },
  { key: "CLAUDE_CODE_OAUTH_REFRESH_TOKEN", label: "OAuth Refresh Token", description: "claude.ai OAuth refresh token, exchanged directly at login", type: "password", group: "auth" },
  { key: "CLAUDE_CODE_OAUTH_SCOPES", label: "OAuth Scopes", description: "Scopes the refresh token was issued with, space-separated", type: "string", group: "auth" },

  // ── Session / 运行时（多数由 Claude Code 自动注入） ──
  { key: "CLAUDE_CONFIG_DIR", label: "Config Dir", description: "Override the configuration directory (default ~/.claude)", type: "string", group: "session" },
  { key: "CLAUDE_CODE_PROJECT_DIR_NAME", label: "Project Dir Name", description: "Used with CLAUDE_CONFIG_DIR to name the directory under projects/", type: "string", group: "session" },
  { key: "CLAUDE_CODE_TMPDIR", label: "Temp Dir", description: "Override the directory used for internal temporary files", type: "string", group: "session" },
  { key: "CLAUDE_CODE_SESSION_ID", label: "Session ID (auto)", description: "Auto-injected: the current session ID inside subprocesses", type: "string", group: "session" },
  { key: "CLAUDE_CODE_BRIDGE_SESSION_ID", label: "Bridge Session ID (auto)", description: "Auto-injected: the bridge session ID while a remote control connection is active", type: "string", group: "session" },
  { key: "CLAUDE_CODE_REMOTE_SESSION_ID", label: "Remote Session ID (auto)", description: "Auto-injected: the session ID of a cloud session", type: "string", group: "session" },
  { key: "CLAUDE_CODE_REMOTE", label: "Is Cloud Session (auto)", description: "Auto-injected: true inside cloud sessions", type: "boolean", group: "session" },
  { key: "CLAUDE_CODE_CHILD_SESSION", label: "Child Session (auto)", description: "Auto-injected: 1 inside subprocesses spawned by Claude Code", type: "boolean", group: "session" },
  { key: "CLAUDECODE", label: "In Claude Code (auto)", description: "Auto-injected: 1 inside subprocesses spawned by Claude Code", type: "boolean", group: "session" },
  { key: "CLAUDE_PID", label: "Claude PID (auto)", description: "Auto-injected: the process ID of Claude Code itself", type: "string", group: "session" },
  { key: "CLAUDE_EFFORT", label: "Current Effort (auto)", description: "Auto-injected: the effort level in force for the current turn", type: "string", group: "session" },
  { key: "CLAUDE_CODE_MESSAGING_SOCKET", label: "Messaging Socket (auto)", description: "Auto-injected: socket path of the session inbox", type: "string", group: "session" },
  { key: "CLAUDE_CODE_MESSAGING_TOKEN", label: "Messaging Token (auto)", description: "Auto-injected: session inbox token", type: "password", group: "session" },
  { key: "CLAUDE_CODE_TASK_LIST_ID", label: "Shared Task List ID", description: "ID that lets multiple sessions share one task list", type: "string", group: "session" },
  { key: "CLAUDE_CODE_RESUME_PROMPT", label: "Resume Prompt", description: "Prompt injected when resuming an interrupted turn", type: "string", group: "session" },
  { key: "CLAUDE_CODE_SKIP_PROMPT_HISTORY", label: "Skip Prompt History", description: "Do not write prompt history and transcripts to disk", type: "boolean", group: "session" },
  { key: "CLAUDE_CODE_SAFE_MODE", label: "Safe Mode", description: "Start in safe mode: skip CLAUDE.md, skills, plugins, hooks, and MCP", type: "boolean", group: "session" },
  { key: "CLAUDE_CODE_SIMPLE", label: "Simple Mode", description: "Minimal system prompt with only Bash and file read/write tools", type: "boolean", group: "session" },
  { key: "CLAUDE_CODE_SIMPLE_SYSTEM_PROMPT", label: "Simple System Prompt", description: "Use a shorter system prompt and trimmed tool descriptions", type: "boolean", group: "session" },
  { key: "CLAUDE_CODE_NEW_INIT", label: "Interactive /init", description: "Make /init use the interactive initialization flow", type: "boolean", group: "session" },
  { key: "CLAUDE_CLIENT_PRESENCE_FILE", label: "Client Presence File", description: "Path to a presence marker file maintained by external tooling, used for lock-screen detection", type: "string", group: "session" },
  { key: "CLAUDE_REMOTE_CONTROL_SESSION_NAME_PREFIX", label: "Remote Control Name Prefix", description: "Prefix for auto-generated remote control session names (defaults to the hostname)", type: "string", group: "session" },
  { key: "CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD", label: "Load CLAUDE.md From --add-dir", description: "Load memory files from directories passed with --add-dir", type: "boolean", group: "session" },

  // ── Shell / 终端 ──
  { key: "CLAUDE_CODE_SHELL", label: "Shell Binary", description: "Path to the shell used by the Bash tool (bash or zsh)", type: "string", placeholder: "/opt/homebrew/bin/bash", group: "shell" },
  { key: "CLAUDE_CODE_SHELL_PREFIX", label: "Shell Prefix", description: "Prefix command wrapped around every spawned shell command", type: "string", group: "shell" },
  { key: "CLAUDE_ENV_FILE", label: "Env File", description: "Path to a script sourced in the same shell process before every Bash command", type: "string", group: "shell" },
  { key: "CLAUDE_CODE_GIT_BASH_PATH", label: "Git Bash Path", description: "Path to Git Bash's bash.exe on Windows", type: "string", group: "shell" },
  { key: "CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR", label: "Keep Project Working Dir", description: "Return to the original working directory after every Bash command", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_POWERSHELL_RESPECT_EXECUTION_POLICY", label: "Respect PowerShell Policy", description: "Stop passing -ExecutionPolicy Bypass", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_PROCESS_WRAPPER", label: "Process Wrapper", description: "Enterprise launcher command wrapping background processes spawned by Claude Code", type: "string", group: "shell" },
  { key: "CLAUDE_CODE_GLOB_HIDDEN", label: "Glob Includes Hidden", description: "Whether Glob results include dotfiles (included by default; false excludes them)", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_GLOB_NO_IGNORE", label: "Glob Ignores .gitignore", description: "Set to false to make Glob honour .gitignore", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_TMUX_TRUECOLOR", label: "Tmux Truecolor", description: "Allow 24-bit truecolor output inside tmux", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_ACCESSIBILITY", label: "Native Cursor (a11y)", description: "Keep the terminal's native cursor so screen magnifiers can follow it", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_NATIVE_CURSOR", label: "Native Cursor", description: "Show the terminal's own cursor at the insertion point instead of drawing a block", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_NO_FLICKER", label: "Fullscreen Rendering", description: "Enable fullscreen rendering that reduces flicker", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_ALT_SCREEN_FULL_REPAINT", label: "Alt Screen Full Repaint", description: "Repaint the whole screen every frame in fullscreen rendering", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_SYNTAX_HIGHLIGHT", label: "Syntax Highlight", description: "Set to false to turn off syntax highlighting in diff output", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_HIDE_CWD", label: "Hide CWD", description: "Hide the working directory in the startup logo, useful for screen recordings", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_ARTIFACT_AUTO_OPEN", label: "Artifact Auto Open", description: "Open the browser automatically when a new artifact is published (0 disables)", type: "boolean", group: "shell" },
  { key: "CLAUDE_AX_SCREEN_READER", label: "Screen Reader Mode", description: "Emit flat text with no decorative borders or animations", type: "boolean", group: "shell" },

  // ── Debug ──
  { key: "CLAUDE_CODE_DEBUG_LOGS_DIR", label: "Debug Log File", description: "Override the debug log file path (debug mode must also be on)", type: "string", group: "debug" },
  { key: "CLAUDE_CODE_DEBUG_LOG_LEVEL", label: "Debug Log Level", description: "Minimum level for debug logs", type: "select", options: ["verbose", "debug", "info", "warn", "error"], group: "debug" },
  { key: "DISABLE_GROWTHBOOK", label: "Disable Feature Flags", description: "Disable feature flags and A/B experiments", type: "boolean", group: "debug" },
  { key: "DISABLE_INSTALLATION_CHECKS", label: "Disable Installation Checks", description: "Skip installation integrity checks at startup", type: "boolean", group: "debug" },
  { key: "IS_DEMO", label: "Demo Mode", description: "Demo mode: hide the email address and organization name", type: "boolean", group: "debug" },
];

/** Map key → def for O(1) lookup */
export const ENV_VAR_DEF_MAP = new Map(ENV_VAR_DEFS.map(d => [d.key, d]));

// All known top-level keys from Claude Code settings.json
export const ALL_SETTING_KEYS = SECTIONS.flatMap(s => s.fields.map(f => f.key));
