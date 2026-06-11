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
  | "micu" | "ctok" | "eflowcode" | "lemondata" | "pipellm" | "opencode"
  // ── 中转平台 ──
  | "newapi"
  // ── 订阅透传 ──
  | "claude_code"
  // ── 测试 ──
  | "mock";
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

export type MockErrorMode = "none" | "http_error" | "rate_limit_429" | "timeout";

/** Mock 平台模拟配置（持久化在 platform.extra 的 `mock` 子对象内） */
export interface MockConfig {
  status_code: number;
  delay_ms: number;
  /** null = 跟随请求的 stream；true/false = 强制流式/非流式 */
  stream_override: boolean | null;
  response_text: string;
  finish_reason: string;
  input_tokens: number;
  output_tokens: number;
  cache_tokens: number;
  error_mode: MockErrorMode;
  chunk_count: number;
}

export const DEFAULT_MOCK_CONFIG: MockConfig = {
  status_code: 200,
  delay_ms: 0,
  stream_override: null,
  response_text: "Hello from mock",
  finish_reason: "end_turn",
  input_tokens: 100,
  output_tokens: 50,
  cache_tokens: 0,
  error_mode: "none",
  chunk_count: 5,
};

/** New API 平台余额查询配置（持久化在 platform.extra 的 `newapi` 子对象内） */
export interface NewApiConfig {
  /** 余额查询专用 API 地址（独立于主 base_url） */
  balance_base_url: string;
  /** 余额查询专用 API key（独立于主 api_key） */
  balance_api_key: string;
  /** 用户 ID（用于 New-Api-User 请求头） */
  user_id: string;
}

export const DEFAULT_NEWAPI_CONFIG: NewApiConfig = {
  balance_base_url: "",
  balance_api_key: "",
  user_id: "",
};

/** 从 platform.extra JSON 字符串解析 New API 配置 */
export function parseNewApiConfig(extra: string): NewApiConfig {
  if (!extra.trim()) return { ...DEFAULT_NEWAPI_CONFIG };
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "newapi" in parsed) {
      const cfg = (parsed as { newapi: unknown }).newapi;
      if (cfg && typeof cfg === "object") {
        return { ...DEFAULT_NEWAPI_CONFIG, ...(cfg as Partial<NewApiConfig>) };
      }
    }
  } catch { /* ignore */ }
  return { ...DEFAULT_NEWAPI_CONFIG };
}

/** 把 New API 配置写回 extra JSON 字符串，保留 extra 中其他键 */
export function serializeNewApiConfig(extra: string, cfg: NewApiConfig): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* ignore */ }
  }
  obj.newapi = cfg;
  return JSON.stringify(obj);
}

export interface Platform {
  id: number;
  name: string;
  platform_type: Protocol;
  base_url: string;
  api_key: string;
  extra: string;
  models: PlatformModels;
  available_models: string[];
  endpoints: PlatformEndpoint[];
  enabled: boolean;
  created_at: number;
  updated_at: number;
  deleted_at: number;
  /** 预估剩余余额（系统维护，只读） */
  est_balance_remaining: number;
  /** 预估 coding plan JSON（系统维护，只读） */
  est_coding_plan: string;
  /** 上次真实 quota 查询毫秒戳（系统维护，只读） */
  last_real_query_at: number;
  /** 自上次真查以来的预估次数（系统维护，只读） */
  estimate_count: number;
  /** 是否在系统托盘展示该平台 quota（互斥单平台） */
  show_in_tray: boolean;
  /** 托盘展示内容："balance" | "coding" */
  tray_display: string;
}

export interface Group {
  id: number;
  name: string;
  path: string;
  routing_mode: RoutingMode;
  /** 关联的平台 ID（十进制字符串；空串表示非自动） */
  auto_from_platform: string;
  created_at: number;
  updated_at: number;
  deleted_at: number;
  /** 超时设置（秒），0 = 继承系统设置 */
  request_timeout_secs: number;
  connect_timeout_secs: number;
  source_protocol: string;
  /** 内联模型映射数组 */
  model_mappings: ModelMapping[];
}

export interface GroupPlatformDetail {
  platform: Platform;
  priority: number;
  weight: number;
}

export interface ModelMapping {
  source_model: string;
  target_platform_id: number;
  target_model: string;
  /** 超时设置（秒），0 = 继承分组设置 */
  request_timeout_secs: number;
  connect_timeout_secs: number;
}

export interface GroupPlatform {
  id: number;
  group_id: number;
  platform_id: number;
  priority: number;
  weight: number;
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
  total_cost: number;
}

/** 从 platform.extra JSON 字符串解析 mock 配置（缺省字段回退默认值） */
export function parseMockConfig(extra: string): MockConfig {
  if (!extra.trim()) return { ...DEFAULT_MOCK_CONFIG };
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "mock" in parsed) {
      const mock = (parsed as { mock: unknown }).mock;
      if (mock && typeof mock === "object") {
        return { ...DEFAULT_MOCK_CONFIG, ...(mock as Partial<MockConfig>) };
      }
    }
  } catch {
    /* 非法 JSON → 回退默认 */
  }
  return { ...DEFAULT_MOCK_CONFIG };
}

/** 把 mock 配置写回 extra JSON 字符串，保留 extra 中其他键 */
export function serializeMockConfig(extra: string, mock: MockConfig): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch {
      /* 非法 JSON → 重建 */
    }
  }
  obj.mock = mock;
  return JSON.stringify(obj);
}

export const platformApi = {
  create: (input: {
    name: string;
    platform_type: Protocol;
    base_url: string;
    api_key: string;
    extra?: string;
    models?: PlatformModels;
    available_models?: string[];
    endpoints?: PlatformEndpoint[];
  }) => invoke<Platform>("platform_create", { input }),

  list: () => invoke<Platform[]>("platform_list"),

  get: (id: number) => invoke<Platform | null>("platform_get", { id }),

  update: (input: {
    id: number;
    name?: string;
    platform_type?: Protocol;
    base_url?: string;
    api_key?: string;
    extra?: string;
    models?: PlatformModels;
    available_models?: string[];
    endpoints?: PlatformEndpoint[];
    enabled?: boolean;
  }) => invoke<Platform>("platform_update", { input }),

  delete: (id: number) => invoke<void>("platform_delete", { id }),

  /** 拖拽排序：传入按新顺序排列的 platform id 列表 */
  reorder: (orderedIds: number[]) =>
    invoke<void>("platform_reorder", { orderedIds }),

  fetchModels: (protocol: Protocol, baseUrl: string, apiKey: string) =>
    invoke<string[]>("platform_fetch_models", { protocol, baseUrl, apiKey }),

  usageStats: (platformId: number) =>
    invoke<PlatformUsageStats>("platform_usage_stats", { platformId }),
};

/** 系统托盘 quota 展示（互斥单平台） */
export const trayApi = {
  /** 设选定平台为唯一托盘展示平台，display: "balance" | "coding" */
  set: (platformId: number, display: string) =>
    invoke<void>("platform_set_tray", {
      platformId,
      trayDisplay: display,
      enabled: true,
    }),
  /** 关闭托盘 quota 展示（清空所有平台） */
  clear: () =>
    invoke<void>("platform_set_tray", {
      platformId: 0,
      trayDisplay: "balance",
      enabled: false,
    }),
};

export const groupUsageApi = {
  stats: (groupName: string) =>
    invoke<PlatformUsageStats>("group_usage_stats", { groupName }),
};

// ─── Tray Config API ───────────────────────────────────────
// 字段名与 Rust serde（src-tauri/src/gateway/models.rs TrayConfig/TrayItem/TrayColor）保持 snake_case 一致。

/** 单项颜色（三态）。
 * - mode="follow": 跟随系统（labelColor，自适应明暗），value 忽略
 * - mode="preset": value ∈ "red" | "green" | "orange"（systemRed/Green/Orange 自适应）
 * - mode="custom": value = hex（如 "#RRGGBB"），固定色，某些菜单栏主题下可读性差
 */
export interface TrayColor {
  mode: "follow" | "preset" | "custom";
  value: string;
}

/** 托盘单个展示项。
 * - item_type="platform": platform_id 指定平台，display ∈ "balance" | "coding"
 * - item_type="today_usage": metric ∈ "tokens" | "cache_rate" | "cost" | "requests"，platform_id/display 忽略
 * - item_type="separator": display 存分隔符文本（如 "|"、"·"、"—"）
 */
export interface TrayItem {
  item_type: "platform" | "today_usage" | "separator";
  platform_id: number | null;
  display: string;
  metric: string | null;
  /** 自定义标签，null = 使用默认自动生成的名称 */
  label: string | null;
  /** 小数位数，null = 默认 5 位 */
  decimals: number | null;
  color: TrayColor;
  font_size: number;
  /** 该项行模式："single"（"名 值" 同行）| "two"（"名/值" 两行）。 */
  line_mode: "single" | "two";
  /** 对齐方式："left" | "center" | "right" */
  align: string;
  /** 两行模式第二行对齐，null = 跟随 align */
  align_row2: string | null;
  enabled: boolean;
  order: number;
}

/** 托盘整体配置（存 settings: scope="tray", key="config"）。
 * 全局仅保留 separator（多 item 间分隔，单行模式用）。 */
export interface TrayConfig {
  /** 多 item 横排时各项之间的分隔符（单行模式使用） */
  separator: string;
  items: TrayItem[];
}

/** 今日统计摘要 */
export interface TodayStats {
  tokens: number;
  cache_rate: number;
  cost: number;
  total_requests: number;
}

export const trayConfigApi = {
  /** 读取托盘配置（无配置时后端迁移旧 show_in_tray 平台生成默认）。 */
  get: () => invoke<TrayConfig>("tray_config_get"),
  /** 保存托盘配置并刷新托盘渲染。 */
  set: (config: TrayConfig) => invoke<void>("tray_config_set", { config }),
  /** 获取今日统计摘要（tokens / cache_rate / cost / requests）。 */
  todayStats: () => invoke<TodayStats>("tray_today_stats"),
};

// ─── Group API ─────────────────────────────────────────────

export const groupApi = {
  create: (input: {
    name: string;
    path: string;
    routing_mode: RoutingMode;
  }) => invoke<Group>("group_create", { input }),

  list: () => invoke<Group[]>("group_list"),

  get: (id: number) => invoke<Group | null>("group_get", { id }),

  update: (input: {
    id: number;
    name?: string;
    path?: string;
    routing_mode?: RoutingMode;
    request_timeout_secs?: number;
    connect_timeout_secs?: number;
    source_protocol?: string;
    model_mappings?: ModelMapping[];
  }) => invoke<Group>("group_update", { input }),

  delete: (id: number) => invoke<void>("group_delete", { id }),

  /** 拖拽排序：传入按新顺序排列的 group id 列表 */
  reorder: (orderedIds: number[]) =>
    invoke<void>("group_reorder", { orderedIds }),

  setPlatforms: (
    groupId: number,
    platforms: { platform_id: number; priority?: number; weight?: number }[]
  ) =>
    invoke<void>("group_set_platforms", {
      input: { group_id: groupId, platforms },
    }),

  getPlatforms: (groupId: number) =>
    invoke<GroupPlatformDetail[]>("group_get_platforms", { groupId }),
};

// ─── Aggregate API ─────────────────────────────────────────

export const groupDetailApi = {
  get: (id: number) =>
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
  platform_id: number;
  status_code: number;
  duration_ms: number;
  input_tokens: number;
  output_tokens: number;
  cache_tokens: number;
  created_at: number;
}

export interface ProxyLogDetail {
  id: string;
  group_name: string;
  model: string;
  actual_model: string;
  source_protocol: string;
  target_protocol: string;
  platform_id: number;
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
  created_at: number;
  updated_at: number;
  deleted_at: number;
}

export interface ProxyLogSettings {
  enabled: boolean;
  log_user_request: boolean;
  log_upstream_request: boolean;
  user_request_retention_days: number;
  upstream_request_retention_days: number;
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

// ─── Proxy Log Filter ──────────────────────────────────────

export interface ProxyLogFilter {
  platform_id?: number;
  group_name?: string;
  /** None=all, 200=success, -1=error */
  status?: number;
  time_start?: number;
  time_end?: number;
  model?: string;
  /** "original" = model 列, "actual" = actual_model 列 */
  model_type?: "original" | "actual";
}

// ─── Proxy Log API ─────────────────────────────────────────

export const proxyLogApi = {
  list: (limit = 50, offset = 0) =>
    invoke<ProxyLogSummary[]>("proxy_log_list", { limit, offset }),
  listFiltered: (filter: ProxyLogFilter, limit = 50, offset = 0) =>
    invoke<ProxyLogSummary[]>("proxy_log_list_filtered", { filter, limit, offset }),
  get: (id: string) =>
    invoke<ProxyLogDetail | null>("proxy_log_get", { id }),
  clear: () => invoke<void>("proxy_log_clear"),
  count: () => invoke<number>("proxy_log_count"),
  countFiltered: (filter: ProxyLogFilter) =>
    invoke<number>("proxy_log_count_filtered", { filter }),
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
  start?: number;
  end?: number;
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
  platform_id: number;
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
  /** New API: 自动获取的用户 ID，前端可回填到配置 */
  newapi_user_id?: string;
}

export const quotaApi = {
  query: (baseUrl: string, apiKey: string, platformId?: number) =>
    invoke<PlatformQuota>("platform_query_quota", { baseUrl, apiKey, platformId: platformId ?? null }),
  queryNewapi: (baseUrl: string, apiKey: string, extra: string, platformId?: number) =>
    invoke<PlatformQuota>("platform_query_quota_newapi", { baseUrl, apiKey, extra, platformId: platformId ?? null }),
};

// ─── Model Price Types & API ──────────────────────────────

export interface ModelPriceSummary {
  id: number;
  model_name: string;
  source: string;
  default_platform: string | null;
  /** $/M input tokens */
  input_price: number | null;
  /** $/M output tokens */
  output_price: number | null;
  /** $/M cache read tokens */
  cache_read_price: number | null;
  updated_at: number;
}

export interface ResolvedPrice {
  input_cost_per_token: number;
  output_cost_per_token: number;
  cache_read_input_token_cost: number;
  source: string;
}

export interface PriceSyncSettings {
  auto_sync_enabled: boolean;
  sync_interval_secs: number;
  last_sync_at: number;
  fallback_input_price: number;
  fallback_output_price: number;
}

export interface PriceSyncResult {
  added: number;
  updated: number;
  unchanged: number;
  failed: number;
  total: number;
}

export interface ModelPriceFilter {
  query?: string;
  source?: string;
}

export const modelPriceApi = {
  list: (limit = 50, offset = 0) =>
    invoke<ModelPriceSummary[]>("model_price_list", { limit, offset }),
  count: () =>
    invoke<number>("model_price_count"),
  search: (query: string, limit = 50) =>
    invoke<ModelPriceSummary[]>("model_price_search", { query, limit }),
  listFiltered: (filter: ModelPriceFilter, limit = 50, offset = 0) =>
    invoke<ModelPriceSummary[]>("model_price_list_filtered", { ...filter, limit, offset }),
  countFiltered: (filter: ModelPriceFilter) =>
    invoke<number>("model_price_count_filtered", { ...filter }),
  delete: (modelName: string) =>
    invoke<void>("model_price_delete", { modelName }),
  upsert: (modelName: string, source: string, priceData: string) =>
    invoke<void>("model_price_upsert", { modelName, source, priceData }),
  resolve: (modelName: string, platformType: string) =>
    invoke<ResolvedPrice>("model_price_resolve", { modelName, platformType }),
  sync: () =>
    invoke<PriceSyncResult>("model_price_sync"),
};

export const priceSyncApi = {
  get: () =>
    invoke<PriceSyncSettings>("price_sync_settings_get"),
  set: (settings: PriceSyncSettings) =>
    invoke<void>("price_sync_settings_set", { settings }),
};
