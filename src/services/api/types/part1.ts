// types/part1.ts — 类型分片 1/4（arch-redesign），纯移动。
// 由 types.ts barrel 统一 re-export；外部应 `import type { X } from "../types"`，
// 不直接 import 本文件（分片边界为实现细节）。

import type { PeakWindow } from "../../../domains/platforms/defaults";


// ─── Types ─────────────────────────────────────────────────

export type Protocol =
  // ── AI 请求协议（endpoint 协议）──
  | "anthropic" | "openai" | "openai_responses" | "openai_completions" | "gemini"
  // ── 平台类型 ──
  | "glm" | "glm_coding" | "glm_en" | "kimi" | "kimi_coding" | "minimax" | "minimax_en" | "codex"
  | "bailian" | "bailian_coding" | "qianfan_coding" | "xiaomi_mimo_coding"
  // ── 国内官方平台 ──
  | "deepseek" | "stepfun" | "stepfun_en" | "doubao" | "byteplus" | "qianfan"
  | "xiaomi_mimo" | "bailing" | "longcat" | "sensenova"
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
  // ── CPA(CLIProxyAPI) 导入专属：OAuth channel / vertex-api-key 段衍生，复用现有 adapter ──
  | "cpa-grok" | "cpa-aistudio" | "cpa-antigravity" | "cpa-vertex"
  // ── CLI 代理（cpa-standalone-module）：platform_type 仅标识，wire/base_url/api_key/models 由 candidate resolve 时从 cli_proxy_provider 表注入 ──
  | "cli-proxy"
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


/** 可模拟的客户端类型（JSON 驱动，serde arbitrary）。
 *  真值源：`src-tauri/defaults/client-types.json`（13 entry，见 prd `07-10-client-types-json-sync`）。
 *  远端 / DB 任何字符串都原值保留；前端展示走 `buildClientTypesFromPresets` 派生层
 *  （invoke `get_client_types_json` → locale 派生 label），禁直读 github / 文件系统。 */
export type ClientType = string;


export type ModelSlot = "default" | "sonnet" | "opus" | "haiku" | "gpt";

/**
 * fetchModels 失败的结构化错误（镜像后端 FetchModelsError enum，tag=kind）。
 * 前端 handleFetchModels 按 kind 分流：Auth → 立即 break + 鉴权专用文案；
 * NotFound / Other → continue 试下一协议 endpoint。
 */
export interface FetchModelsError {
  kind: "Auth" | "NotFound" | "Other";
  code: number;
  message: string;
}


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

/** 时段模型规则：按时段窗口切换主力模型档（time_models） */
export interface TimeModelRule {
  /** 时段窗口列表（复用 PeakWindow 定义，multiplier 字段忽略） */
  windows: PeakWindow[];
  /** 5 槽模型配置（default/opus/sonnet/haiku/gpt） */
  models: PlatformModels;
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


export interface NewApiConfig {
  /** 余额查询专用 API 地址（独立于主 base_url） */
  balance_base_url: string;
  /** 余额查询专用 API key（独立于主 api_key） */
  balance_api_key: string;
  /** 用户 ID（用于 New-Api-User 请求头） */
  user_id: string;
}


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
  /** 过期时间（毫秒 unix 时间戳，0 = 永不过期）。>0 且 now>=expires_at 时路由排除（等效自动禁用）。
   *  独立于 status 三态：用户改值（清空/延后）即恢复，无需退避试探。 */
  expires_at: number;
  /** 最近一次失败的错误信息（系统维护，只读）。空串 = 最近一次成功或无记录；非请求记录实时取。 */
  last_error?: string;
  /** 最近一次错误的毫秒 unix 时间戳（系统维护，只读）；0 = 无。 */
  last_error_at?: number;
}

/** 单平台可分享配置（剥离 DB 内部 / 运行时字段，含明文 api_key）。
 *  后端 platform_share_export 返回结构化对象，前端按 YAML / JSON / Base64 转换；
 *  platform_share_parse 反向解析（serde_yml 兼容 YAML / JSON 超集）。
 *  顶层 aidog_platform_share=1 为格式标识，接收端据此校验合法分享串。 */


export interface SharePlatform {
  aidog_platform_share: number;
  name: string;
  platform_type: Protocol;
  base_url: string;
  api_key: string;
  extra: string;
  models: PlatformModels;
  available_models: string[];
  endpoints: PlatformEndpoint[];
  manual_budgets: ManualBudget[];
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
  /** 用户自定义环境变量（内联 JSON 数组）。sync 时注入 settings.{group}.json
   * 的 env block；aidog 强写的 ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN 被跳过。 */
  env_vars: EnvVar[];
  /** 是否为默认分组（单选）：true 时该组 config merge 写入
   * ~/.claude/settings.json + ~/.codex/config.toml，使用户直接 claude/codex
   * 不带 -c/--profile 即走该组。 */
  is_default?: boolean;
  /** JSON 扩展字段（仿 platform.extra）。承载 `_ui_*` UI 态（卡片折叠等）。空串视作 {}。 */
  extra: string;
}


export interface GroupPlatformDetail {
  platform: Platform;
  priority: number;
  weight: number;
  /**
   * per-group 平台优先级（1~10，默认 5，10=最高优先；数大优先高）。
   * 后端返回时恒有值；前端乐观插入新明细时可省略（落库走 DEFAULT 5）。
   */
  level_priority?: number;
}


export interface ModelMapping {
  source_model: string;
  target_platform_id: number;
  target_model: string;
  /** 超时设置（秒），0 = 继承分组设置 */
  request_timeout_secs: number;
  connect_timeout_secs: number;
}

/** 内联于 group.env_vars JSON 数组的元素（用户自定义环境变量） */


export interface EnvVar {
  key: string;
  value: string;
}


export interface GroupPlatform {
  id: number;
  group_id: number;
  platform_id: number;
  priority: number;
  weight: number;
  /** per-group 平台优先级（1~10，默认 5，10=最高优先；数大优先高） */
  level_priority: number;
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
  bind_lan: boolean;
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
  /** 今日（本地 00:00 起）token 总量（input + output） */
  today_tokens: number;
  /** 今日（本地 00:00 起）预估花费（$） */
  today_cost: number;
}

/** 平台「最近一次测试结果」（来自 proxy_log 中 source_protocol='test' 的最新一条）。 */


export interface LastTestResult {
  /** status_code ∈ [200, 300) → true */
  success: boolean;
  status_code: number;
  duration_ms: number;
  /** proxy_log.created_at（毫秒 epoch） */
  created_at: number;
  /** 失败时取 response_body 截断 ~200 字符；成功为空串（短摘要） */
  error: string;
  /** 测试响应正文（成功/失败均带），截断 ~4000 字符；供前端 JSON 解析结构化展示 */
  response_body: string;
}

/** 从 platform.extra JSON 字符串解析 mock 配置（缺省字段回退默认值） */


export interface PlatformBreaker {
  failure_threshold: number;
  open_secs: number;
  half_open_max: number;
}

/** 从 platform.extra JSON 解析 breaker 覆盖（空/非法/缺键 → 全 0 继承全局默认）。 */


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
  input_tokens: number;
  output_tokens: number;
  cache_tokens: number;
  cache_rate: number;
  cost: number;
  total_requests: number;
}


export type PopoverItemType =
  | "today_cost"
  | "today_cache_rate"
  | "today_tokens"
  | "platform_today"
  | "proxy_status"
  | "platform_balance"
  | "cost_trend"
  | "platform_metric"
  | "group_cost"
  | "group_tokens"
  | "group_requests"
  | "group_balance";

/** cost_trend 卡片统计维度。 */


export type PopoverTrendScope = "overall" | "group" | "platform";
/** cost_trend 卡片时间窗。 */


export type PopoverTrendWindow = "today" | "7d" | "30d";

/** 卡片尺寸 / 内容密度：s=仅核心数值，m=当前样式，l=富信息。旧配置无此字段后端默认 "m"。 */


export type PopoverItemSize = "s" | "m" | "l";

/** Popover 浮窗单个展示项（预定义指标集内组合）。
 * 跨层字段名与 Rust serde（PopoverItem，无 rename）保持 snake_case 一致。 */


export interface PopoverItem {
  /** 稳定 id（前端生成，拖拽 key 用）。 */
  id: string;
  item_type: PopoverItemType;
  visible: boolean;
  order: number;
  /** cost_trend / platform_metric：统计维度（platform_metric 固定 "platform"）。旧配置无此字段后端默认 "overall"。 */
  scope?: PopoverTrendScope;
  /** scope!=overall：group → group_key；platform → platform_id 字符串。 */
  scope_ref?: string | null;
  /** cost_trend / platform_metric：时间窗。旧配置无此字段后端默认 "7d"。 */
  time_window?: PopoverTrendWindow;
  /** 二维布局行号。旧配置无此字段后端默认 0；渲染层 `row ?? order` fallback 老用户各占一行。 */
  row?: number;
  /** 卡片尺寸 / 内容密度。旧配置无此字段后端默认 "m"。 */
  size?: PopoverItemSize;
  /** 卡片数值颜色（复用 tray 三态颜色）。旧配置无此字段后端默认 follow。 */
  color?: TrayColor;
}

/** Popover 单行布局元信息（按 row 索引）。缺省 / 越界视为 cols=1。 */


export interface RowMeta {
  /** 该行列数 1 | 2 | 3。 */
  cols: 1 | 2 | 3;
}

/** Popover 浮窗整体配置（存 settings: scope="popover", key="config"）。 */


export interface PopoverConfig {
  items: PopoverItem[];
  /** 各行布局元信息（按 row 索引）；缺省项 / 越界视为 cols=1。旧配置无此字段。 */
  rows?: RowMeta[];
}

/** 单平台当日使用（popover「各平台当日」+ 设置预览）。 */


export interface TodayPlatformStat {
  platform_id: number;
  platform_name: string;
  tokens: number;
  cost: number;
  requests: number;
}

