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
  | "micu" | "ctok" | "eflowcode" | "lemondata" | "pipellm" | "opencode" | "opencode_zen"
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
  /** 分组密钥：Bearer token + 路由匹配键 + 日志归属键。UNIQUE。创建后锁定不可改。 */
  group_key: string;
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
  /** 是否为默认分组（单选）：true 时该组 config merge 写入
   * ~/.claude/settings.json + ~/.codex/config.toml，使用户直接 claude/codex
   * 不带 -c/--profile 即走该组。 */
  is_default?: boolean;
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

/** 平台级熔断阈值覆盖，存于 platform.extra JSON 的嵌套对象 breaker。
 *  每字段 0/缺省 = 继承全局 SchedulingBreakerSettings 默认。 */
export interface PlatformBreaker {
  failure_threshold: number;
  open_secs: number;
  half_open_max: number;
}

/** 从 platform.extra JSON 解析 breaker 覆盖（空/非法/缺键 → 全 0 继承全局默认）。 */
export function parsePlatformBreaker(extra: string): PlatformBreaker {
  const zero: PlatformBreaker = { failure_threshold: 0, open_secs: 0, half_open_max: 0 };
  if (!extra.trim()) return zero;
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "breaker" in parsed) {
      const b = (parsed as { breaker: unknown }).breaker;
      if (b && typeof b === "object") {
        const o = b as Record<string, unknown>;
        return {
          failure_threshold: typeof o.failure_threshold === "number" ? o.failure_threshold : 0,
          open_secs: typeof o.open_secs === "number" ? o.open_secs : 0,
          half_open_max: typeof o.half_open_max === "number" ? o.half_open_max : 0,
        };
      }
    }
  } catch {
    /* 非法 JSON → 回退全 0 */
  }
  return zero;
}

/** 把 breaker 覆盖写回 extra JSON（保留其余键）。三值全 0 → 移除 breaker 键（无覆盖=继承全局）。 */
export function serializePlatformBreaker(extra: string, b: PlatformBreaker): string {
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
  if (b.failure_threshold === 0 && b.open_secs === 0 && b.half_open_max === 0) {
    delete obj.breaker;
  } else {
    obj.breaker = b;
  }
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
    /** 是否自动创建默认分组（transient 创建时一次性判断；省略=true 旧行为；false=不建）。 */
    auto_group?: boolean;
    /** 额外加入的已有分组 ID 列表（plain membership）。 */
    join_group_ids?: number[];
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
    /** 熔断阈值覆盖现走 extra.breaker（随 extra 整体更新），无独立字段。 */
    /** 全量同步该平台的手动组成员关系（省略=不动）。 */
    join_group_ids?: number[];
  }) => invoke<Platform>("platform_update", { input }),

  delete: (id: number) => invoke<void>("platform_delete", { id }),

  /** 为平台补建默认 auto 分组（若已存在则跳过）。供批量导入回挂复用（cc-switch / 导入）。 */
  ensureAutoGroup: (id: number) => invoke<void>("platform_ensure_auto_group", { id }),

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
  stats: (groupKey: string) =>
    invoke<PlatformUsageStats>("group_usage_stats", { groupKey }),
  // 批量：单次 invoke 返回所有 group → 聚合 map（group_key → stats），消除前端逐 group N+1 往返。
  // 后端 GROUP BY group_key，共享平台不重复计入。
  statsAll: () =>
    invoke<Record<string, PlatformUsageStats>>("all_group_usage_stats"),
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
    /** 分组密钥；省略/空 → 后端自动生成 gk_<32hex>。创建后锁定不可改。 */
    group_key?: string;
    routing_mode: RoutingMode;
  }) => invoke<Group>("group_create", { input }),

  list: () => invoke<Group[]>("group_list"),

  get: (id: number) => invoke<Group | null>("group_get", { id }),

  update: (input: {
    id: number;
    name?: string;
    routing_mode?: RoutingMode;
    request_timeout_secs?: number;
    connect_timeout_secs?: number;
    source_protocol?: string;
    /** 分组级最大重试次数（0 = 不重试） */
    max_retries?: number;
    model_mappings?: ModelMapping[];
  }) => invoke<Group>("group_update", { input }),

  delete: (id: number) => invoke<void>("group_delete", { id }),

  /** 设置默认分组（单选）。传 id=null 取消默认（无默认组）。
   * 设置后该组 config merge 写入 ~/.claude/settings.json + ~/.codex/config.toml。 */
  setDefault: (id: number | null) =>
    invoke<void>("group_set_default", { id }),

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

  /** 分组内平台拖拽排序：orderedIds 按序赋 priority 1,2,3… */
  reorderPlatforms: (groupId: number, orderedIds: number[]) =>
    invoke<void>("group_platform_reorder", { groupId, orderedIds }),

  /** 跨分组移动平台：从 from 组移除、加入 to 组 */
  movePlatform: (platformId: number, fromGroupId: number, toGroupId: number) =>
    invoke<void>("group_platform_move", { platformId, fromGroupId, toGroupId }),
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
  group_key: string;
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
  group_key: string;
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
  group_key?: string;
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
  /** group_key | platform_id(字符串) | ''(global) */
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

/** 通知类型（serde snake_case）。3 类型：task_complete / waiting_input / error。 */
export type NotifType = "task_complete" | "waiting_input" | "error";

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

/**
 * 单事件触发配置（per_event 值；N2 hook 事件通知 — 逐事件自含）。
 * 镜像后端 `src-tauri/src/gateway/models.rs` 的 `EventSetting`。
 * 已删 `notif_type`：每事件独立 tts/popup 通道 + 专属默认模板（见 NotificationEventList EVENT_CATALOG）。
 */
export interface EventSetting {
  /** 是否启用该事件（注入 hook + 触发通知）。 */
  enabled: boolean;
  /** 该事件是否 TTS 播报（与全局 tts_enabled 取与；缺省 true）。 */
  tts: boolean;
  /** 该事件是否弹窗（缺省 true）。 */
  popup: boolean;
  /** 该事件是否播提示音（独立通道，不跟随弹窗；缺省 true）。 */
  sound: boolean;
  /** 可选 per-event 自定义文案（空则回退该事件专属默认模板）。 */
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
  /**
   * 按事件配置（key = CC hook 事件名，见 NotificationEventList.tsx CC_HOOK_EVENTS）。
   * 旧配置无此字段 → undefined / 空对象（前端按默认目录展示，用户开启才写入）。
   */
  per_event?: Record<string, EventSetting>;
}

/** 收件箱通知项（notification 表行）。 */
export interface Notification {
  id: number;
  notif_type: string;
  title: string;
  body: string;
  created_at: number;
}

/** notify hook 片段中单个 handler（CC hooks schema：type=command + 脚本命令串）。 */
export interface NotifyHookHandler {
  type: string;
  command: string;
}

/** notify hook 片段中单个匹配组（backend inject 产出无 matcher 字段，匹配所有）。 */
export interface NotifyHookGroup {
  hooks: NotifyHookHandler[];
}

/** `build_notify_hooks_fragment` 返回的 CC hooks 子对象（`{Stop:[...], Notification:[...]}`）。 */
export type NotifyHooksFragment = Record<string, NotifyHookGroup[]>;

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
  /** 清空收件箱。 */
  clearInbox: () => invoke<void>("notification_clear"),
  /** 测试通知（走分发逻辑，含弹窗/TTS）。 */
  testNotify: (notifType: NotifType | string, content?: string) =>
    invoke<NotifyDispatchResult>("notification_test", { notifType, content }),
  /** 仅测 TTS 通道：按当前 settings.tts_backend 播报 text，不走 dispatch。 */
  testTts: (text: string) =>
    invoke<void>("notification_test_tts", { text }),
  /** 仅测系统弹窗通道：直接调 tauri-plugin-notification，不走 dispatch。 */
  testPopup: (title: string, body: string) =>
    invoke<void>("notification_test_popup", { title, body }),
  /** 仅测系统提示音通道：跨平台 spawn beep（macOS afplay / Windows powershell / Linux paplay）。 */
  testBeep: () =>
    invoke<void>("notification_test_beep"),
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
  /** 读取「默认为所有分组注入通知 hook」总开关（基线 _aidog_hooks.enabled）。 */
  getDefaultHooksEnabled: () =>
    invoke<boolean>("get_default_hooks_enabled"),
  /**
   * 构造通知 hook 片段供 Hooks 编辑器并入草稿（只读式：确保 notify 脚本落盘，
   * 但不写 DB、不 sync）。返回 `{Stop:[...], Notification:[...]}` 形状的 CC hooks 子对象。
   * 前端把它并入草稿 config.hooks，由用户正常保存触发既有 sync 物化。
   */
  buildNotifyHooksFragment: () =>
    invoke<NotifyHooksFragment>("build_notify_hooks_fragment"),
  /**
   * 设置「默认为所有分组注入通知 hook」总开关：开=全分组注入 CC hooks + Codex notify，
   * 关=全移除。写基线 _aidog_hooks.enabled 并 re-sync 物化。
   */
  setDefaultHooksEnabled: (enabled: boolean) =>
    invoke<void>("set_default_hooks_enabled", { enabled }),
};

/** hook 注入客户端类型（N2）。 */
export type HookClient = "claude_code" | "codex";

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
  /**
   * Generate the statusline Python script in ~/.aidog/scripts/ and return the
   * **command string** to invoke it (`uv run --script <path>` or `python3 <path>`,
   * per the resolved ScriptInvoker). Write this verbatim into the native
   * `statusLine.command` / `subagentStatusLine.command` field.
   */
  generate: (scriptType: string, content: string) =>
    invoke<string>("generate_statusline_script", { scriptType, content }),
};

// ─── Script Executor (uv / python3) ────────────────────────

/** 脚本执行器选择。"uv" → uv run --script；"python3" → python3。 */
export type ScriptExecutor = "uv" | "python3";

export const scriptExecutorApi = {
  /** 检测 uv 是否可用（true=已安装）。 */
  checkUv: () => invoke<boolean>("check_uv"),
  /**
   * 自动安装 uv（官方安装脚本）。成功返回 true 并持久化执行器为 "uv"。
   * 仅 Unix 支持自动安装；其他平台抛错由前端引导手动安装。
   */
  installUv: () => invoke<boolean>("install_uv"),
  /** 持久化脚本执行器选择，避免每次注入时重复询问。 */
  setExecutor: (executor: ScriptExecutor) =>
    invoke<void>("set_script_executor", { executor }),
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
  granularity?: "hourly" | "daily" | "minute" | "5min";
  group_by?: "platform" | "model" | "group";
  filter_group?: string;
  filter_model?: string;
  filter_platform?: string;
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
  available_models: string[];
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
  /** 最大输入 token（模型固有，平台无关）。null = 未知。 */
  max_input_tokens?: number | null;
  /** 最大输出 token（出站裁剪用）。null = 未知/无限制。 */
  max_output_tokens?: number | null;
  /** 上下文窗口。null = 未知。 */
  context_window?: number | null;
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
  /** 已在哪些目标 agent（claude/codex 子集）启用。 */
  enabled_agents: SkillAgent[];
  scope: SkillScope;
  installed_path: string | null;
  description: string | null;
  /** 来源 owner/repo（锁文件 source）。第三方/手动 symlink skill → null。 */
  source: string | null;
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

/** skill 详情：文件列表（只读浏览）。 */
export interface SkillFile {
  rel_path: string;
  size: number;
  is_text: boolean;
}

export interface SkillDetail {
  skill_name: string;
  root: string;
  files: SkillFile[];
}

/** 单文件读取结果（带路径遍历防护 + 二进制检测 + 大小上限）。 */
export interface SkillFileContent {
  content: string | null;
  truncated: boolean;
  size: number;
}

/**
 * SWR list 缓存返回（后端 `skills_list_installed` / `skills_list_refresh`）。
 * - items：缓存/最新 skill 列表。
 * - stale：true = 无缓存命中（冷启动），前端应显加载态并强制 refresh。
 */
export interface CachedSkills {
  items: SkillInfo[];
  stale: boolean;
}

export const skillsApi = {
  /** 探测 npx / node 环境。 */
  checkEnv: () => invoke<SkillsEnv>("skills_check_env"),
  /** 浏览 catalog（skills.sh HTTP 端点当前 404，恒返回空；前端用 search）。 */
  browseCatalog: () => invoke<CatalogEntry[]>("skills_browse_catalog"),
  /** 搜索 catalog（`npx skills find <kw>`，id = `owner/repo@skill`）。 */
  search: (keyword: string) =>
    invoke<CatalogEntry[]>("skills_search", { keyword }),
  /**
   * 从 catalog 安装 skill 到多个 agent（`npx skills add <id> -a <slug> [-g] -y`）。
   * id = CatalogEntry.id（`owner/repo@skill`，含子 skill 选取，无需 -s）。
   */
  install: (id: string, agents: SkillAgent[], scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_install", { id, agents, scope }),
  /** 列已装 skill 目录文件树（详情视图，只读）。 */
  detail: (installedPath: string) =>
    invoke<SkillDetail>("skill_detail", { installedPath }),
  /** 读 skill 内单文件（只读，带路径遍历防护）。 */
  readFile: (installedPath: string, rel: string) =>
    invoke<SkillFileContent>("skill_read_file", { installedPath, rel }),
  /**
   * 列指定 scope 下已装 skills —— **立即返回缓存**（命中即 0 子进程）。
   * 冷启动返回 `{ items: [], stale: true }`，调用方据此显加载态 + 触发 refresh。
   */
  listInstalled: (scope: SkillScope) =>
    invoke<CachedSkills>("skills_list_installed", { scope }),
  /** 强制跑 npx 刷新缓存并返回 fresh（SWR revalidate 半）。 */
  listRefresh: (scope: SkillScope) =>
    invoke<CachedSkills>("skills_list_refresh", { scope }),
  /** 为某 agent 启用 skill（npx add，用 skill 本地 path 作 add package）。 */
  enable: (name: string, path: string, agent: SkillAgent, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_enable", { name, path, agent, scope }),
  /** 为某 agent 关闭 skill（npx remove）。 */
  disable: (name: string, agent: SkillAgent, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_disable", { name, agent, scope }),
  /** 更新已装 skills。 */
  update: (scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_update", { scope }),
  /** 一键卸载当前 scope 所有平台所有 skills（破坏性）。 */
  uninstallAll: (scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_uninstall_all", { scope }),
  /** 卸载单一 skill（破坏性）：删规范存储 + 所有 agent 启用配置。 */
  uninstall: (name: string, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_uninstall", { name, scope }),
  /** 组级卸载（破坏性）：卸载某 source 分组（groupSource=null = 「其他」组）内所有 skill。 */
  uninstallGroup: (groupSource: string | null, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_uninstall_group", { groupSource, scope }),
  /** 对齐两 agent 的 skills 启用配置（使 to 与 from 完全一致）。 */
  alignAgents: (from: SkillAgent, to: SkillAgent, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_align_agents", { from, to, scope }),
  /** 为某 agent 启用当前 scope 全部已装 skills（只增不减）。 */
  enableAll: (agent: SkillAgent, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_enable_all", { agent, scope }),
  /** 组级 agent 批量：对某 source 组（groupSource=null = 「其他」组）内所有 skill 统一启用/禁用某 agent。 */
  setGroupAgent: (
    groupSource: string | null,
    agent: SkillAgent,
    enable: boolean,
    scope: SkillScope,
  ) =>
    invoke<SkillsOpResult>("skills_set_group_agent", {
      groupSource,
      agent,
      enable,
      scope,
    }),
};

// ─── MCP 管理 ─────────────────────────────────────────────

/** 受管 agent slug（对齐后端 mcp.rs::McpAgent，注意 claude-code 非 "claude"）。 */
export type McpAgentSlug = "claude-code" | "codex";

/** MCP 传输类型。 */
export type McpTransport = "stdio" | "http" | "sse";

/**
 * DB 中 MCP server（列表用）。env/headers 已脱敏（敏感值 → "***"）。
 * 后端 McpServerInfo serde camelCase。
 */
export interface McpServerInfo {
  id: number;
  name: string;
  transport: McpTransport;
  command: string;
  args: string[];
  /** 脱敏后。 */
  env: Record<string, string>;
  url: string;
  /** 脱敏后。 */
  headers: Record<string, string>;
  enabledAgents: McpAgentSlug[];
  createdAt: number;
  updatedAt: number;
}

/** 扫描结果项（claude.json + codex config.toml 去重合并）。 */
export interface McpScanItem {
  name: string;
  transport: McpTransport;
  command: string;
  args: string[];
  env: Record<string, string>;
  url: string;
  headers: Record<string, string>;
  foundInAgents: McpAgentSlug[];
  alreadyImported: boolean;
}

/** 导入项。env/headers 前端传脱敏值，后端优先从 agent 配置取原值。 */
export interface McpImportPayload {
  name: string;
  transport: McpTransport;
  command: string;
  args: string[];
  env: Record<string, string>;
  url: string;
  headers: Record<string, string>;
  sourceAgent: McpAgentSlug;
}

export interface McpImportReport {
  imported: string[];
  skipped: string[];
}

/** 编辑 MCP 入参。env/headers 未改的敏感值前端传 "***"，后端 merge 旧 DB 明文。 */
export interface McpUpdatePayload {
  name: string;
  transport: McpTransport;
  command: string;
  args: string[];
  env: Record<string, string>;
  url: string;
  headers: Record<string, string>;
}

export const mcpApi = {
  /** 列出 DB 中所有 MCP（env/headers 脱敏）。 */
  list: () => invoke<McpServerInfo[]>("mcp_list"),
  /** 扫描 Claude Code + Codex 配置去重合并。 */
  scan: () => invoke<McpScanItem[]>("mcp_scan"),
  /** 批量导入（enabled = source agent）。 */
  import: (items: McpImportPayload[]) =>
    invoke<McpImportReport>("mcp_import", { items }),
  /** per-agent 启用/禁用（同步写/删 agent 配置）。 */
  setAgent: (name: string, agent: McpAgentSlug, enabled: boolean) =>
    invoke<void>("mcp_set_agent", { name, agent, enabled }),
  /** 编辑（全字段 + 改名 + transport 切换，同步 agent 配置）。 */
  update: (oldName: string, payload: McpUpdatePayload) =>
    invoke<McpServerInfo>("mcp_update", { oldName, payload }),
  /** 手动添加（enabled 空，不写 agent 配置；后续 setAgent 启用）。 */
  add: (payload: McpUpdatePayload) =>
    invoke<McpServerInfo>("mcp_add", { payload }),
  /** 删除（DB + 所有 enabled agent 配置，破坏性）。 */
  delete: (name: string) => invoke<void>("mcp_delete", { name }),
  /** 重新同步全部：从 DB 全量重写所有 enabled agent 配置（修复外部污染如 env:null）。 */
  resync: () => invoke<number>("mcp_resync"),
};

// ─── 导入导出子系统 ───────────────────────────────────────

export type ImportExportScope =
  | "platform"
  | "group"
  | "group_platform"
  | "setting"
  | "codex"
  | "claude_code"
  | "model_price"
  | "skills";

export interface ImportExportManifest {
  format_version: number;
  aidog_version: string;
  created_at: string;
  source_machine: string;
  scopes: string[];
  checksum: string;
}

export type ImportDecision =
  | { kind: "overwrite" }
  | { kind: "skip" }
  | { kind: "rename"; new_key: string };

export interface ConflictItem {
  scope: string;
  key: string;
  existing_summary: string;
  incoming_summary: string;
}

export interface ConflictDecision {
  scope: string;
  key: string;
  decision: ImportDecision;
}

export interface ImportPreview {
  manifest: ImportExportManifest;
  scopes: string[];
  conflicts: ConflictItem[];
  counts: Record<string, number>;
}

export interface ImportReport {
  applied: Record<string, number>;
  skipped: Record<string, number>;
  errors: string[];
}

export const importExportApi = {
  /** 导出勾选范围到用户选择的文件。 */
  exportToFile: (scopes: ImportExportScope[], path: string) =>
    invoke<void>("export_to_file", { scopes, path }),
  /** 读文件 → 解密 → 冲突预览。 */
  readPreview: (path: string) =>
    invoke<ImportPreview>("import_read_file", { path }),
  /** 按决策应用导入。 */
  apply: (path: string, decisions: ConflictDecision[]) =>
    invoke<ImportReport>("import_apply", { path, decisions }),
};

// ─── cc-switch 导入（异源单向，仅 claude + codex provider）───

/** codex provider config.toml 解析后字段（后端已解析，前端直接消费）。 */
export interface CodexConfigParsed {
  model?: string;
  modelProvider?: string;
  baseUrl?: string;
  wireApi?: string;
  providerName?: string;
}

/** cc-switch provider 中间表示（后端 DTO，camelCase）。 */
export interface CcProvider {
  id: string;
  appType: "claude" | "codex";
  name: string;
  /** 原始 settings_config JSON。 */
  settingsConfig: Record<string, unknown>;
  websiteUrl?: string;
  /** claude: env.ANTHROPIC_BASE_URL；codex: config.toml base_url。 */
  detectedBaseUrl?: string;
  /** claude: env.ANTHROPIC_AUTH_TOKEN/API_KEY；codex: auth.OPENAI_API_KEY。 */
  detectedApiKey?: string;
  /** codex 专用：解析后的 config.toml 字段。claude 为 undefined。 */
  codexConfigParsed?: CodexConfigParsed;
}

export interface CcswitchDetection {
  found: boolean;
  path?: string;
  /** `sqlite` | `json` | `none`。 */
  sourceType: string;
  providerCount: number;
}

export interface CcswitchReadResult {
  sourceType: string;
  path: string;
  providers: CcProvider[];
}

export const ccswitchApi = {
  /** 探测 cc-switch 配置存在性 + 路径。 */
  detect: (overridePath?: string) =>
    invoke<CcswitchDetection>("ccswitch_detect", { overridePath }),
  /** 读取 providers（仅 claude + codex）。 */
  read: (path?: string) =>
    invoke<CcswitchReadResult>("ccswitch_read", { path }),
  /** 接收前端转换好的 Platform JSON + 决策，走 apply::apply 写入。 */
  import: (platformPayload: unknown[], decisions: ConflictDecision[]) =>
    invoke<ImportReport>("ccswitch_import", { platformPayload, decisions }),
};

// ─── 定时备份 ───────────────────────────────────────────────

/** 定时备份设置（字段 snake_case，与后端 BackupSettings 对齐）。 */
export interface BackupSettings {
  enabled: boolean;
  /** 间隔小时，≥1。 */
  interval_hours: number;
  /** 保留天数，1..=90。 */
  retention_days: number;
  /** 上次成功备份 epoch 毫秒（0=从未），后端写。 */
  last_backup_at: number;
  /** 上次错误信息（空=成功），后端写。 */
  last_backup_error: string;
}

/** 立即备份结果。 */
export interface BackupResult {
  ok: boolean;
  path?: string;
  error?: string;
  timestamp: number;
}

export const backupApi = {
  /** 读取定时备份设置（缺省/解析失败 → 后端默认）。 */
  get: () => invoke<BackupSettings>("backup_settings_get"),
  /** 写入设置（后端会 clamp 非法值，返回规范化后的值）。 */
  set: (settings: BackupSettings) =>
    invoke<BackupSettings>("backup_settings_set", { settings }),
  /** 立即触发一次备份（忽略 throttle）。 */
  runNow: () => invoke<BackupResult>("backup_run_now"),
};

// ─── About / 版本信息 ───────────────────────────────────────

/** 关于页版本信息（字段 snake_case，与后端 AboutInfo 对齐）。 */
export interface AboutInfo {
  app_version: string;
  tauri_version: string;
  os: string;
  arch: string;
  family: string;
  profile: string;
  /** git 短 commit（无 git 时 "unknown"）。 */
  git_commit: string;
  /** 构建时间 epoch 秒字符串（前端格式化）。 */
  build_time: string;
}

export const aboutApi = {
  /** 读取应用 / 运行时 / 系统 / 构建版本信息。 */
  info: () => invoke<AboutInfo>("about_info"),
};
