import { invoke } from "@tauri-apps/api/core";

// ─── Types ─────────────────────────────────────────────────

export type Protocol = "anthropic" | "openai" | "glm" | "kimi" | "minimax" | "codex" | "claude_code";
export type RoutingMode = "load_balance" | "failover";

export type ModelSlot = "default" | "sonnet" | "opus" | "haiku" | "gpt";

export interface PlatformModels {
  default?: string;
  sonnet?: string;
  opus?: string;
  haiku?: string;
  gpt?: string;
}

export interface Platform {
  id: string;
  name: string;
  protocol: Protocol;
  base_url: string;
  api_key: string;
  extra: string | null;
  models: PlatformModels;
  available_models: string[];
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface Group {
  id: string;
  name: string;
  path: string;
  routing_mode: RoutingMode;
  /** 关联的平台 ID（自动创建的分组） */
  auto_from_platform?: string;
  created_at: string;
  updated_at: string;
  /** 超时设置（秒），0 = 继承系统设置 */
  request_timeout_secs: number;
  connect_timeout_secs: number;
}

export interface GroupPlatformDetail {
  platform: Platform;
  priority: number;
  weight: number;
}

export interface ModelMapping {
  id: string;
  group_id: string;
  source_model: string;
  target_platform_id: string;
  target_model: string;
  /** 超时设置（秒），0 = 继承分组设置 */
  request_timeout_secs: number;
  connect_timeout_secs: number;
}

export interface GroupDetail {
  group: Group;
  platforms: GroupPlatformDetail[];
  model_mappings: ModelMapping[];
}

export interface ProxySettings {
  port: number;
  autostart: boolean;
}

// ─── Platform API ──────────────────────────────────────────

export const platformApi = {
  create: (input: {
    name: string;
    protocol: Protocol;
    base_url: string;
    api_key: string;
    extra?: string;
    models?: PlatformModels;
    available_models?: string[];
  }) => invoke<Platform>("platform_create", { input }),

  list: () => invoke<Platform[]>("platform_list"),

  get: (id: string) => invoke<Platform | null>("platform_get", { id }),

  update: (input: {
    id: string;
    name?: string;
    protocol?: Protocol;
    base_url?: string;
    api_key?: string;
    extra?: string;
    models?: PlatformModels;
    available_models?: string[];
    enabled?: boolean;
  }) => invoke<Platform>("platform_update", { input }),

  delete: (id: string) => invoke<void>("platform_delete", { id }),

  fetchModels: (protocol: Protocol, baseUrl: string, apiKey: string) =>
    invoke<string[]>("platform_fetch_models", { protocol, baseUrl, apiKey }),
};

// ─── Group API ─────────────────────────────────────────────

export const groupApi = {
  create: (input: {
    name: string;
    path: string;
    routing_mode: RoutingMode;
  }) => invoke<Group>("group_create", { input }),

  list: () => invoke<Group[]>("group_list"),

  get: (id: string) => invoke<Group | null>("group_get", { id }),

  update: (input: {
    id: string;
    name?: string;
    path?: string;
    routing_mode?: RoutingMode;
  }) => invoke<Group>("group_update", { input }),

  delete: (id: string) => invoke<void>("group_delete", { id }),

  setPlatforms: (
    groupId: string,
    platforms: { platform_id: string; priority?: number; weight?: number }[]
  ) =>
    invoke<void>("group_set_platforms", {
      input: { group_id: groupId, platforms },
    }),

  getPlatforms: (groupId: string) =>
    invoke<GroupPlatformDetail[]>("group_get_platforms", { groupId }),
};

// ─── Model Mapping API ─────────────────────────────────────

export const mappingApi = {
  create: (input: {
    group_id: string;
    source_model: string;
    target_platform_id: string;
    target_model: string;
  }) => invoke<ModelMapping>("mapping_create", { input }),

  list: (groupId: string) =>
    invoke<ModelMapping[]>("mapping_list", { groupId }),

  update: (input: {
    id: string;
    source_model?: string;
    target_platform_id?: string;
    target_model?: string;
  }) => invoke<ModelMapping>("mapping_update", { input }),

  delete: (id: string) => invoke<void>("mapping_delete", { id }),
};

// ─── Aggregate API ─────────────────────────────────────────

export const groupDetailApi = {
  get: (id: string) =>
    invoke<GroupDetail | null>("group_detail", { id }),

  list: () => invoke<GroupDetail[]>("group_detail_list"),
};

// ─── Proxy API ─────────────────────────────────────────────

export const proxyApi = {
  start: (port: number) => invoke<string>("proxy_start", { port }),
  stop: () => invoke<void>("proxy_stop"),
  status: () => invoke<boolean>("proxy_status"),
  getSettings: () => invoke<ProxySettings>("proxy_get_settings"),
  setAutostart: (enabled: boolean) =>
    invoke<void>("proxy_set_autostart", { enabled }),
};

// ─── Claude Code Config Export ─────────────────────────────

export const configApi = {
  exportClaudeConfig: (port: number) =>
    invoke<string>("export_claude_config", { port }),
  syncGroupSettings: () =>
    invoke<string[]>("sync_group_settings"),
};

// ─── Proxy Log Types ───────────────────────────────────────

export interface ProxyLogSummary {
  id: string;
  group_name: string;
  model: string;
  actual_model: string;
  source_protocol: string;
  target_protocol: string;
  status_code: number;
  duration_ms: number;
  input_tokens: number;
  output_tokens: number;
  created_at: string;
}

export interface ProxyLogDetail {
  id: string;
  group_name: string;
  model: string;
  actual_model: string;
  source_protocol: string;
  target_protocol: string;
  request_headers: string;
  request_body: string;
  response_body: string;
  status_code: number;
  duration_ms: number;
  input_tokens: number;
  output_tokens: number;
  created_at: string;
}

export interface ProxyLogSettings {
  enabled: boolean;
  retention_days: number;
}

export interface ProxyTimeoutSettings {
  request_timeout_secs: number;
  connect_timeout_secs: number;
}

// ─── Proxy Log API ─────────────────────────────────────────

export const proxyLogApi = {
  list: (limit = 50, offset = 0) =>
    invoke<ProxyLogSummary[]>("proxy_log_list", { limit, offset }),
  get: (id: string) =>
    invoke<ProxyLogDetail | null>("proxy_log_get", { id }),
  clear: () => invoke<void>("proxy_log_clear"),
  count: () => invoke<number>("proxy_log_count"),
  getSettings: () =>
    invoke<ProxyLogSettings>("proxy_log_settings_get"),
  setSettings: (settings: ProxyLogSettings) =>
    invoke<void>("proxy_log_settings_set", { settings }),
};

// ─── Proxy Timeout API ──────────────────────────────────────

export const proxyTimeoutApi = {
  get: () => invoke<ProxyTimeoutSettings>("proxy_timeout_get"),
  set: (settings: ProxyTimeoutSettings) =>
    invoke<void>("proxy_timeout_set", { settings }),
};

// ─── Settings API ──────────────────────────────────────────

export const settingsApi = {
  get: (scope: string, key: string) =>
    invoke<Record<string, any> | null>("settings_get", { scope, key }),

  set: (scope: string, key: string, value: Record<string, any>) =>
    invoke<void>("settings_set", { input: { scope, key, value } }),

  delete: (scope: string, key: string) =>
    invoke<void>("settings_delete", { scope, key }),

  list: (scope: string) =>
    invoke<string[]>("settings_list", { scope }),
};
