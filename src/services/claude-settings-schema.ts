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
      { key: "enabledPlugins", label: "Enabled Plugins", type: "kv", description: "插件@市场 → true/false", skipGui: true },
      { key: "extraKnownMarketplaces", label: "Extra Marketplaces", type: "kv", description: "命名市场源定义", skipGui: true },
      { key: "skillOverrides", label: "Skill Overrides", type: "kv", description: "按 skill 名称的可见性覆盖", skipGui: true },
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

export const ENV_VAR_GROUP_ORDER = ["performance", "toggles", "network", "provider", "model"] as const;

export const ENV_VAR_GROUP_LABEL_KEYS: Record<string, string> = {
  performance: "env.group.performance",
  toggles: "env.group.toggles",
  network: "env.group.network",
  provider: "env.group.provider",
  model: "env.group.model",
};

export const ENV_VAR_DEFS: EnvVarDef[] = [
  // ── Performance & Limits ──
  { key: "CLAUDE_CODE_EFFORT_LEVEL", label: "Effort Level", type: "select", options: ["low", "medium", "high", "xhigh", "max", "auto"], group: "performance" },
  { key: "CLAUDE_AUTOCOMPACT_PCT_OVERRIDE", label: "Auto Compact %", description: "触发自动压缩的上下文容量百分比 (1-100)", type: "number", min: 1, max: 100, placeholder: "95", group: "performance" },
  { key: "CLAUDE_CODE_MAX_OUTPUT_TOKENS", label: "Max Output Tokens", type: "number", placeholder: "16384", group: "performance" },
  { key: "MAX_THINKING_TOKENS", label: "Max Thinking Tokens", description: "扩展思考令牌预算，0 禁用思考", type: "number", placeholder: "0", group: "performance" },
  { key: "API_TIMEOUT_MS", label: "API Timeout (ms)", type: "number", placeholder: "600000", group: "performance" },
  { key: "BASH_DEFAULT_TIMEOUT_MS", label: "Bash Timeout (ms)", type: "number", placeholder: "120000", group: "performance" },
  { key: "BASH_MAX_OUTPUT_LENGTH", label: "Bash Max Output", description: "bash 输出最大字符数", type: "number", placeholder: "10240", group: "performance" },
  { key: "BASH_MAX_TIMEOUT_MS", label: "Bash Max Timeout (ms)", type: "number", placeholder: "600000", group: "performance" },
  { key: "CLAUDE_CODE_FILE_READ_MAX_OUTPUT_TOKENS", label: "File Read Token Limit", type: "number", placeholder: "10240", group: "performance" },
  { key: "TASK_MAX_OUTPUT_LENGTH", label: "Task Max Output", description: "subagent 输出最大字符数", type: "number", placeholder: "32000", group: "performance" },
  { key: "CLAUDE_CODE_MAX_CONTEXT_TOKENS", label: "Max Context Tokens", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_MAX_RETRIES", label: "Max Retries", type: "number", placeholder: "10", group: "performance" },
  { key: "CLAUDE_CODE_MAX_TOOL_USE_CONCURRENCY", label: "Max Tool Concurrency", type: "number", placeholder: "10", group: "performance" },
  { key: "CLAUDE_CODE_MAX_TURNS", label: "Max Turns", description: "限制代理转换数量", type: "number", group: "performance" },
  { key: "MAX_MCP_OUTPUT_TOKENS", label: "MCP Output Tokens", description: "MCP 工具响应最大令牌数", type: "number", placeholder: "25000", group: "performance" },
  { key: "MAX_STRUCTURED_OUTPUT_RETRIES", label: "Structured Output Retries", description: "结构化输出验证重试次数", type: "number", placeholder: "5", group: "performance" },
  { key: "CLAUDE_STREAM_IDLE_TIMEOUT_MS", label: "Stream Idle Timeout (ms)", description: "流式空闲超时", type: "number", placeholder: "300000", group: "performance" },

  // ── Feature Toggles ──
  { key: "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", label: "Disable Nonessential Traffic", description: "禁用自动更新、反馈、错误报告、遥测", type: "boolean", group: "toggles" },
  { key: "DISABLE_TELEMETRY", label: "Disable Telemetry", description: "选择退出遥测", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_TELEMETRY", label: "Enable OpenTelemetry", description: "启用 OTEL 数据收集", type: "boolean", group: "toggles" },
  { key: "DISABLE_ERROR_REPORTING", label: "Disable Error Reporting", type: "boolean", group: "toggles" },
  { key: "DISABLE_AUTOUPDATER", label: "Disable Auto Updater", type: "boolean", group: "toggles" },
  { key: "DISABLE_UPDATES", label: "Disable All Updates", description: "阻止所有更新（含手动）", type: "boolean", group: "toggles" },
  { key: "ENABLE_PROMPT_CACHING_1H", label: "Prompt Caching 1H", description: "1 小时 prompt cache TTL", type: "boolean", group: "toggles" },
  { key: "DISABLE_PROMPT_CACHING", label: "Disable Prompt Caching", type: "boolean", group: "toggles" },
  { key: "DISABLE_COST_WARNINGS", label: "Disable Cost Warnings", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_FAST_MODE", label: "Disable Fast Mode", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_THINKING", label: "Disable Thinking", description: "强制禁用扩展思考", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING", label: "Disable Adaptive Thinking", description: "回退固定思考预算", type: "boolean", group: "toggles" },
  { key: "DISABLE_INTERLEAVED_THINKING", label: "Disable Interleaved Thinking", type: "boolean", group: "toggles" },
  { key: "DISABLE_AUTO_COMPACT", label: "Disable Auto Compact", type: "boolean", group: "toggles" },
  { key: "DISABLE_COMPACT", label: "Disable All Compact", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_FILE_CHECKPOINTING", label: "Disable File Checkpointing", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_AUTO_MEMORY", label: "Disable Auto Memory", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_ATTACHMENTS", label: "Disable Attachments", description: "禁用 @ 文件附件处理", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS", label: "Disable Git Instructions", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_TERMINAL_TITLE", label: "Disable Terminal Title", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", label: "Disable Background Tasks", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_1M_CONTEXT", label: "Disable 1M Context", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_ALTERNATE_SCREEN", label: "Disable Alternate Screen", description: "使用经典主屏幕渲染器", type: "boolean", group: "toggles" },
  { key: "ENABLE_TOOL_SEARCH", label: "Enable Tool Search", description: "MCP 工具搜索延迟加载", type: "select", options: ["true", "auto", "false"], group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_CRON", label: "Disable Cron", description: "禁用计划任务", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_WORKFLOWS", label: "Disable Workflows", type: "boolean", group: "toggles" },
  { key: "DISABLE_LOGIN_COMMAND", label: "Disable Login Command", type: "boolean", group: "toggles" },
  { key: "DISABLE_LOGOUT_COMMAND", label: "Disable Logout Command", type: "boolean", group: "toggles" },
  { key: "DEBUG", label: "Debug Mode", type: "boolean", group: "toggles" },

  // ── Network & Proxy ──
  { key: "ANTHROPIC_BASE_URL", label: "Base URL", description: "覆盖 API 端点", type: "string", placeholder: "https://api.anthropic.com", group: "network" },
  { key: "ANTHROPIC_API_KEY", label: "API Key", type: "password", group: "network" },
  { key: "ANTHROPIC_AUTH_TOKEN", label: "Auth Token", description: "自定义 Authorization 标头值", type: "password", group: "network" },
  { key: "ANTHROPIC_CUSTOM_HEADERS", label: "Custom Headers", description: "Name: Value 格式，多个用换行分隔", type: "string", group: "network" },
  { key: "ANTHROPIC_BETAS", label: "Beta Headers", description: "逗号分隔的 anthropic-beta 标头值", type: "string", group: "network" },
  { key: "HTTP_PROXY", label: "HTTP Proxy", type: "string", group: "network" },
  { key: "HTTPS_PROXY", label: "HTTPS Proxy", type: "string", group: "network" },
  { key: "NO_PROXY", label: "No Proxy", description: "绕过代理的域名列表", type: "string", group: "network" },

  // ── Provider Routing ──
  { key: "CLAUDE_CODE_USE_BEDROCK", label: "Use Bedrock", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_USE_VERTEX", label: "Use Vertex AI", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_USE_FOUNDRY", label: "Use Microsoft Foundry", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_USE_ANTHROPIC_AWS", label: "Use Anthropic AWS", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_USE_MANTLE", label: "Use Bedrock Mantle", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_BEDROCK_AUTH", label: "Skip Bedrock Auth", description: "跳过 AWS 身份验证（使用 LLM 网关时）", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_VERTEX_AUTH", label: "Skip Vertex Auth", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_FOUNDRY_AUTH", label: "Skip Foundry Auth", type: "boolean", group: "provider" },
  { key: "ANTHROPIC_AWS_API_KEY", label: "AWS API Key", description: "Claude Platform on AWS 工作区密钥", type: "password", group: "provider" },
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
  { key: "ANTHROPIC_MODEL", label: "Model Override", description: "覆盖使用的模型", type: "string", placeholder: "claude-sonnet-4-6", group: "model" },
  { key: "CLAUDE_CODE_SUBAGENT_MODEL", label: "Subagent Model", type: "string", group: "model" },
  { key: "ANTHROPIC_CUSTOM_MODEL_OPTION", label: "Custom Model Option", description: "在 /model 选择器中添加自定义条目", type: "string", group: "model" },
  { key: "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME", label: "Custom Model Name", description: "自定义模型显示名称", type: "string", group: "model" },
  { key: "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION", label: "Custom Model Description", description: "自定义模型显示描述", type: "string", group: "model" },

  // ── Misc / Undocumented ──
  { key: "CLAUDE_AUTO_BACKGROUND_TASKS", label: "Auto Background Tasks", description: "自动将长时间运行的子代理移到后台", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ATTRIBUTION_HEADER", label: "Attribution Header", description: "从系统提示省略归属块，改善代理缓存", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", label: "Agent Teams (Experimental)", description: "启用代理团队协作", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_AUTO_COMPACT_WINDOW", label: "Auto Compact Window", description: "用于自动压缩计算的上下文令牌数", type: "number", placeholder: "180000", group: "performance" },
  { key: "CLAUDE_CODE_PLAN_MODE_REQUIRED", label: "Plan Mode Required", type: "boolean", group: "toggles" },
  { key: "FORCE_AUTOUPDATE_PLUGINS", label: "Force Autoupdate Plugins", type: "boolean", group: "toggles" },
];

/** Map key → def for O(1) lookup */
export const ENV_VAR_DEF_MAP = new Map(ENV_VAR_DEFS.map(d => [d.key, d]));

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
  "autoConnectIde": true,
  "alwaysThinkingEnabled": true,
  "autoMemoryEnabled": true,
  "cleanupPeriodDays": 30,
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
    "CLAUDE_CODE_MAX_TOOL_USE_CONCURRENCY": "10",
    "BASH_MAX_OUTPUT_LENGTH": "10240",
    "CLAUDE_AUTOCOMPACT_PCT_OVERRIDE": "80",
    "CLAUDE_AUTO_BACKGROUND_TASKS": "1",
    "CLAUDE_CODE_ATTRIBUTION_HEADER": "0",
    "CLAUDE_CODE_ENABLE_TELEMETRY": "0",
    "ANTHROPIC_MODEL": "sonnet",
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1",
    "NO_PROXY": "localhost,127.0.0.1,192.168.0.0/16,10.0.0.0/8,*.cn",
    "CLAUDE_CODE_FILE_READ_MAX_OUTPUT_TOKENS": "10240",
    "CLAUDE_CODE_AUTO_COMPACT_WINDOW": "180000",
    "CLAUDE_CODE_PLAN_MODE_REQUIRED": "true",
    "FORCE_AUTOUPDATE_PLUGINS": "true",
    "CLAUDE_CODE_EFFORT_LEVEL": "medium",
    "CLAUDE_CODE_DISABLE_FAST_MODE": "1",
    "ENABLE_TOOL_SEARCH": "0",
    "ENABLE_PROMPT_CACHING_1H": "1",
  },
};
