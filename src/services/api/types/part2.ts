// types/part2.ts — 类型分片 2/4（arch-redesign），纯移动。
// 由 types.ts barrel 统一 re-export；外部应 `import type { X } from "../types"`，
// 不直接 import 本文件（分片边界为实现细节）。

import type { RoutingMode } from "./part1";

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
  /** 路径片段：对 request_url 做 LIKE %v% 模糊匹配 */
  path?: string;
}

// ─── Proxy Log API ─────────────────────────────────────────


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
  /**
   * 收件箱历史自动清理保留天数（default 7）。`0` = 不清理（永久保留）。
   * 后端硬删（参 proxy_log retention），旧配置无此字段 → 后端 serde 回退 7。
   */
  inbox_retention_days?: number;
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


export type HookClient = "claude_code" | "codex";

/** WebSpeech 播报请求事件名（payload = 文本；前端 webview SpeechSynthesis 朗读）。 */


export type ScriptExecutor = "uv" | "python3";



// ─── DB Maintenance (Tier 1: VACUUM reclaim) ──────────────

export interface DbCompactResult {
  before_bytes: number;
  after_bytes: number;
}



// ─── CLI 集成联动开关 ──────────────────────────

export interface CodingToolsSettings {
  apply_to_claude_plugin: boolean;
  skip_claude_onboarding: boolean;
}



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

