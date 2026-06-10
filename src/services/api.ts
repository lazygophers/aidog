import { invoke } from "@tauri-apps/api/core";

// ─── Types ─────────────────────────────────────────────────

export type Protocol =
  // ── AI 请求协议（endpoint 协议）──
  | "anthropic" | "openai" | "openai_responses" | "openai_completions" | "gemini"
  // ── 平台类型 ──
  | "glm" | "glm_en" | "kimi" | "minimax" | "minimax_en" | "codex" | "bailian" | "bailian_coding"
  // ── 国内官方平台 ──
  | "deepseek" | "stepfun" | "stepfun_en" | "doubao" | "doubao_seed" | "byteplus" | "qianfan"
  | "xiaomi_mimo" | "bailing" | "longcat"
  // ── 聚合平台 ──
  | "openrouter" | "siliconflow" | "siliconflow_en" | "aihubmix" | "dmxapi" | "modelscope"
  | "shengsuanyun" | "atlascloud" | "novita" | "therouter" | "cherryin"
  // ── 第三方平台 ──
  | "packycode" | "cubence" | "aigocode" | "rightcode" | "aicodemirror" | "nvidia"
  | "pateway" | "ccsub" | "apikeyfun" | "apinebula" | "sudocode" | "claudeapi" | "claudecn"
  | "runapi" | "relaxycode" | "crazyrouter" | "sssaicode" | "compshare" | "compshare_coding"
  | "micu" | "ctok" | "eflowcode" | "lemondata" | "pipellm" | "opencode";
export type RoutingMode = "load_balance" | "failover";
export type ClientType =
  | "default"
  | "claude_code" | "claude_code_vscode" | "claude_code_sdk_ts" | "claude_code_sdk_py" | "claude_code_gh_action"
  | "codex_cli" | "codex_tui" | "codex_desktop" | "codex_vscode"
  | "cursor" | "windsurf";

export type ModelSlot = "default" | "sonnet" | "opus" | "haiku" | "gpt";

export interface PlatformEndpoint {
  protocol: Protocol;
  base_url: string;
  client_type?: ClientType;
  coding_plan?: boolean;
}

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
  endpoints: PlatformEndpoint[];
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
  source_protocol: string;
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
  source_protocol: string;
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

export interface PlatformUsageStats {
  total_requests: number;
  success_count: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cache_tokens: number;
  cache_rate: number;
  recent_failures: number;
  recent_total: number;
}

export const platformApi = {
  create: (input: {
    name: string;
    protocol: Protocol;
    base_url: string;
    api_key: string;
    extra?: string;
    models?: PlatformModels;
    available_models?: string[];
    endpoints?: PlatformEndpoint[];
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
    endpoints?: PlatformEndpoint[];
    enabled?: boolean;
  }) => invoke<Platform>("platform_update", { input }),

  delete: (id: string) => invoke<void>("platform_delete", { id }),

  fetchModels: (protocol: Protocol, baseUrl: string, apiKey: string) =>
    invoke<string[]>("platform_fetch_models", { protocol, baseUrl, apiKey }),

  usageStats: (platformId: string) =>
    invoke<PlatformUsageStats>("platform_usage_stats", { platformId }),
};

export const groupUsageApi = {
  stats: (groupName: string) =>
    invoke<PlatformUsageStats>("group_usage_stats", { groupName }),
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
    request_timeout_secs?: number;
    connect_timeout_secs?: number;
    source_protocol?: string;
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
  cache_tokens: number;
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
  upstream_request_headers: string;
  upstream_request_body: string;
  response_body: string;
  request_url: string;
  upstream_request_url: string;
  upstream_response_headers: string;
  upstream_status_code: number;
  user_response_headers: string;
  user_response_body: string;
  status_code: number;
  duration_ms: number;
  input_tokens: number;
  output_tokens: number;
  cache_tokens: number;
  created_at: string;
}

export interface ProxyLogSettings {
  enabled: boolean;
  retention_days: number;
}

export interface ProxyTimeoutSettings {
  request_timeout_secs: number;
  connect_timeout_secs: number;
  source_protocol: string;
}

export interface AppLogSettings {
  file_enabled: boolean;
  level: string;
  retention_hours: number;
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

// ─── App Log Settings API ─────────────────────────────────

export const appLogApi = {
  get: () => invoke<AppLogSettings>("app_log_settings_get"),
  set: (settings: AppLogSettings) =>
    invoke<void>("app_log_settings_set", { settings }),
};

// ─── Statistics Types & API ──────────────────────────────

export interface StatsQuery {
  start?: string;
  end?: string;
  granularity?: "hourly" | "daily";
  group_by?: "platform" | "model" | "group";
  filter_group?: string;
  filter_model?: string;
  filter_protocol?: string;
}

export interface StatsOverview {
  total_requests: number;
  success_rate: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cache_tokens: number;
  cache_rate: number;
  avg_duration_ms: number;
}

export interface StatsBucket {
  time_bucket: string;
  total_requests: number;
  success_count: number;
  error_count: number;
  input_tokens: number;
  output_tokens: number;
  cache_tokens: number;
  avg_duration_ms: number;
}

export interface DimensionEntry {
  name: string;
  total_requests: number;
  success_count: number;
  input_tokens: number;
  output_tokens: number;
  cache_tokens: number;
  avg_duration_ms: number;
}

export interface StatsResult {
  overview: StatsOverview;
  buckets: StatsBucket[];
  dimension_data: DimensionEntry[];
}

export const statsApi = {
  query: (query: StatsQuery) =>
    invoke<StatsResult>("stats_query", { query }),
};

// ─── Model Testing Types & API ───────────────────────────

export interface ModelTestRequest {
  platform_id: string;
  model?: string;
  prompt?: string;
  max_tokens?: number;
}

export interface ModelTestResult {
  success: boolean;
  model: string;
  prompt_preview: string;
  response_preview: string;
  duration_ms: number;
  input_tokens: number;
  output_tokens: number;
  error: string;
}

export const modelTestApi = {
  test: (req: ModelTestRequest) =>
    invoke<ModelTestResult>("model_test", { req }),
};

// ─── Platform Quota Types & API ────────────────────────────

export interface QuotaTier {
  name: string;          // "five_hour" | "weekly_limit"
  utilization: number;   // 0-100
  resets_at: string | null;
}

export interface BalanceInfo {
  remaining: number;
  total: number | null;
  used: number | null;
  currency: string;
  is_valid: boolean;
}

export interface CodingPlanInfo {
  tiers: QuotaTier[];
  level: string | null;
}

export interface PlatformQuota {
  success: boolean;
  error: string | null;
  queried_at: number;    // unix millis
  balance: BalanceInfo | null;
  coding_plan: CodingPlanInfo | null;
}

export const quotaApi = {
  query: (baseUrl: string, apiKey: string) =>
    invoke<PlatformQuota>("platform_query_quota", { baseUrl, apiKey }),
};
