// OpenAI Codex CLI 配置 schema（~/.codex/config.toml）— 对照 claude-settings-schema.ts。
// 权威来源：research/codex-config.md（5 篇官方文档抽取）。
// D1 范围：全局设置子集（Core / Reasoning / Approval & Sandbox / Model Providers /
//          MCP / TUI / Web Search / Features / Diagnostics）。复杂嵌套用 json 类型兜底。
//
// 字段类型复用 components/settings/editors.tsx 的 FieldRenderer：
//   string / boolean / select / json / string[] / kv。

import type { SettingField, SettingSection } from "./claude-settings-schema";

/** aidog 本地代理默认端口（与 load_proxy_settings 内置默认一致）。 */
export const AIDOG_DEFAULT_PORT = 9890;

// ── Sections ──

export const CODEX_SECTIONS: SettingSection[] = [
  {
    id: "core",
    labelKey: "codex.sectionCore",
    fields: [
      { key: "model", label: "Model", type: "string", placeholder: "e.g. gpt-5.5" },
      { key: "model_provider", label: "Model Provider", type: "string", placeholder: "aidog", description: "Provider id chosen from [model_providers]" },
      { key: "oss_provider", label: "OSS Provider", type: "select", options: ["lmstudio", "ollama"], description: "Default local provider for --oss" },
      { key: "service_tier", label: "Service Tier", type: "select", options: ["flex", "fast"] },
      { key: "review_model", label: "Review Model", type: "string", description: "Model override for /review" },
      { key: "model_context_window", label: "Context Window (tokens)", type: "string", placeholder: "auto" },
      { key: "model_auto_compact_token_limit", label: "Auto Compact Token Limit", type: "string" },
    ],
  },
  {
    id: "reasoning",
    labelKey: "codex.sectionReasoning",
    fields: [
      { key: "model_reasoning_effort", label: "Reasoning Effort", type: "select", options: ["minimal", "low", "medium", "high", "xhigh"], description: "Responses API only" },
      { key: "plan_mode_reasoning_effort", label: "Plan Mode Reasoning Effort", type: "select", options: ["none", "minimal", "low", "medium", "high", "xhigh"] },
      { key: "model_reasoning_summary", label: "Reasoning Summary", type: "select", options: ["auto", "concise", "detailed", "none"] },
      { key: "model_verbosity", label: "Verbosity", type: "select", options: ["low", "medium", "high"], description: "GPT-5 Responses text verbosity" },
    ],
  },
  {
    id: "approval",
    labelKey: "codex.sectionApproval",
    fields: [
      { key: "approval_policy", label: "Approval Policy", type: "select", options: ["untrusted", "on-request", "never"], description: "When to pause and ask for approval" },
      { key: "sandbox_mode", label: "Sandbox Mode", type: "select", options: ["read-only", "workspace-write", "danger-full-access"] },
      { key: "approvals_reviewer", label: "Approvals Reviewer", type: "select", options: ["user", "auto_review"] },
      { key: "allow_login_shell", label: "Allow Login Shell", type: "boolean" },
      { key: "sandbox_workspace_write", label: "Workspace-Write Sandbox", type: "json", description: "{ writable_roots, network_access, exclude_tmpdir_env_var, exclude_slash_tmp }" },
    ],
  },
  {
    id: "providers",
    labelKey: "codex.sectionProviders",
    fields: [
      { key: "openai_base_url", label: "OpenAI Base URL Override", type: "string", placeholder: "http://127.0.0.1:9890/proxy", description: "Lightweight option: override the built-in openai provider base URL directly" },
      { key: "chatgpt_base_url", label: "ChatGPT Base URL", type: "string" },
      { key: "model_providers", label: "Model Providers", type: "json", description: "[model_providers.<id>] custom providers (including the aidog proxy). Each entry is { name, base_url, wire_api, env_key, ... }" },
    ],
  },
  {
    id: "mcp",
    labelKey: "codex.sectionMcp",
    fields: [
      { key: "mcp_servers", label: "MCP Servers", type: "json", description: "[mcp_servers.<id>] — stdio (command/args/env) or HTTP (url/...)" },
      { key: "mcp_oauth_credentials_store", label: "MCP OAuth Store", type: "select", options: ["auto", "file", "keyring"] },
    ],
  },
  {
    id: "tui",
    labelKey: "codex.sectionTui",
    fields: [
      { key: "tui", label: "TUI Settings", type: "json", description: "{ notifications, theme, vim_mode_default, status_line, terminal_title, ... }" },
    ],
  },
  {
    id: "search",
    labelKey: "codex.sectionSearch",
    fields: [
      { key: "web_search", label: "Web Search", type: "select", options: ["disabled", "cached", "live"], description: "cached = pre-indexed, injection-resistant; live = real time" },
      { key: "personality", label: "Personality", type: "select", options: ["none", "friendly", "pragmatic"] },
    ],
  },
  {
    id: "features",
    labelKey: "codex.sectionFeatures",
    fields: [
      { key: "features", label: "Features", type: "json", description: "[features] — apps / hooks / fast_mode / multi_agent / undo / network_proxy ..." },
    ],
  },
  {
    id: "shell",
    labelKey: "codex.sectionShell",
    fields: [
      { key: "shell_environment_policy", label: "Shell Environment Policy", type: "json", description: "{ inherit, include_only, exclude, set, ignore_default_excludes }" },
      { key: "history", label: "History", type: "json", description: "{ persistence: save-all|none, max_bytes }" },
    ],
  },
  {
    id: "diagnostics",
    labelKey: "codex.sectionDiagnostics",
    fields: [
      { key: "file_opener", label: "File Opener", type: "string", placeholder: "vscode" },
      { key: "check_for_update_on_startup", label: "Check Update On Startup", type: "boolean" },
      { key: "analytics", label: "Analytics", type: "json", description: "{ enabled }" },
      { key: "feedback", label: "Feedback", type: "json", description: "{ enabled }" },
    ],
  },
];

/** All known top-level keys covered by the schema. */
export const ALL_CODEX_KEYS: string[] = CODEX_SECTIONS.flatMap((s) => s.fields.map((f) => f.key));

// 复用 Claude Code schema 的字段/分区接口类型。
export type { SettingField, SettingSection };
