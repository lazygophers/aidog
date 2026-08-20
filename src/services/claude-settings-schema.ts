// Claude Code settings schema — aligned with https://code.claude.com/docs/zh-Hans/settings
// Organized by section for GUI rendering

import DEFAULT_SETTINGS from "../../src-tauri/defaults/settings.json";

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

export const LANGUAGE_OPTIONS: string[] = LANGUAGE_GROUPS.flatMap((g) => g.options.map((o) => o.value));

export const SECTIONS: SettingSection[] = [
  {
    id: "core",
    labelKey: "settings.sectionCore",
    fields: [
      // ── 原 core ──
      { key: "model", label: "Model", type: "string", placeholder: "e.g. claude-sonnet-4-6, sonnet" },
      { key: "effortLevel", label: "Effort Level", type: "select", options: ["low", "medium", "high", "xhigh"] },
      { key: "outputStyle", label: "Output Style", type: "string", placeholder: "Explanatory, Concise..." },
      { key: "language", label: "Language", type: "string", placeholder: "zh-Hans, en-US, ja-JP...", options: LANGUAGE_OPTIONS },
      { key: "agent", label: "Agent", type: "string", description: "将主线程作为命名 subagent 运行" },
      { key: "apiKeyHelper", label: "API Key Helper", type: "string", placeholder: "/bin/generate_temp_api_key.sh", pathType: "file" },
      { key: "modelOverrides", label: "Model Overrides", type: "kv", description: "模型 ID 映射，如 Bedrock ARN" },
      { key: "advisorModel", label: "Advisor Model", type: "string", placeholder: "fable / opus / sonnet", description: "服务端 advisor 工具使用的模型" },
      { key: "availableModels", label: "Available Models", type: "string[]", description: "限制可选模型清单（主会话 / subagent / skill / advisor）" },
      { key: "fallbackModel", label: "Fallback Models", type: "string[]", description: "主模型过载时按顺序回退的模型链" },
      { key: "teammateDefaultModel", label: "Teammate Default Model", type: "string", placeholder: "sonnet", description: "agent team 队友默认模型，留空继承 lead" },
      { key: "switchModelsOnFlag", label: "Switch Model On Flag", type: "boolean", description: "安全分类器命中时自动切换到回退模型（默认开）" },
      { key: "fastMode", label: "Fast Mode", type: "boolean", description: "在支持的会话中开启快速模式" },
      { key: "skillOverrides", label: "Skill Overrides", type: "kv-select", valueOptions: ["on", "name-only", "user-invocable-only", "off"], keyPlaceholder: "skill-name", description: "按 skill 名覆盖可见性，无需改 SKILL.md" },
      { key: "skillListingMaxDescChars", label: "Skill Listing Max Desc Chars", type: "string", placeholder: "1536", description: "skill 清单里 description + when_to_use 的字符上限" },
      { key: "disableBundledSkills", label: "Disable Bundled Skills", type: "boolean", description: "禁用 Claude Code 内置 skill 与 workflow" },
      { key: "disableSkillShellExecution", label: "Disable Skill Shell Execution", type: "boolean", description: "禁止 skill / 自定义命令内联执行 shell" },
      { key: "workflowSizeGuideline", label: "Workflow Size Guideline", type: "select", options: ["unrestricted", "small", "medium", "large"], description: "动态 workflow 的 agent 规模建议（默认 medium）" },
      { key: "workflowKeywordTriggerEnabled", label: "Workflow Keyword Trigger", type: "boolean", description: "提示词里的 ultracode 关键字是否触发动态 workflow（默认开）" },
      { key: "autoCompactEnabled", label: "Auto Compact", type: "boolean", description: "上下文接近上限时自动压缩（默认开）" },
      { key: "autoCompactWindow", label: "Auto Compact Window", type: "string", placeholder: "180000", description: "触发自动压缩的上下文令牌数（100000–1000000）" },
      { key: "fileCheckpointingEnabled", label: "File Checkpointing", type: "boolean", description: "每次编辑前快照文件，供 /rewind 回滚（默认开）" },
      { key: "useAutoModeDuringPlan", label: "Auto Mode During Plan", type: "boolean", description: "plan 模式是否沿用 auto 模式语义（默认开）" },
      { key: "askUserQuestionTimeout", label: "AskUserQuestion Timeout", type: "select", options: ["60s", "5m", "10m", "never"], description: "AskUserQuestion 无人应答的自动继续时限（默认 never）" },
      { key: "dialogExpiry", label: "Dialog Expiry", type: "select", options: ["60s", "5m", "10m", "never"], description: "转发给远程客户端的对话框应答期限（默认 5m）" },
      { key: "crossSessionInbound", label: "Cross-Session Inbound", type: "select", options: ["accept", "hold", "refuse"], description: "如何处理来自其他会话的跨会话消息" },
      { key: "isolatePeerMachines", label: "Isolate Peer Machines", type: "boolean", description: "SendMessage 跨机器投递前需显式批准" },
      { key: "respondToBashCommands", label: "Respond To Bash Commands", type: "boolean", description: "输入框 ! 命令执行后是否让 Claude 应答（默认开）" },
      { key: "respectGitignore", label: "Respect .gitignore", type: "boolean", description: "@ 文件选择器是否遵守 .gitignore（默认开）" },
      { key: "includeGitInstructions", label: "Include Git Instructions", type: "boolean", description: "系统提示是否含内置 commit / PR 流程说明（默认开）" },
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
      { key: "autoMode", label: "Auto Mode", type: "json", description: "自动模式分类器规则" },
      // ── 原 memory ──
      { key: "claudeMdExcludes", label: "CLAUDE.md Excludes", type: "string[]", description: "跳过的 CLAUDE.md glob 模式" },
      { key: "autoMemoryDirectory", label: "Auto Memory Directory", type: "string", placeholder: "~/my-memory-dir", pathType: "directory" },
      { key: "plansDirectory", label: "Plans Directory", type: "string", placeholder: "~/.claude/plans", pathType: "directory" },
    ],
  },
  {
    id: "ui",
    labelKey: "settings.sectionUI",
    fields: [
      { key: "theme", label: "Theme", type: "string", options: ["auto", "dark", "light", "dark-daltonized", "light-daltonized", "dark-ansi", "light-ansi"], placeholder: "dark", description: "界面配色，也可填 custom:<slug>" },
      { key: "tui", label: "TUI Renderer", type: "select", options: ["fullscreen", "default"] },
      { key: "editorMode", label: "Editor Mode", type: "select", options: ["normal", "vim"] },
      { key: "vimInsertModeRemaps", label: "Vim Insert Remaps", type: "kv", keyPlaceholder: "jk", description: "vim 模式下把两键序列映射为 Escape，值固定 <Esc>" },
      { key: "defaultShell", label: "Default Shell", type: "select", options: ["bash", "powershell"] },
      { key: "viewMode", label: "View Mode", type: "select", options: ["default", "verbose", "focus"] },
      { key: "verbose", label: "Verbose Output", type: "boolean", description: "显示完整工具输出而非截断摘要" },
      { key: "showThinkingSummaries", label: "Show Thinking Summaries", type: "boolean" },
      { key: "showTurnDuration", label: "Show Turn Duration", type: "boolean" },
      { key: "showClearContextOnPlanAccept", label: "Clear Context On Plan Accept", type: "boolean", description: "plan 接受页是否显示「清空上下文」选项" },
      { key: "spinnerTipsEnabled", label: "Spinner Tips", type: "boolean" },
      { key: "spinnerTipsOverride", label: "Spinner Tips Override", type: "object", description: "自定义 spinner 提示语", objectFields: [
        { key: "tips", label: "Tips", type: "string[]" },
        { key: "excludeDefault", label: "Exclude Default", type: "boolean" },
      ] },
      { key: "spinnerVerbs", label: "Spinner Verbs", type: "object", description: "自定义回合进行中的动作词", objectFields: [
        { key: "mode", label: "Mode", type: "select", options: ["replace", "append"] },
        { key: "verbs", label: "Verbs", type: "string[]" },
      ] },
      { key: "autoScrollEnabled", label: "Auto Scroll", type: "boolean" },
      { key: "wheelScrollAccelerationEnabled", label: "Wheel Scroll Acceleration", type: "boolean", description: "全屏渲染下快速滚轮加速（默认开）" },
      { key: "terminalProgressBarEnabled", label: "Terminal Progress Bar", type: "boolean" },
      { key: "awaySummaryEnabled", label: "Away Summary", type: "boolean" },
      { key: "axScreenReader", label: "Screen Reader Mode", type: "boolean", description: "扁平文本输出，无装饰边框与动画（屏幕阅读器友好）" },
      { key: "emojiCompletionEnabled", label: "Emoji Completion", type: "boolean", description: "输入 :shortcode 时提示并替换为 emoji（默认开）" },
      { key: "promptSuggestionEnabled", label: "Prompt Suggestions", type: "boolean", description: "输入框里的灰色预测建议（默认开）" },
      { key: "permissionExplainerEnabled", label: "Permission Explainer", type: "boolean", description: "权限提示上按 Ctrl+E 解释命令（默认开）" },
      { key: "spellcheck", label: "Spellcheck", type: "boolean", description: "输入时下划线标出拼写错误（需自行安装拼写检查器）" },
      { key: "voice", label: "Voice Dictation", type: "object", description: "语音听写设置，/voice 会自动写入", objectFields: [
        { key: "enabled", label: "Enabled", type: "boolean" },
        { key: "mode", label: "Mode", type: "select", options: ["hold", "tap"] },
        { key: "autoSubmit", label: "Auto Submit", type: "boolean" },
      ] },
      { key: "voiceEnabled", label: "Voice Enabled (legacy)", type: "boolean", description: "voice.enabled 的旧别名，优先用 voice 对象" },
      { key: "externalEditorContext", label: "External Editor Context", type: "boolean", description: "Ctrl+G 打开外部编辑器时带上上一条回复作为注释" },
      { key: "diffTool", label: "Diff Tool", type: "select", options: ["auto", "terminal"], description: "连接 IDE 时 diff 的展示位置" },
      { key: "autoConnectIde", label: "Auto Connect IDE", type: "boolean", description: "从外部终端启动时自动连接运行中的 IDE" },
      { key: "autoInstallIdeExtension", label: "Auto Install IDE Extension", type: "boolean", description: "在 VS Code 终端里运行时自动安装 IDE 扩展（默认开）" },
      { key: "footerLinksRegexes", label: "Footer Link Badges", type: "json", description: "正则命中回合输出时在底栏渲染可点徽标：[{pattern,url,label}]" },
      { key: "prUrlTemplate", label: "PR URL Template", type: "string", placeholder: "https://review.example.com/{owner}/{repo}/{number}", description: "底栏 PR 徽标的 URL 模板" },
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
    id: "mcp",
    labelKey: "settings.sectionMcp",
    fields: [
      { key: "enableAllProjectMcpServers", label: "Enable All Project MCP Servers", type: "boolean", description: "自动批准项目 .mcp.json 里定义的全部 MCP 服务器" },
      { key: "enabledMcpjsonServers", label: "Enabled .mcp.json Servers", type: "string[]", description: "按名批准的 .mcp.json 服务器" },
      { key: "disabledMcpjsonServers", label: "Disabled .mcp.json Servers", type: "string[]", description: "按名拒绝的 .mcp.json 服务器" },
      { key: "disableClaudeAiConnectors", label: "Disable claude.ai Connectors", type: "boolean", description: "禁止自动拉取与连接 claude.ai MCP 连接器" },
    ],
  },
  {
    id: "remote",
    labelKey: "settings.sectionRemote",
    fields: [
      { key: "disableRemoteControl", label: "Disable Remote Control", type: "boolean", description: "禁用远程控制（命令、flag、自动启动与会话内开关）" },
      { key: "remoteControlAtStartup", label: "Remote Control At Startup", type: "boolean", description: "每次交互式会话启动即自动连接远程控制" },
      { key: "agentPushNotifEnabled", label: "Push When Claude Decides", type: "boolean", description: "远程控制已连接时，允许主动推送到手机" },
      { key: "inputNeededNotifEnabled", label: "Push When Input Needed", type: "boolean", description: "等待权限确认或提问时推送到手机" },
      { key: "enableArtifact", label: "Enable Artifact", type: "boolean", description: "启用 Artifact 工具（把会话输出发布为 claude.ai 私有页面）" },
      { key: "disableArtifact", label: "Disable Artifact", type: "boolean", description: "禁用 Artifact 工具，等价于 CLAUDE_CODE_DISABLE_ARTIFACT=1" },
    ],
  },
  {
    id: "plugins",
    labelKey: "settings.sectionPlugins",
    fields: [
      { key: "enabledPlugins", label: "Enabled Plugins", type: "kv", description: "插件@市场 → true/false", skipGui: true },
      { key: "extraKnownMarketplaces", label: "Extra Marketplaces", type: "kv", description: "命名市场源定义", skipGui: true },
      { key: "pluginConfigs", label: "Plugin Configs", type: "json", description: "按插件 ID（plugin@marketplace）配置 MCP 服务器与插件选项", skipGui: true },
      { key: "skippedPlugins", label: "Skipped Plugins", type: "string[]", description: "用户选择不安装的插件 ID", skipGui: true },
      { key: "skippedMarketplaces", label: "Skipped Marketplaces", type: "string[]", description: "用户选择不安装的市场名", skipGui: true },
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
      { key: "statusLine", label: "Status Line", type: "json", description: "自定义状态行配置", skipGui: true },
      { key: "subagentStatusLine", label: "Subagent Status Line", type: "json", description: "子代理状态行配置", skipGui: true },
      { key: "fileSuggestion", label: "File Suggestion", type: "string", description: "自定义文件建议脚本路径", pathType: "file" },
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
      { key: "attribution", label: "Attribution", type: "json", description: "commit / pr 署名字段", skipGui: true },
      { key: "companyAnnouncements", label: "Company Announcements", type: "string[]", description: "公司公告列表" },
      { key: "includeCoAuthoredBy", label: "Include Co-Authored-By", type: "boolean", description: "已废弃，改用 attribution：commit / PR 是否带 Claude 署名行" },
      { key: "skillListingBudgetFraction", label: "Skill Listing Budget", type: "string", placeholder: "0.01" },
      { key: "preferredNotifChannel", label: "Notification Channel", type: "select", options: ["auto", "terminal_bell", "iterm2", "iterm2_with_bell", "kitty", "ghostty", "notifications_disabled"] },
      // ── 原 network ──
      { key: "autoUpdatesChannel", label: "Auto Updates Channel", type: "select", options: ["latest", "stable"] },
      { key: "minimumVersion", label: "Minimum Version", type: "string", placeholder: "e.g. 2.1.100" },
      { key: "skipWebFetchPreflight", label: "Skip WebFetch Preflight", type: "boolean" },
      { key: "allowedHttpHookUrls", label: "Allowed HTTP Hook URLs", type: "string[]", description: "HTTP hook URL 白名单" },
      { key: "httpHookAllowedEnvVars", label: "HTTP Hook Env Vars", type: "string[]", description: "HTTP hook 环境变量白名单" },
      { key: "awsAuthRefresh", label: "AWS Auth Refresh", type: "string", placeholder: "aws sso login --profile myprofile", description: "刷新 .aws 目录的自定义脚本" },
      { key: "awsCredentialExport", label: "AWS Credential Export", type: "string", placeholder: "/bin/generate_aws_grant.sh", pathType: "file", description: "输出 AWS 凭证 JSON 的自定义脚本" },
      { key: "gcpAuthRefresh", label: "GCP Auth Refresh", type: "string", placeholder: "gcloud auth application-default login", description: "刷新 GCP 应用默认凭证的自定义脚本" },
      { key: "otelHeadersHelper", label: "OTel Headers Helper", type: "string", placeholder: "/bin/generate_otel_headers.sh", pathType: "file", description: "生成动态 OpenTelemetry 请求头的脚本" },
      { key: "sshConfigs", label: "SSH Configs", type: "json", description: "桌面端环境下拉里的 SSH 连接：[{id,name,sshHost,sshPort?,sshIdentityFile?,startDirectory?}]" },
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
  { key: "FORCE_AUTOUPDATE_PLUGINS", label: "Force Autoupdate Plugins", type: "boolean", group: "plugins" },

  // ── Model Config（续，官方 env-vars 全量） ──
  { key: "ANTHROPIC_DEFAULT_MODEL", label: "Default Model", description: "新会话默认使用的模型", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_OPUS_MODEL", label: "Default Opus Model", description: "opus 别名解析到的模型 ID（opusplan 计划模式也用它）", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME", label: "Opus Display Name", description: "/model 选择器里固定 Opus 的显示名", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_OPUS_MODEL_DESCRIPTION", label: "Opus Display Description", description: "/model 选择器里固定 Opus 的显示描述", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES", label: "Opus Capabilities", description: "逗号分隔的能力列表，如 effort,thinking", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_SONNET_MODEL", label: "Default Sonnet Model", description: "sonnet 别名解析到的模型 ID", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME", label: "Sonnet Display Name", description: "/model 选择器里固定 Sonnet 的显示名", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_SONNET_MODEL_DESCRIPTION", label: "Sonnet Display Description", description: "/model 选择器里固定 Sonnet 的显示描述", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES", label: "Sonnet Capabilities", description: "逗号分隔的能力列表，如 effort,thinking", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_HAIKU_MODEL", label: "Default Haiku Model", description: "haiku 别名解析到的模型 ID，后台任务也用它", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME", label: "Haiku Display Name", description: "/model 选择器里固定 Haiku 的显示名", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_HAIKU_MODEL_DESCRIPTION", label: "Haiku Display Description", description: "/model 选择器里固定 Haiku 的显示描述", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES", label: "Haiku Capabilities", description: "逗号分隔的能力列表，如 effort,thinking", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_FABLE_MODEL", label: "Default Fable Model", description: "fable 别名解析到的模型 ID", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_FABLE_MODEL_NAME", label: "Fable Display Name", description: "/model 选择器里固定 Fable 的显示名", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_FABLE_MODEL_DESCRIPTION", label: "Fable Display Description", description: "/model 选择器里固定 Fable 的显示描述", type: "string", group: "model" },
  { key: "ANTHROPIC_DEFAULT_FABLE_MODEL_SUPPORTED_CAPABILITIES", label: "Fable Capabilities", description: "逗号分隔的能力列表，如 effort,thinking", type: "string", group: "model" },
  { key: "ANTHROPIC_CUSTOM_MODEL_OPTION_SUPPORTED_CAPABILITIES", label: "Custom Model Capabilities", description: "自定义模型条目支持的能力，逗号分隔", type: "string", group: "model" },
  { key: "ANTHROPIC_SMALL_FAST_MODEL", label: "Small Fast Model (deprecated)", description: "已废弃：后台任务使用的 Haiku 级模型名", type: "string", group: "model" },
  { key: "ANTHROPIC_SMALL_FAST_MODEL_AWS_REGION", label: "Small Fast Model AWS Region", description: "Bedrock 上 Haiku 级模型的区域覆盖", type: "string", group: "model" },
  { key: "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY", label: "Gateway Model Discovery", description: "从网关 /v1/models 端点填充 /model 选择器", type: "boolean", group: "model" },
  { key: "FALLBACK_FOR_ALL_PRIMARY_MODELS", label: "Fallback For All Primary Models", description: "未配置回退模型时，任何模型过载即停止重试并报错", type: "boolean", group: "model" },

  // ── Provider Routing（续） ──
  { key: "ANTHROPIC_BEDROCK_MANTLE_BASE_URL", label: "Bedrock Mantle Base URL", description: "覆盖 Bedrock Mantle 端点 URL", type: "string", group: "provider" },
  { key: "ANTHROPIC_BEDROCK_REGION_PREFIX", label: "Bedrock Region Prefix", description: "跨区域推理配置前缀：us / eu / apac / jp / au / global", type: "select", options: ["us", "eu", "apac", "jp", "au", "global"], group: "provider" },
  { key: "ANTHROPIC_FOUNDRY_AUTH_TOKEN", label: "Foundry Auth Token", description: "Microsoft Foundry 的 Bearer 令牌（如 Entra 访问令牌）", type: "password", group: "provider" },
  { key: "ANTHROPIC_ORGANIZATION_ID", label: "Organization ID", description: "工作负载身份联合的组织 ID，与 Federation Rule ID 配对", type: "string", group: "provider" },
  { key: "ANTHROPIC_WORKSPACE_ID", label: "Workspace ID", description: "联合规则跨多工作区时指定的工作区 ID", type: "string", group: "provider" },
  { key: "ANTHROPIC_FEDERATION_RULE_ID", label: "Federation Rule ID", description: "工作负载身份联合规则 ID", type: "string", group: "provider" },
  { key: "ANTHROPIC_PROFILE", label: "Anthropic Profile", description: "使用命名的 Anthropic 配置档鉴权（优先于 /login 凭证）", type: "string", group: "provider" },
  { key: "AWS_BEARER_TOKEN_BEDROCK", label: "Bedrock API Key", description: "Amazon Bedrock API key", type: "password", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_MANTLE_AUTH", label: "Skip Mantle Auth", description: "跳过 Bedrock Mantle 的 AWS 鉴权（走网关时）", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_ANTHROPIC_AWS_AUTH", label: "Skip Anthropic AWS Auth", description: "跳过 Claude Platform on AWS 的客户端鉴权", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_SKIP_AWS_CRED_CACHE", label: "Skip AWS Cred Cache", description: "关闭 AWS 凭证链的进程内缓存，每次请求重新解析", type: "boolean", group: "provider" },
  { key: "CLAUDE_CODE_AWS_CHAIN_RESOLVE_TIMEOUT_MS", label: "AWS Chain Resolve Timeout (ms)", description: "等待 AWS 默认凭证链返回凭证的超时", type: "number", group: "provider" },
  { key: "CLAUDE_CODE_DISABLE_BEDROCK_CONTENT_TYPE_GUARD", label: "Disable Bedrock Content-Type Guard", description: "跳过 Bedrock 流式响应的 content-type 校验", type: "boolean", group: "provider" },
  { key: "CLAUDE_ENABLE_BYTE_WATCHDOG_BEDROCK", label: "Byte Watchdog (Bedrock)", description: "对 Bedrock eventstream 响应启用字节级空闲看门狗", type: "boolean", group: "provider" },
  { key: "ENABLE_PROMPT_CACHING_1H_BEDROCK", label: "Prompt Caching 1H (Bedrock, deprecated)", description: "已废弃，改用 ENABLE_PROMPT_CACHING_1H", type: "boolean", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_5_OPUS", label: "Vertex Region — Opus 5", description: "Vertex 上 Claude Opus 5 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_5_SONNET", label: "Vertex Region — Sonnet 5", description: "Vertex 上 Claude Sonnet 5 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_FABLE_5", label: "Vertex Region — Fable 5", description: "Vertex 上 Claude Fable 5 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_8_OPUS", label: "Vertex Region — Opus 4.8", description: "Vertex 上 Claude Opus 4.8 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_7_OPUS", label: "Vertex Region — Opus 4.7", description: "Vertex 上 Claude Opus 4.7 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_6_OPUS", label: "Vertex Region — Opus 4.6", description: "Vertex 上 Claude Opus 4.6 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_6_SONNET", label: "Vertex Region — Sonnet 4.6", description: "Vertex 上 Claude Sonnet 4.6 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_5_OPUS", label: "Vertex Region — Opus 4.5", description: "Vertex 上 Claude Opus 4.5 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_5_SONNET", label: "Vertex Region — Sonnet 4.5", description: "Vertex 上 Claude Sonnet 4.5 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_HAIKU_4_5", label: "Vertex Region — Haiku 4.5", description: "Vertex 上 Claude Haiku 4.5 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_1_OPUS", label: "Vertex Region — Opus 4.1", description: "Vertex 上 Claude Opus 4.1 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_0_OPUS", label: "Vertex Region — Opus 4.0", description: "Vertex 上 Claude Opus 4.0 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_4_0_SONNET", label: "Vertex Region — Sonnet 4.0", description: "Vertex 上 Claude Sonnet 4.0 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_3_7_SONNET", label: "Vertex Region — Sonnet 3.7", description: "Vertex 上 Claude 3.7 Sonnet 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_3_5_SONNET", label: "Vertex Region — Sonnet 3.5", description: "Vertex 上 Claude 3.5 Sonnet 的区域覆盖", type: "string", group: "provider" },
  { key: "VERTEX_REGION_CLAUDE_3_5_HAIKU", label: "Vertex Region — Haiku 3.5", description: "Vertex 上 Claude 3.5 Haiku 的区域覆盖", type: "string", group: "provider" },

  // ── OpenTelemetry / 遥测 ──
  { key: "OTEL_METRICS_EXPORTER", label: "Metrics Exporter", description: "指标导出器：otlp / prometheus / console", type: "string", placeholder: "otlp", group: "otel" },
  { key: "OTEL_LOGS_EXPORTER", label: "Logs Exporter", description: "日志导出器：otlp / console", type: "string", placeholder: "otlp", group: "otel" },
  { key: "OTEL_EXPORTER_OTLP_PROTOCOL", label: "OTLP Protocol", description: "OTLP 传输协议：grpc / http/protobuf / http/json", type: "select", options: ["grpc", "http/protobuf", "http/json"], group: "otel" },
  { key: "OTEL_EXPORTER_OTLP_ENDPOINT", label: "OTLP Endpoint", description: "OTLP 采集器地址", type: "string", placeholder: "http://localhost:4317", group: "otel" },
  { key: "OTEL_EXPORTER_OTLP_HEADERS", label: "OTLP Headers", description: "OTLP 请求头，形如 key=value,key2=value2", type: "string", group: "otel" },
  { key: "OTEL_LOG_USER_PROMPTS", label: "Log User Prompts", description: "在遥测中包含用户提示词原文（默认脱敏）", type: "boolean", group: "otel" },
  { key: "OTEL_LOG_ASSISTANT_RESPONSES", label: "Log Assistant Responses", description: "在 assistant_response 事件中包含模型回复正文", type: "boolean", group: "otel" },
  { key: "OTEL_LOG_TOOL_CONTENT", label: "Log Tool Content", description: "在 span 事件中包含工具输入输出内容（默认关）", type: "boolean", group: "otel" },
  { key: "OTEL_LOG_TOOL_DETAILS", label: "Log Tool Details", description: "包含工具参数、MCP 服务器名、原始错误串等细节", type: "boolean", group: "otel" },
  { key: "OTEL_LOG_RAW_API_BODIES", label: "Log Raw API Bodies", description: "把 Messages API 请求/响应 JSON 作为日志事件导出", type: "boolean", group: "otel" },
  { key: "OTEL_METRICS_INCLUDE_SESSION_ID", label: "Metrics Include Session ID", description: "指标属性含 session ID（默认含，设 false 排除）", type: "boolean", group: "otel" },
  { key: "OTEL_METRICS_INCLUDE_ACCOUNT_UUID", label: "Metrics Include Account UUID", description: "指标属性含账号 UUID（默认含）", type: "boolean", group: "otel" },
  { key: "OTEL_METRICS_INCLUDE_VERSION", label: "Metrics Include Version", description: "指标属性含 Claude Code 版本（默认不含）", type: "boolean", group: "otel" },
  { key: "OTEL_METRICS_INCLUDE_ENTRYPOINT", label: "Metrics Include Entrypoint", description: "指标属性含会话入口（默认不含）", type: "boolean", group: "otel" },
  { key: "OTEL_METRICS_INCLUDE_RESOURCE_ATTRIBUTES", label: "Metrics Include Resource Attrs", description: "把 OTEL_RESOURCE_ATTRIBUTES 附到指标标签（默认含）", type: "boolean", group: "otel" },
  { key: "OTEL_ATTRIBUTE_VALUE_LENGTH_LIMIT", label: "Attribute Value Length Limit", description: "OTel SDK 属性值长度上限", type: "number", group: "otel" },
  { key: "CLAUDE_CODE_OTEL_CONTENT_MAX_LENGTH", label: "OTel Content Max Length", description: "含内容的遥测属性的最大长度（含截断标记）", type: "number", group: "otel" },
  { key: "CLAUDE_CODE_OTEL_FLUSH_TIMEOUT_MS", label: "OTel Flush Timeout (ms)", description: "刷写待发送 span 的超时（默认 5000）", type: "number", group: "otel" },
  { key: "CLAUDE_CODE_OTEL_SHUTDOWN_TIMEOUT_MS", label: "OTel Shutdown Timeout (ms)", description: "退出时导出器收尾超时（默认 2000）", type: "number", group: "otel" },
  { key: "CLAUDE_CODE_OTEL_HEADERS_HELPER_DEBOUNCE_MS", label: "OTel Headers Refresh (ms)", description: "动态 OTel 请求头刷新间隔（默认 1740000）", type: "number", group: "otel" },
  { key: "CLAUDE_CODE_OTEL_DIAG_STDERR", label: "OTel Diagnostics To Stderr", description: "把导出器诊断错误写到 stderr", type: "boolean", group: "otel" },
  { key: "CLAUDE_CODE_ENABLE_FEEDBACK_SURVEY_FOR_OTEL", label: "Feedback Survey Via OTel", description: "把会话质量问卷改投到自建 OTel 采集器", type: "boolean", group: "otel" },
  { key: "DO_NOT_TRACK", label: "Do Not Track", description: "退出遥测，等价 DISABLE_TELEMETRY", type: "boolean", group: "otel" },

  // ── MCP ──
  { key: "MCP_TIMEOUT", label: "MCP Startup Timeout (ms)", description: "MCP 服务器启动超时（默认 30000）", type: "number", group: "mcp" },
  { key: "MCP_TOOL_TIMEOUT", label: "MCP Tool Timeout (ms)", description: "MCP 工具执行超时", type: "number", group: "mcp" },
  { key: "MCP_CONNECT_TIMEOUT_MS", label: "MCP Connect Timeout (ms)", description: "阻塞式启动等待连接批次的超时（默认 5000）", type: "number", group: "mcp" },
  { key: "MCP_CONNECTION_NONBLOCKING", label: "MCP Nonblocking Startup", description: "启动是否等待 MCP 服务器连接完成（默认非阻塞）", type: "boolean", group: "mcp" },
  { key: "MCP_SERVER_CONNECTION_BATCH_SIZE", label: "MCP Local Batch Size", description: "启动时并行连接的本地 stdio 服务器数（默认 3）", type: "number", group: "mcp" },
  { key: "MCP_REMOTE_SERVER_CONNECTION_BATCH_SIZE", label: "MCP Remote Batch Size", description: "启动时并行连接的远端 HTTP/SSE 服务器数（默认 20）", type: "number", group: "mcp" },
  { key: "MCP_DISCOVERY_CACHE", label: "MCP Discovery Cache", description: "设 0 关闭跨进程 MCP 发现缓存", type: "boolean", group: "mcp" },
  { key: "MCP_SDK_GENERATION", label: "MCP SDK Generation", description: "固定使用的 MCP 客户端运行时代数", type: "string", group: "mcp" },
  { key: "MCP_OAUTH_CALLBACK_PORT", label: "MCP OAuth Callback Port", description: "OAuth 回调固定端口", type: "number", group: "mcp" },
  { key: "MCP_CLIENT_SECRET", label: "MCP Client Secret", description: "需预置凭证的 MCP 服务器的 OAuth client secret", type: "password", group: "mcp" },
  { key: "CLAUDE_CODE_MCP_ALLOWLIST_ENV", label: "MCP Env Allowlist", description: "stdio MCP 服务器只继承安全基线环境 + 自身 env", type: "boolean", group: "mcp" },
  { key: "CLAUDE_CODE_MCP_AUTO_BACKGROUND_MS", label: "MCP Auto Background (ms)", description: "MCP 工具调用转后台任务的时限（默认 120000，0 关闭）", type: "number", group: "mcp" },
  { key: "CLAUDE_CODE_MCP_TOOL_IDLE_TIMEOUT", label: "MCP Tool Idle Timeout (ms)", description: "MCP 工具调用的空闲超时", type: "number", group: "mcp" },
  { key: "CLAUDE_AGENT_SDK_MCP_NO_PREFIX", label: "SDK MCP No Prefix", description: "SDK 创建的 MCP 工具名去掉 mcp__<server>__ 前缀", type: "boolean", group: "mcp" },
  { key: "ENABLE_CLAUDEAI_MCP_SERVERS", label: "Enable claude.ai MCP Servers", description: "启用 claude.ai MCP 服务器（登录用户默认开）", type: "boolean", group: "mcp" },

  // ── Plugins / Skills ──
  { key: "CLAUDE_CODE_PLUGIN_CACHE_DIR", label: "Plugin Root Dir", description: "覆盖插件根目录（市场与插件缓存都在其下）", type: "string", group: "plugins" },
  { key: "CLAUDE_CODE_PLUGIN_SEED_DIR", label: "Plugin Seed Dir", description: "只读插件种子目录，多个用 : 或 ; 分隔", type: "string", group: "plugins" },
  { key: "CLAUDE_CODE_PLUGIN_GIT_TIMEOUT_MS", label: "Plugin Git Timeout (ms)", description: "安装/更新插件时 git 操作超时（默认 120000）", type: "number", group: "plugins" },
  { key: "CLAUDE_CODE_PLUGIN_PREFER_HTTPS", label: "Plugin Prefer HTTPS", description: "owner/repo 简写用 HTTPS 而非 SSH 克隆", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_PLUGIN_KEEP_MARKETPLACE_ON_FAILURE", label: "Keep Marketplace On Failure", description: "git pull 失败时保留现有市场缓存，不重新克隆", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL", label: "Disable Official Marketplace Autoinstall", description: "不自动注册官方插件市场", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_ENABLE_BACKGROUND_PLUGIN_REFRESH", label: "Background Plugin Refresh", description: "非交互模式下后台安装完成后在回合边界刷新插件状态", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_SYNC_PLUGIN_INSTALL", label: "Sync Plugin Install", description: "非交互模式首个查询前等待插件安装完成", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_SYNC_PLUGIN_INSTALL_TIMEOUT_MS", label: "Sync Plugin Install Timeout (ms)", description: "同步安装插件的超时，超时后无插件继续", type: "number", group: "plugins" },
  { key: "CLAUDE_CODE_SYNC_SKILLS", label: "Sync claude.ai Skills", description: "把 claude.ai 上启用的 skill 同步到 ~/.claude/skills/synced/", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_SYNC_SKILLS_INSTALL_TIMEOUT_MS", label: "Sync Skills Install Timeout (ms)", description: "会话中 skill 重新同步的超时（默认 30000）", type: "number", group: "plugins" },
  { key: "CLAUDE_CODE_SYNC_SKILLS_WAIT_TIMEOUT_MS", label: "Sync Skills Wait Timeout (ms)", description: "首个查询等待初始 skill 列表的超时（默认 5000）", type: "number", group: "plugins" },
  { key: "CLAUDE_CODE_DISABLE_BUNDLED_SKILLS", label: "Disable Bundled Skills", description: "禁用随 Claude Code 附带的 skill 与 workflow", type: "boolean", group: "plugins" },
  { key: "CLAUDE_CODE_DISABLE_POLICY_SKILLS", label: "Disable Policy Skills", description: "不加载系统级托管 skill 目录", type: "boolean", group: "plugins" },
  { key: "SLASH_COMMAND_TOOL_CHAR_BUDGET", label: "Slash Command Char Budget", description: "Skill 工具可见的 skill 元数据字符预算", type: "number", group: "plugins" },

  // ── IDE / 终端集成 ──
  { key: "CLAUDE_CODE_AUTO_CONNECT_IDE", label: "Auto Connect IDE", description: "覆盖 IDE 自动连接行为", type: "boolean", group: "ide" },
  { key: "CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL", label: "Skip IDE Extension Install", description: "跳过 IDE 扩展自动安装", type: "boolean", group: "ide" },
  { key: "CLAUDE_CODE_IDE_SKIP_VALID_CHECK", label: "Skip IDE Lockfile Check", description: "连接时跳过 IDE lockfile 校验", type: "boolean", group: "ide" },
  { key: "CLAUDE_CODE_IDE_HOST_OVERRIDE", label: "IDE Host Override", description: "覆盖连接 IDE 扩展使用的主机地址", type: "string", group: "ide" },
  { key: "CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST", label: "Provider Managed By Host", description: "由宿主平台接管模型 provider 路由（宿主设置）", type: "boolean", group: "ide" },

  // ── Network / TLS（续） ──
  { key: "CLAUDE_CODE_CERT_STORE", label: "CA Cert Store", description: "TLS 使用的 CA 证书来源，逗号分隔：bundled / system", type: "string", placeholder: "bundled,system", group: "network" },
  { key: "CLAUDE_CODE_CLIENT_CERT", label: "Client Certificate", description: "mTLS 客户端证书文件路径", type: "string", group: "network" },
  { key: "CLAUDE_CODE_CLIENT_KEY", label: "Client Key", description: "mTLS 客户端私钥文件路径", type: "string", group: "network" },
  { key: "CLAUDE_CODE_CLIENT_KEY_PASSPHRASE", label: "Client Key Passphrase", description: "加密客户端私钥的口令", type: "password", group: "network" },
  { key: "CLAUDE_CODE_DISABLE_MTLS_RELOAD_ON_STALE_CONNECTION", label: "Disable mTLS Reload", description: "连接层错误时不重新读取 mTLS 证书与私钥", type: "boolean", group: "network" },
  { key: "CLAUDE_CODE_PROXY_RESOLVES_HOSTS", label: "Proxy Resolves Hosts", description: "由代理执行 DNS 解析而非本地解析", type: "boolean", group: "network" },
  { key: "CLAUDE_CODE_PROPAGATE_TRACEPARENT", label: "Propagate Traceparent", description: "自定义代理场景下透传 W3C trace context", type: "boolean", group: "network" },
  { key: "CLAUDE_CODE_EXTRA_BODY", label: "Extra Request Body", description: "合并进每个 API 请求体顶层的 JSON 对象", type: "string", group: "network" },

  // ── Performance & Limits（续） ──
  { key: "API_FORCE_IDLE_TIMEOUT", label: "API Idle Timeout (ms)", description: "覆盖 5 分钟流式响应空闲中断，0 = 关闭", type: "number", group: "performance" },
  { key: "CLAUDE_BYTE_STREAM_IDLE_TIMEOUT_MS", label: "Byte Stream Idle Timeout (ms)", description: "字节级流式空闲看门狗超时", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS", label: "Max Concurrent Subagents", description: "同一会话并发 subagent 上限（默认 20）", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH", label: "Max Subagent Spawn Depth", description: "subagent 嵌套层数上限（默认 3）", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_MAX_SUBAGENTS_PER_SESSION", label: "Max Subagents Per Session (no-op)", description: "v2.1.224 起为空操作，原为单会话 subagent 总数上限", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_MAX_WEB_SEARCHES_PER_SESSION", label: "Max Web Searches", description: "单会话 WebSearch 调用上限（默认 200）", type: "number", group: "performance" },
  { key: "CLAUDE_ASYNC_AGENT_STALL_TIMEOUT_MS", label: "Async Agent Stall Timeout (ms)", description: "后台 subagent 停滞超时（默认 600000）", type: "number", group: "performance" },
  { key: "CLAUDE_SUBAGENT_BG_SHELL_MAX_MS", label: "Subagent Bg Shell Max (ms)", description: "subagent 后台 shell 命令的最长存活时间（默认 3600000）", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS", label: "Print Mode Bg Wait (ms)", description: "-p 模式末回合后等待后台任务的上限", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_TEAM_TEARDOWN_PARK_TIMEOUT_MS", label: "Team Teardown Timeout (ms)", description: "退出时等待 agent team 拆除的时长（1000–60000）", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_SESSIONEND_HOOKS_TIMEOUT_MS", label: "SessionEnd Hooks Timeout (ms)", description: "SessionEnd 钩子的时间预算", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_STOP_HOOK_BLOCK_CAP", label: "Stop Hook Block Cap", description: "Stop/SubagentStop 钩子连续阻断回合的次数上限", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_GLOB_TIMEOUT_SECONDS", label: "Glob Timeout (s)", description: "Glob 工具文件发现超时（默认 20s，WSL 60s）", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_WEBFETCH_CACHE_TTL_MS", label: "WebFetch Cache TTL (ms)", description: "WebFetch 结果缓存时长（默认 900000）", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_TOOL_MEMORY_LIMIT", label: "Tool Memory Limit", description: "Linux/WSL 上限制 Bash/PowerShell 命令内存，如 4G", type: "string", placeholder: "4G", group: "performance" },
  { key: "CLAUDE_CODE_SCRIPT_CAPS", label: "Script Invocation Caps", description: "限制特定脚本每会话调用次数的 JSON 对象", type: "string", group: "performance" },
  { key: "CLAUDE_CODE_SCROLL_SPEED", label: "Scroll Speed", description: "全屏渲染下的滚轮倍率（最大 20，可小于 1）", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_GOAL_CHECKIN_MINUTES", label: "Goal Check-in (min)", description: "后台工作阻塞目标多久后触发检查（默认 30，0 关闭）", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_WORKFLOW_PREFIX_STAGGER_MS", label: "Workflow Stagger (ms)", description: "同前缀 workflow agent 首次请求的错峰上限", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_RESUME_INTERRUPTED_TURN_MAX_AGE_MS", label: "Resume Turn Max Age (ms)", description: "断点续跑允许的最后一条消息最大年龄", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_EXIT_AFTER_STOP_DELAY", label: "Exit After Stop Delay (ms)", description: "查询循环空闲后自动退出的等待时间", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_USER_DIALOG_TIMEOUT_MS", label: "User Dialog Timeout (ms)", description: "转发给远程客户端的对话框应答期限", type: "number", group: "performance" },
  { key: "CLAUDE_AFK_TIMEOUT_MS", label: "AFK Timeout (ms)", description: "AskUserQuestion 无人应答自动继续的空闲时长", type: "number", group: "performance" },
  { key: "CLAUDE_AFK_COUNTDOWN_MS", label: "AFK Countdown (ms)", description: "自动继续前显示倒计时的提前量（默认 20000）", type: "number", group: "performance" },
  { key: "CLAUDE_AX_PREPARK_MS", label: "Screen Reader Prepark (ms)", description: "屏幕阅读器模式下写新行前的等待", type: "number", group: "performance" },
  { key: "CLAUDE_AX_STARTUP_QUIET_MS", label: "Screen Reader Startup Quiet (ms)", description: "屏幕阅读器模式下首屏渲染的延后时长", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_CONNECT_TIMEOUT_MS", label: "Connect Timeout (ms, no-op)", description: "v2.1.186 起为空操作", type: "number", group: "performance" },
  { key: "CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT", label: "Skip Unknown Model Window Enforcement", description: "模型 ID 不识别时跳过主动自动压缩", type: "boolean", group: "performance" },
  { key: "CLAUDE_CODE_API_KEY_HELPER_TTL_MS", label: "API Key Helper TTL (ms)", description: "apiKeyHelper 凭证刷新间隔", type: "number", group: "performance" },

  // ── Feature Toggles（续） ──
  { key: "CLAUDE_CODE_DISABLE_ADVISOR_TOOL", label: "Disable Advisor Tool", description: "禁用 advisor 工具与 /advisor 命令", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_AGENT_VIEW", label: "Disable Agent View", description: "关闭后台 agent 与 agent view", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_ARTIFACT", label: "Disable Artifact", description: "禁用 Artifact 工具", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_CLAUDE_MDS", label: "Disable CLAUDE.md", description: "不加载任何 CLAUDE.md 记忆文件", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_EXPLORE_PLAN_AGENTS", label: "Disable Explore/Plan Agents", description: "禁用内置 Explore 与 Plan subagent", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS", label: "Disable Experimental Betas", description: "剥离 anthropic-beta 请求头与 beta 工具字段", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY", label: "Disable Feedback Survey", description: "关闭会话质量问卷", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_LEGACY_MODEL_REMAP", label: "Disable Legacy Model Remap", description: "不把 Opus 4.0/4.1 自动重映射到当前 Opus", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK", label: "Disable Non-Streaming Fallback", description: "流式中途失败时不回退到非流式请求", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_NOTIFICATION_PRESENCE_CHECK", label: "Disable Notification Presence Check", description: "即使终端有焦点也发送 PushNotification 桌面通知", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_PERMISSION_PROMPT_NOTIFY_HOOKS", label: "Disable Permission Notify Hooks", description: "远程转发的权限请求不再触发 Notification 钩子", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_MOUSE", label: "Disable Mouse", description: "全屏渲染下禁用鼠标跟踪", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_MOUSE_CLICKS", label: "Disable Mouse Clicks", description: "禁用点击/拖拽/悬停，仅保留滚轮", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_VIRTUAL_SCROLL", label: "Disable Virtual Scroll", description: "全屏渲染下渲染完整记录，不做虚拟滚动", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_BG_EXIT_HANDOFF", label: "Disable Bg Exit Handoff", description: "后台会话退出时一并停止其后台任务", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_BG_SHELL_PRESSURE_REAP", label: "Disable Bg Shell Reap", description: "内存压力时不终止后台 shell 命令", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_DISABLE_ADMIN_ENV_UNION", label: "Disable Admin Env Union", description: "不跨管理源按键合并 env 块", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ALWAYS_ENABLE_EFFORT", label: "Always Send Effort", description: "对所有模型都发送 effort 参数", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_APPEND_SUBAGENT_PROMPT", label: "Append Subagent Prompt", description: "允许给非 fork subagent 的系统提示追加文本", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_AUTO_MODE", label: "Enable Auto Mode (no-op)", description: "兼容保留，已无效果：auto 模式默认可用", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_AWAY_SUMMARY", label: "Away Summary Override", description: "强制开启或关闭离开回顾", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING", label: "Fine-Grained Tool Streaming", description: "工具入参随生成流式返回", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION", label: "Prompt Suggestions", description: "输入框灰色预测建议开关", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_TASKS", label: "Enable Task Tools", description: "选择提供哪套任务跟踪工具", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_TODO_TOOLS", label: "Enable Todo Tools", description: "在默认不带的模型上启用任务跟踪工具", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_ENABLE_OPUS_4_7_FAST_MODE", label: "Opus 4.7 Fast Mode (removed)", description: "v2.1.142 已移除", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_OPUS_4_6_FAST_MODE_OVERRIDE", label: "Opus 4.6 Fast Mode (removed)", description: "v2.1.160 已移除，空操作", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_FORCE_SESSION_PERSISTENCE", label: "Force Session Persistence", description: "嵌套启动时也强制持久化记录与提示历史", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_FORCE_STRIKETHROUGH", label: "Force Strikethrough", description: "强制渲染 ~~删除线~~", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_FORCE_SYNC_OUTPUT", label: "Force Synchronized Output", description: "强制启用 DEC 2026 同步输出", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_PERFORCE_MODE", label: "Perforce Mode", description: "启用 Perforce 写保护：只读文件提示 p4 edit", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_USE_NATIVE_FILE_SEARCH", label: "Native File Search", description: "用 Node 文件 API 代替 ripgrep 发现命令与 agent", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_USE_POWERSHELL_TOOL", label: "PowerShell Tool", description: "控制 PowerShell 工具的启用", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_FORK_SUBAGENT", label: "Fork Subagent", description: "允许 Claude 自行派生 fork subagent", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_FORWARD_SUBAGENT_TEXT", label: "Forward Subagent Text", description: "stream-json 输出中带上 subagent 文本与思考块", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_RESUME_INTERRUPTED_TURN", label: "Resume Interrupted Turn", description: "上次会话中途结束时自动续跑", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_RETRY_WATCHDOG", label: "Retry Watchdog", description: "无人值守场景无限重试 429/529 容量错误", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_PACKAGE_MANAGER_AUTO_UPDATE", label: "Package Manager Auto Update", description: "允许后台执行包管理器升级命令", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", label: "Subprocess Env Scrub", description: "从子进程环境剥离 Anthropic 与云厂商凭证", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_SKIP_FAST_MODE_ORG_CHECK", label: "Skip Fast Mode Org Check", description: "跳过客户端快速模式可用性检查", type: "boolean", group: "toggles" },
  { key: "CLAUDE_CODE_SKIP_FAST_MODE_NETWORK_ERRORS", label: "Skip Fast Mode Network Errors", description: "可用性检查失败时按可用处理", type: "boolean", group: "toggles" },
  { key: "CLAUDE_DISABLE_ADOPT", label: "Disable Adopt", description: "转后台时停止在途工作而非接管", type: "boolean", group: "toggles" },
  { key: "CLAUDE_ENABLE_BYTE_WATCHDOG", label: "Byte Watchdog", description: "强制开/关字节级流式空闲看门狗", type: "boolean", group: "toggles" },
  { key: "CLAUDE_ENABLE_STREAM_WATCHDOG", label: "Stream Watchdog", description: "强制开/关事件级流式空闲看门狗", type: "boolean", group: "toggles" },
  { key: "CLAUDE_AGENT_SDK_DISABLE_BUILTIN_AGENTS", label: "SDK Disable Builtin Agents", description: "非交互模式下禁用 Explore / Plan 等内置 subagent", type: "boolean", group: "toggles" },
  { key: "CCR_FORCE_BUNDLE", label: "Force Cloud Bundle", description: "claude --cloud 强制打包上传本地仓库", type: "boolean", group: "toggles" },
  { key: "DISABLE_DOCTOR_COMMAND", label: "Disable /doctor", description: "隐藏 /doctor 与 /checkup", type: "boolean", group: "toggles" },
  { key: "DISABLE_FEEDBACK_COMMAND", label: "Disable /feedback", description: "禁用 /feedback（含 /bug 与 /share）", type: "boolean", group: "toggles" },
  { key: "DISABLE_UPGRADE_COMMAND", label: "Disable /upgrade", description: "隐藏 /upgrade 命令", type: "boolean", group: "toggles" },
  { key: "DISABLE_EXTRA_USAGE_COMMAND", label: "Disable /usage-credits", description: "隐藏购买额外用量的命令", type: "boolean", group: "toggles" },
  { key: "DISABLE_INSTALL_GITHUB_APP_COMMAND", label: "Disable /install-github-app", description: "隐藏 /install-github-app 命令", type: "boolean", group: "toggles" },
  { key: "DISABLE_PROMPT_CACHING_OPUS", label: "Disable Prompt Caching (Opus)", description: "对 Opus 系列关闭 prompt 缓存", type: "boolean", group: "toggles" },
  { key: "DISABLE_PROMPT_CACHING_SONNET", label: "Disable Prompt Caching (Sonnet)", description: "对 Sonnet 系列关闭 prompt 缓存", type: "boolean", group: "toggles" },
  { key: "DISABLE_PROMPT_CACHING_HAIKU", label: "Disable Prompt Caching (Haiku)", description: "对 Haiku 系列关闭 prompt 缓存", type: "boolean", group: "toggles" },
  { key: "DISABLE_PROMPT_CACHING_FABLE", label: "Disable Prompt Caching (Fable)", description: "对 Fable 系列关闭 prompt 缓存", type: "boolean", group: "toggles" },
  { key: "FORCE_PROMPT_CACHING_5M", label: "Force 5m Prompt Cache", description: "强制 5 分钟缓存 TTL，覆盖 1 小时设置", type: "boolean", group: "toggles" },
  { key: "FORCE_HYPERLINK", label: "Force Hyperlinks", description: "强制开/关 OSC 8 可点击链接", type: "boolean", group: "toggles" },
  { key: "USE_BUILTIN_RIPGREP", label: "Use Builtin Ripgrep", description: "设 0 改用系统安装的 rg", type: "boolean", group: "toggles" },

  // ── Auth / 凭证 ──
  { key: "CLAUDE_CODE_OAUTH_TOKEN", label: "OAuth Token", description: "claude.ai OAuth 访问令牌，替代 /login", type: "password", group: "auth" },
  { key: "CLAUDE_CODE_OAUTH_REFRESH_TOKEN", label: "OAuth Refresh Token", description: "claude.ai OAuth 刷新令牌，登录时直接兑换", type: "password", group: "auth" },
  { key: "CLAUDE_CODE_OAUTH_SCOPES", label: "OAuth Scopes", description: "刷新令牌签发时的 scope，空格分隔", type: "string", group: "auth" },

  // ── Session / 运行时（多数由 Claude Code 自动注入） ──
  { key: "CLAUDE_CONFIG_DIR", label: "Config Dir", description: "覆盖配置目录（默认 ~/.claude）", type: "string", group: "session" },
  { key: "CLAUDE_CODE_PROJECT_DIR_NAME", label: "Project Dir Name", description: "与 CLAUDE_CONFIG_DIR 配套，指定 projects/ 下的目录名", type: "string", group: "session" },
  { key: "CLAUDE_CODE_TMPDIR", label: "Temp Dir", description: "覆盖内部临时文件目录", type: "string", group: "session" },
  { key: "CLAUDE_CODE_SESSION_ID", label: "Session ID (auto)", description: "自动注入：子进程中的当前会话 ID", type: "string", group: "session" },
  { key: "CLAUDE_CODE_BRIDGE_SESSION_ID", label: "Bridge Session ID (auto)", description: "自动注入：远程控制连接期间的桥接会话 ID", type: "string", group: "session" },
  { key: "CLAUDE_CODE_REMOTE_SESSION_ID", label: "Remote Session ID (auto)", description: "自动注入：云端会话的会话 ID", type: "string", group: "session" },
  { key: "CLAUDE_CODE_REMOTE", label: "Is Cloud Session (auto)", description: "自动注入：云端会话中为 true", type: "boolean", group: "session" },
  { key: "CLAUDE_CODE_CHILD_SESSION", label: "Child Session (auto)", description: "自动注入：Claude Code 派生的子进程中为 1", type: "boolean", group: "session" },
  { key: "CLAUDECODE", label: "In Claude Code (auto)", description: "自动注入：Claude Code 派生的子进程中为 1", type: "boolean", group: "session" },
  { key: "CLAUDE_PID", label: "Claude PID (auto)", description: "自动注入：Claude Code 自身进程 ID", type: "string", group: "session" },
  { key: "CLAUDE_EFFORT", label: "Current Effort (auto)", description: "自动注入：本回合生效的 effort 级别", type: "string", group: "session" },
  { key: "CLAUDE_CODE_MESSAGING_SOCKET", label: "Messaging Socket (auto)", description: "自动注入：会话收件箱 socket 路径", type: "string", group: "session" },
  { key: "CLAUDE_CODE_MESSAGING_TOKEN", label: "Messaging Token (auto)", description: "自动注入：会话收件箱令牌", type: "password", group: "session" },
  { key: "CLAUDE_CODE_TASK_LIST_ID", label: "Shared Task List ID", description: "多个会话共用同一任务列表的 ID", type: "string", group: "session" },
  { key: "CLAUDE_CODE_RESUME_PROMPT", label: "Resume Prompt", description: "断点续跑时注入的续跑提示语", type: "string", group: "session" },
  { key: "CLAUDE_CODE_SKIP_PROMPT_HISTORY", label: "Skip Prompt History", description: "不把提示历史与会话记录写盘", type: "boolean", group: "session" },
  { key: "CLAUDE_CODE_SAFE_MODE", label: "Safe Mode", description: "安全模式启动：不加载 CLAUDE.md / skill / 插件 / 钩子 / MCP 等", type: "boolean", group: "session" },
  { key: "CLAUDE_CODE_SIMPLE", label: "Simple Mode", description: "最小系统提示，仅保留 Bash 与读写文件工具", type: "boolean", group: "session" },
  { key: "CLAUDE_CODE_SIMPLE_SYSTEM_PROMPT", label: "Simple System Prompt", description: "使用更短的系统提示与精简工具描述", type: "boolean", group: "session" },
  { key: "CLAUDE_CODE_NEW_INIT", label: "Interactive /init", description: "让 /init 走交互式初始化流程", type: "boolean", group: "session" },
  { key: "CLAUDE_CLIENT_PRESENCE_FILE", label: "Client Presence File", description: "外部工具维护的在场标记文件路径（锁屏检测）", type: "string", group: "session" },
  { key: "CLAUDE_REMOTE_CONTROL_SESSION_NAME_PREFIX", label: "Remote Control Name Prefix", description: "自动生成的远程控制会话名前缀（默认主机名）", type: "string", group: "session" },
  { key: "CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD", label: "Load CLAUDE.md From --add-dir", description: "从 --add-dir 指定目录加载记忆文件", type: "boolean", group: "session" },

  // ── Shell / 终端 ──
  { key: "CLAUDE_CODE_SHELL", label: "Shell Binary", description: "Bash 工具使用的 shell 路径（bash 或 zsh）", type: "string", placeholder: "/opt/homebrew/bin/bash", group: "shell" },
  { key: "CLAUDE_CODE_SHELL_PREFIX", label: "Shell Prefix", description: "包裹所有派生 shell 命令的前缀命令", type: "string", group: "shell" },
  { key: "CLAUDE_ENV_FILE", label: "Env File", description: "每条 Bash 命令前在同一 shell 进程内执行的脚本路径", type: "string", group: "shell" },
  { key: "CLAUDE_CODE_GIT_BASH_PATH", label: "Git Bash Path", description: "Windows：Git Bash 的 bash.exe 路径", type: "string", group: "shell" },
  { key: "CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR", label: "Keep Project Working Dir", description: "每条 Bash 命令后回到原工作目录", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_POWERSHELL_RESPECT_EXECUTION_POLICY", label: "Respect PowerShell Policy", description: "不再传 -ExecutionPolicy Bypass", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_PROCESS_WRAPPER", label: "Process Wrapper", description: "包裹 Claude Code 派生后台进程的企业启动器命令", type: "string", group: "shell" },
  { key: "CLAUDE_CODE_GLOB_HIDDEN", label: "Glob Includes Hidden", description: "Glob 结果是否含 dotfile（默认含，设 false 排除）", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_GLOB_NO_IGNORE", label: "Glob Ignores .gitignore", description: "设 false 让 Glob 遵守 .gitignore", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_TMUX_TRUECOLOR", label: "Tmux Truecolor", description: "允许在 tmux 内输出 24 位真彩", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_ACCESSIBILITY", label: "Native Cursor (a11y)", description: "保留终端原生光标，便于屏幕放大镜跟随", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_NATIVE_CURSOR", label: "Native Cursor", description: "在输入插入点显示终端自身光标而非绘制方块", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_NO_FLICKER", label: "Fullscreen Rendering", description: "启用减少闪烁的全屏渲染", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_ALT_SCREEN_FULL_REPAINT", label: "Alt Screen Full Repaint", description: "全屏渲染每帧整屏重绘", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_SYNTAX_HIGHLIGHT", label: "Syntax Highlight", description: "设 false 关闭 diff 输出的语法高亮", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_HIDE_CWD", label: "Hide CWD", description: "启动 logo 中隐藏工作目录（录屏用）", type: "boolean", group: "shell" },
  { key: "CLAUDE_CODE_ARTIFACT_AUTO_OPEN", label: "Artifact Auto Open", description: "发布新 artifact 时自动打开浏览器（设 0 关闭）", type: "boolean", group: "shell" },
  { key: "CLAUDE_AX_SCREEN_READER", label: "Screen Reader Mode", description: "输出扁平文本，无装饰边框与动画", type: "boolean", group: "shell" },

  // ── Debug ──
  { key: "CLAUDE_CODE_DEBUG_LOGS_DIR", label: "Debug Log File", description: "覆盖调试日志文件路径（需同时开启调试模式）", type: "string", group: "debug" },
  { key: "CLAUDE_CODE_DEBUG_LOG_LEVEL", label: "Debug Log Level", description: "调试日志最低级别", type: "select", options: ["verbose", "debug", "info", "warn", "error"], group: "debug" },
  { key: "DISABLE_GROWTHBOOK", label: "Disable Feature Flags", description: "关闭 GrowthBook 特性开关拉取，全部用代码默认值", type: "boolean", group: "debug" },
  { key: "DISABLE_INSTALLATION_CHECKS", label: "Disable Installation Checks", description: "关闭安装位置告警", type: "boolean", group: "debug" },
  { key: "IS_DEMO", label: "Demo Mode", description: "演示模式：隐藏邮箱与组织名", type: "boolean", group: "debug" },
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

// 单一事实源：派生自后端内置默认 src-tauri/defaults/settings.json，避免前后端两份默认漂移。
// 仅 language 用运行时检测覆盖（JSON 内固定为 zh-Hans）。
export const RECOMMENDED_CONFIG: Record<string, any> = {
  ...DEFAULT_SETTINGS,
  language: detectLanguage(),
};
