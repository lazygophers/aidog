import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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
/** 路由 / 调度策略。
 *  load_balance: 加权随机；failover: priority 升序；
 *  health_aware: 熔断摘除后健康集加权随机；least_latency: 延迟 EMA 升序；
 *  sticky: session 键绑定平台，失效/熔断回退加权随机。 */
export type RoutingMode =
  | "load_balance"
  | "failover"
  | "health_aware"
  | "least_latency"
  | "sticky";

/** 平台三态状态：enabled(用户启用) / disabled(用户手动禁用) / auto_disabled(401/403 自动禁用) */
export type PlatformStatus = "enabled" | "disabled" | "auto_disabled";
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

/** 手动预算限额种类。 */
export type ManualBudgetKind = "total" | "rolling" | "fixed" | "daily";
/** 手动预算计量单位。 */
export type ManualBudgetUnit = "usd" | "token";
/** 窗口时长单位（仅 rolling/fixed）。month 固定按 30 天换算。 */
export type WindowUnit = "minute" | "hour" | "day" | "week" | "month";

/** 手动预算限额（仅无上游 quota 自动支持平台开放）。
 *  consumed / window_start_at 由系统维护（请求驱动），编辑表单只设配置字段。 */
export interface ManualBudget {
  id: string;
  kind: ManualBudgetKind;
  unit: ManualBudgetUnit;
  amount: number;
  /** 窗口数值（该 window_unit 下的数量），仅 rolling/fixed。
   *  历史字段名保留为 window_hours（不改名以最小化迁移），实际含义为窗口数值。 */
  window_hours?: number | null;
  /** 窗口时长单位（minute/hour/day/week/month），旧数据缺失 → 默认 hour。 */
  window_unit?: WindowUnit;
  /** 当前窗口已消耗（系统维护，只读）。 */
  consumed: number;
  /** 当前窗口起始毫秒戳（系统维护，只读）。 */
  window_start_at?: number | null;
  enabled: boolean;
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
  /** 旧布尔启用位（向后兼容）；新逻辑用 status 三态。`status==enabled → true`。 */
  enabled: boolean;
  /** 三态状态：enabled / disabled(用户手动) / auto_disabled(401/403 自动) */
  status: PlatformStatus;
  /** auto_disabled 下次试探时间（毫秒 unix 时间戳）；退避用，0 = 立即可试探 */
  auto_disabled_until: number;
  /** 连续自动禁用次数（指数退避指数）；恢复 enabled 时清零 */
  auto_disable_strikes: number;
  /** 熔断失败阈值（连续失败达此数 → Open）；0 = 继承全局 SchedulingBreakerSettings 默认。 */
  breaker_failure_threshold: number;
  /** 熔断 Open 持续秒数（之后转 HalfOpen 探测）；0 = 继承全局默认。 */
  breaker_open_secs: number;
  /** HalfOpen 允许的最大探测请求数；0 = 继承全局默认。 */
  breaker_half_open_max: number;
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
  /** 手动预算限额列表（无上游 quota 平台；请求驱动扣减 + 耗尽阻断）。 */
  manual_budgets: ManualBudget[];
  /** 余额使用速率配色级别（后端 platform_list 按动态窗口日速率算 days_remaining 填充，只读）。
   *  "red"|"yellow"|"green"|"neutral"；空串 = 无数据 → 前端退中性。前端只消费不重算阈值。 */
  balance_level?: string;
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
  /** 分组级最大重试次数：失败后最多再换几个候选平台（0 = 不重试，只试 1 次） */
  max_retries: number;
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
  silent_launch: boolean;
}

export interface ProxyClientSettings {
  enabled: boolean;
  proxy_type: string; // "socks5" | "http" | "https"
  host: string;
  port: number;
  username: string;
  password: string;
  dns_over_proxy: boolean;
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
    manual_budgets?: ManualBudget[];
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
    /** 三态切换：仅可置 enabled / disabled（auto_disabled 仅系统 401/403 联动设置）。
     *  置 enabled 会清空退避状态（手动恢复）。 */
    status?: PlatformStatus;
    manual_budgets?: ManualBudget[];
    /** 熔断阈值覆盖（0=继承全局默认）；省略=保留既有值。 */
    breaker_failure_threshold?: number;
    breaker_open_secs?: number;
    breaker_half_open_max?: number;
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

// ─── Popover Config API ────────────────────────────────────
// 字段名与 Rust serde（src-tauri/src/gateway/models.rs PopoverConfig/PopoverItem）保持 snake_case 一致。

/** Popover 浮窗预定义指标集 item type。
 * - "today_cost"       今日已用金额
 * - "today_cache_rate" 今日缓存率
 * - "today_tokens"     今日 token 总量
 * - "platform_today"   各平台当日使用（只含已用，列表）
 * - "proxy_status"     代理状态行
 * - "platform_balance" 平台余额 / coding 列（来自 tray 配置）
 */
export type PopoverItemType =
  | "today_cost"
  | "today_cache_rate"
  | "today_tokens"
  | "platform_today"
  | "proxy_status"
  | "platform_balance";

/** Popover 浮窗单个展示项（预定义指标集内组合）。 */
export interface PopoverItem {
  /** 稳定 id（前端生成，拖拽 key 用）。 */
  id: string;
  item_type: PopoverItemType;
  visible: boolean;
  order: number;
}

/** Popover 浮窗整体配置（存 settings: scope="popover", key="config"）。 */
export interface PopoverConfig {
  items: PopoverItem[];
}

/** 单平台当日使用（popover「各平台当日」+ 设置预览）。 */
export interface TodayPlatformStat {
  platform_id: number;
  platform_name: string;
  tokens: number;
  cost: number;
  requests: number;
}

export const popoverConfigApi = {
  /** 读取 popover 配置（无配置 → 默认）。 */
  get: () => invoke<PopoverConfig>("popover_config_get"),
  /** 保存 popover 配置。 */
  set: (config: PopoverConfig) => invoke<void>("popover_config_set", { config }),
  /** 各平台当日使用（设置页预览）。 */
  platformToday: () => invoke<TodayPlatformStat[]>("popover_platform_today"),
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
    /** 分组级最大重试次数（0 = 不重试） */
    max_retries?: number;
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
  setAutolaunch: (enabled: boolean) =>
    invoke<void>("app_set_autolaunch", { enabled }),
  getAutolaunch: () => invoke<boolean>("app_get_autolaunch"),
  setSilentLaunch: (enabled: boolean) =>
    invoke<void>("app_set_silent_launch", { enabled }),
  getProxyClientSettings: () => invoke<ProxyClientSettings>("proxy_client_get_settings"),
  setProxyClientSettings: (settings: ProxyClientSettings) =>
    invoke<void>("proxy_client_set_settings", { settings }),
};

// ─── Claude Code Config Export ─────────────────────────────

export const configApi = {
  exportClaudeConfig: (port: number) =>
    invoke<string>("export_claude_config", { port }),
  syncGroupSettings: () =>
    invoke<string[]>("sync_group_settings"),
};

// ─── Proxy Log Types ───────────────────────────────────────

/** 单次平台尝试快照（proxy_log.attempts JSON 数组元素）。 */
export interface ProxyAttempt {
  platform_id: number;
  platform_name: string;
  /** 上游 HTTP 状态码；连接失败 / 超时为 0 */
  status_code: number;
  /** 错误描述（连接失败 / 超时 / 上游错误体摘要）；成功为空串 */
  error: string;
  duration_ms: number;
  /** 本次尝试发起时间（毫秒 unix 时间戳） */
  ts: number;
}

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
  is_stream: boolean;
  /** 重试次数（>0 时列表显示重试徽标） */
  retry_count: number;
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
  est_cost: number;
  is_stream: boolean;
  /** 每次平台尝试快照（时序列表）；单平台一次成功时长度 1 */
  attempts: ProxyAttempt[];
  /** 重试次数 = attempts.length - 1（0 表示一次成功） */
  retry_count: number;
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

// ─── Middleware Rule Engine API (C1 契约冻结点) ─────────────
// 字段名与 Rust serde（src-tauri/src/gateway/models.rs MiddlewareRule/MiddlewareSettings
// + 枚举 RuleType/RuleScope/MatchType/RuleAction）严格 snake_case 一致。
// 契约由 C1 冻结，C5(UI) 仅消费不改。设计见 design.md。
// 注：熔断器已移出中间件层（归 group 独立 task），MiddlewareSettings 不含 breaker。

/** 规则类型（8 类中间件能力）。 */
export type RuleType =
  | "request_filter"
  | "sensitive_word"
  | "redaction"
  | "content_filter"
  | "dynamic_injection"
  | "response_override"
  | "rectifier"
  | "error_rule";

/** 规则作用域（三级，就近覆盖：platform > group > global）。 */
export type RuleScope = "global" | "group" | "platform";

/** 匹配方式。 */
export type MatchType = "regex" | "contains" | "exact";

/** 命中动作。 */
export type RuleAction =
  | "mask"
  | "block"
  | "warn"
  | "inject"
  | "override"
  | "classify";

/** 单条中间件规则（对应 middleware_rule 表一行）。
 * config 为 type-specific JSON 字符串（按 rule_type 形状，见 design.md），前端按类型解析。 */
export interface MiddlewareRule {
  id: number;
  name: string;
  description: string;
  rule_type: RuleType;
  scope: RuleScope;
  /** group_name | platform_id(字符串) | ''(global) */
  scope_ref: string;
  match_type: MatchType;
  /** 匹配模式 / 目标 path / header 名 */
  pattern: string;
  action: RuleAction;
  /** type-specific JSON 字符串，默认 "{}" */
  config: string;
  /** 越小越先 */
  priority: number;
  enabled: boolean;
  is_builtin: boolean;
  created_at: number;
  updated_at: number;
}

/** 创建规则入参（不含 id / 时间戳，后端生成）。 */
export interface CreateMiddlewareRule {
  name: string;
  description?: string;
  rule_type: RuleType;
  scope?: RuleScope;
  scope_ref?: string;
  match_type?: MatchType;
  pattern?: string;
  action?: RuleAction;
  config?: string;
  priority?: number;
  enabled?: boolean;
  is_builtin?: boolean;
}

/** 更新规则入参（全量覆盖，id 必填）。 */
export interface UpdateMiddlewareRule {
  id: number;
  name: string;
  description?: string;
  rule_type: RuleType;
  scope?: RuleScope;
  scope_ref?: string;
  match_type?: MatchType;
  pattern?: string;
  action?: RuleAction;
  config?: string;
  priority?: number;
  enabled?: boolean;
  is_builtin?: boolean;
}

/** 中间件总设置（settings KV: scope="middleware" key="settings"）。
 * enabled 总开关（OFF = 全旁路）；type_toggles 按 rule_type 子开关（缺省键视为 true）。 */
export interface MiddlewareSettings {
  enabled: boolean;
  /** key = rule_type，缺省键视为 true */
  type_toggles: Record<string, boolean>;
}

export const middlewareApi = {
  /** 列出全部规则（后端按 priority 升序、id 升序）。 */
  listRules: () => invoke<MiddlewareRule[]>("middleware_list_rules"),
  /** 创建规则，返回新规则；后端写库后自动 reload 引擎缓存。 */
  createRule: (input: CreateMiddlewareRule) =>
    invoke<MiddlewareRule>("middleware_create_rule", { input }),
  /** 全量更新规则，返回更新后规则；写库后自动 reload。 */
  updateRule: (input: UpdateMiddlewareRule) =>
    invoke<MiddlewareRule>("middleware_update_rule", { input }),
  /** 删除规则；写库后自动 reload。 */
  deleteRule: (id: number) => invoke<void>("middleware_delete_rule", { id }),
  /** 读取中间件总设置（无配置 → 默认 enabled=true）。 */
  getSettings: () => invoke<MiddlewareSettings>("middleware_settings_get"),
  /** 保存中间件总设置。 */
  setSettings: (settings: MiddlewareSettings) =>
    invoke<void>("middleware_settings_set", { settings }),
};

// ─── Scheduling & Breaker Settings ─────────────────────────
// 字段名与 Rust serde（src-tauri/src/gateway/models.rs SchedulingBreakerSettings）一致。
// Platform 的 breaker_* 字段为 0 时继承本结构对应默认值（5/1800/2）。

/** 全局调度 + 熔断默认设置（settings scope=scheduling）。 */
export interface SchedulingBreakerSettings {
  /** 全局默认调度策略（Group routing_mode 覆盖之）。 */
  default_routing_mode: RoutingMode;
  /** 全局默认熔断失败阈值（default 5）。 */
  breaker_failure_threshold: number;
  /** 全局默认 Open 持续秒数（default 1800）。 */
  breaker_open_secs: number;
  /** 全局默认 HalfOpen 最大探测数（default 2）。 */
  breaker_half_open_max: number;
  /** 熔断总开关（default true；false = 旁路熔断）。 */
  enabled: boolean;
}

export const schedulingApi = {
  /** 读取全局调度+熔断设置（无配置 → 默认 5/1800/2，load_balance，enabled=true）。 */
  getSettings: () => invoke<SchedulingBreakerSettings>("scheduling_settings_get"),
  /** 保存全局调度+熔断设置。 */
  setSettings: (settings: SchedulingBreakerSettings) =>
    invoke<void>("scheduling_settings_set", { settings }),
};

// ─── Notification（N1 — 系统通知模块；契约冻结，N3 消费）────
// 字段名与 Rust serde（src-tauri/src/gateway/models.rs / notification.rs）一致。

/** 通知类型（serde snake_case；custom = 用户自定义类型）。 */
export type NotifType = "task_complete" | "waiting_input" | "error" | "custom";

/** 呈现形态：完整播报 / 仅弹窗 / 仅收件箱 / 仅提示音。 */
export type NotifForm = "popup_only" | "inbox_only" | "sound_only" | "full";

/** TTS 后端：跨平台 tts crate（默认）/ macOS `say` / 前端 WebSpeech。 */
export type TtsBackend = "cross_platform" | "mac_say" | "web_speech";

/** 单类型通知配置。template 含变量占位（{project}/{status}/{time}/{session}/{group}）。 */
export interface TypeSetting {
  /** 本类型是否 TTS 播报（与全局 tts_enabled 取与）。 */
  tts: boolean;
  /** 本类型是否弹窗。 */
  popup: boolean;
  /** 呈现形态。 */
  form: NotifForm;
  /** 模板（body 文本，含变量占位）。 */
  template: string;
}

/** 通知设置（settings scope=notification）。 */
export interface NotificationSettings {
  /** 总开关（OFF 时全部分发旁路；default true）。 */
  enabled: boolean;
  /** TTS 总开关（default true）。 */
  tts_enabled: boolean;
  /** TTS 后端（default cross_platform）。 */
  tts_backend: TtsBackend;
  /** 按类型配置（key = NotifType 字面量；缺省键视为全 true + full）。 */
  per_type: Record<string, TypeSetting>;
}

/** 收件箱通知项（notification 表行）。 */
export interface Notification {
  id: number;
  notif_type: string;
  title: string;
  body: string;
  read: boolean;
  created_at: number;
}

/** 分发结果（testNotify / 端点返回）。 */
export interface NotifyDispatchResult {
  dispatched: boolean;
  title: string;
  body: string;
  tts: boolean;
  popup: boolean;
  sound: boolean;
  inbox: boolean;
  inbox_id: number | null;
}

export const notificationApi = {
  /** 读取通知设置（无配置 → 默认全开 cross_platform）。 */
  getSettings: () => invoke<NotificationSettings>("notification_settings_get"),
  /** 保存通知设置。 */
  setSettings: (settings: NotificationSettings) =>
    invoke<void>("notification_settings_set", { settings }),
  /** 列收件箱（倒序；limit 默认 100）。 */
  listInbox: (limit?: number) =>
    invoke<Notification[]>("notification_inbox_list", { limit }),
  /** 未读数。 */
  unreadCount: () => invoke<number>("notification_inbox_unread"),
  /** 标记已读：id 省略 → 全部已读。 */
  markRead: (id?: number) =>
    invoke<void>("notification_mark_read", { id }),
  /** 清空收件箱。 */
  clearInbox: () => invoke<void>("notification_clear"),
  /** 测试通知（走分发逻辑，含弹窗/TTS）。 */
  testNotify: (notifType: NotifType | string, content?: string) =>
    invoke<NotifyDispatchResult>("notification_test", { notifType, content }),
  /**
   * 一键注入通知 hook（N2）。
   * - client="claude_code"：把 hooks.Stop/Notification 注入基线配置并 re-sync 到所有 settings.{group}.json。
   * - client="codex"：把 notify=[complete 脚本] 注入 ~/.codex/config.toml。
   * 同时物化内置默认模板（task_complete/waiting_input）。group 用于 API 对称。
   */
  injectHooks: (group: string, client: HookClient) =>
    invoke<void>("inject_hooks", { group, client }),
  /** 一键移除通知 hook（strip）。client 同 injectHooks。 */
  removeHooks: (group: string, client: HookClient) =>
    invoke<void>("remove_hooks", { group, client }),
};

/** hook 注入客户端类型（N2）。 */
export type HookClient = "claude_code" | "codex";

/** 收件箱未读数变化事件名（后端 emit / 前端 listen 必须一致）。 */
export const NOTIF_INBOX_UPDATED = "notif-inbox-updated";
/** WebSpeech 播报请求事件名（payload = 文本；前端 webview SpeechSynthesis 朗读）。 */
export const NOTIF_SPEAK = "notif-speak";

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

// ─── StatusLine Script Generation ──────────────────────────

export const statuslineApi = {
  /** Generate statusline script in ~/.aidog/ and return absolute path */
  generate: (scriptType: string, content: string) =>
    invoke<string>("generate_statusline_script", { scriptType, content }),
};

// ─── Codex Config API ─────────────────────────────────────

export const codexApi = {
  /** Read ~/.codex/config.toml (TOML) → JSON. Missing file → {}. */
  read: () => invoke<Record<string, unknown>>("codex_config_read"),
  /** Write JSON → ~/.codex/config.toml (TOML). Creates ~/.codex/ if missing. */
  write: (value: Record<string, unknown>) =>
    invoke<void>("codex_config_write", { value }),
  /** Absolute path of ~/.codex/config.toml. */
  path: () => invoke<string>("codex_config_path"),
};

// ─── Claude Code Settings Import ──────────────────────────

export const claudeSettingsImportApi = {
  /** Read ~/.claude/settings.json and return parsed JSON */
  readDefault: () =>
    invoke<Record<string, any>>("read_claude_code_settings"),
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
  total_cost: number;
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
  total_cost: number;
}

export interface DimensionEntry {
  name: string;
  total_requests: number;
  success_count: number;
  input_tokens: number;
  output_tokens: number;
  cache_tokens: number;
  avg_duration_ms: number;
  total_cost: number;
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
  name: string;          // "five_hour" | "weekly_limit" | "mcp_monthly"
  utilization: number;   // 0-100
  resets_at: string | null;
  /** 绝对上限（token 数 / 调用次数），仅部分平台有值 */
  limit: number | null;
  /** 绝对剩余量（token 数 / 调用次数），仅部分平台有值 */
  remaining: number | null;
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

// ─── Realtime Events ───────────────────────────────────────
// 后端每条 proxy_log 写库成功后 emit "proxy-log-updated"（payload 为 platform_id）。
// Platforms / Stats / Groups 三页用此事件实时刷新统计。

/** 后端代理日志更新事件名（后端 emit / 前端 listen 必须一致） */
export const PROXY_LOG_UPDATED = "proxy-log-updated";

/**
 * 监听 proxy-log-updated，debounce 合并突发后调 callback。
 * 返回 cleanup 函数：清 timer + unlisten，供 useEffect cleanup 使用。
 */
export function onProxyLogUpdated(callback: () => void, debounceMs = 500): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const unlistenPromise = listen(PROXY_LOG_UPDATED, () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => { callback(); }, debounceMs);
  });
  return () => {
    if (timer) clearTimeout(timer);
    unlistenPromise.then((un) => un()).catch((e) => console.error(e));
  };
}

// ─── Skills API ────────────────────────────────────────────
// 字段名严格 snake_case，与 Rust gateway/skills.rs 模型一一对齐（cross-layer-rules）。

/** 目标 agent（决定 --agent 参数 + 本地配置目录）。 */
export type SkillAgent = "claude" | "codex";

/**
 * 安装 scope（Rust 端 #[serde(tag = "kind")] 内部 tag 枚举）。
 * - global：用户级全局（npx skills add -g）。
 * - project：项目级，path 为项目根目录。
 */
export type SkillScope =
  | { kind: "global" }
  | { kind: "project"; path: string };

/** npx/node 环境探测结果。 */
export interface SkillsEnv {
  npx_available: boolean;
  node_version: string | null;
}

/** 已装 skill（`npx skills list --json` 解析，统一一条/skill，不分 agent）。 */
export interface SkillInfo {
  name: string;
  source: string | null;
  /** 已在哪些目标 agent（claude/codex 子集）启用。 */
  enabled_agents: SkillAgent[];
  scope: SkillScope;
  installed_path: string | null;
  description: string | null;
}

/** catalog 条目（可装 skill）。 */
export interface CatalogEntry {
  id: string;
  name: string;
  description: string | null;
  repo_url: string | null;
}

/** 写操作（install/update/remove）结果。 */
export interface SkillsOpResult {
  success: boolean;
  stdout: string;
  stderr: string;
}

export const skillsApi = {
  /** 探测 npx / node 环境。 */
  checkEnv: () => invoke<SkillsEnv>("skills_check_env"),
  /** 浏览 catalog（HTTP 抓 skills.sh，回退 npx find）。 */
  browseCatalog: () => invoke<CatalogEntry[]>("skills_browse_catalog"),
  /** 搜索 catalog。 */
  search: (keyword: string) =>
    invoke<CatalogEntry[]>("skills_search", { keyword }),
  /** 列指定 scope 下已装 skills（统一一条/skill，走 npx list --json）。 */
  listInstalled: (scope: SkillScope) =>
    invoke<SkillInfo[]>("skills_list_installed", { scope }),
  /** 为某 agent 启用 skill（npx add）。 */
  enable: (name: string, agent: SkillAgent, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_enable", { name, agent, scope }),
  /** 为某 agent 关闭 skill（npx remove）。 */
  disable: (name: string, agent: SkillAgent, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_disable", { name, agent, scope }),
  /** 更新已装 skills。 */
  update: (scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_update", { scope }),
};
