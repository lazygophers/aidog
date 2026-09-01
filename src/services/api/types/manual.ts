// types/manual.ts — c1b (ts-rs codegen) 手写类型收敛文件。
//
// 本文件收纳「不该 / 不能由 ts-rs 从 Rust struct 派生」的类型，按理由分 6 节：
//   1. 锁定 enum（c4-protocol 并行改 protocol.rs，本轮禁加 #[derive(TS)]，Rust 端仍是唯一真值源）
//   2. camelCase DTO（跨层 snake_case 硬约束下唯一例外，Rust 侧本就 #[serde(rename_all = "camelCase")]）
//   3. 非 aidog_core::gateway::models crate（commands_* / aidog_core 其他子模块，本 subtask 授权编辑范围仅
//      gateway/models/**，越界不碰；多为 commands_platform/commands_system/commands_cli_env/commands_cli_proxy）
//   4. 前端派生 / 无 Rust 同名结构（字面量联合类型细化、或数据本就来自 serde_json::Value 内嵌子对象，
//      Rust 侧无强类型 struct 承载，只能手写）
//   5. aidog_core::gateway::models 内已知 drift / 不兼容豁免（ProxyLogDetail↔ProxyLog 字段差、
//      RequestLogSummary 的 #[serde(flatten)]，ts-rs 生成不出对应形状）
//   6. 其余待核实（尚无法 100% 确认 Rust 侧真实来源，暂手写占位）
//
// generated/ 下由 ts-rs `cargo test -p aidog_core` 产出的类型不放这里，见 generated/index.ts。
// 迁移完成：part1~5.ts + check-types.mjs 已删，Rust 侧 gateway/models/** 为唯一真值源。

import type { ProxyLogSummary } from "./generated/ProxyLogSummary";

// ─── 1. 锁定 enum（Protocol / RoutingMode / PlatformStatus）──────────────
// c4-protocol 正并行重构 src-tauri/crates/aidog_core/src/gateway/models/protocol.rs，
// 本轮禁止在该文件加 #[derive(TS)]。Rust 端字段引用改用
// `#[ts(type = "import(\"../manual\").Protocol")]` 等指回本文件，手工保持同步。

export type Protocol =
  // ── AI 请求协议（endpoint 协议）──
  | "anthropic" | "openai" | "openai_responses" | "openai_completions" | "gemini"
  // ── 平台类型 ──
  | "glm" | "glm_coding" | "glm_en" | "glm_coding_en" | "kimi" | "kimi_en" | "kimi_coding" | "minimax" | "minimax_en" | "minimax_coding" | "codex"
  | "bailian" | "bailian_en" | "bailian_coding" | "bailian_coding_en" | "qianfan_coding" | "xiaomi_mimo_coding" | "xiaomi_mimo_coding_en"
  // ── 国内官方平台 ──
  | "deepseek" | "stepfun" | "stepfun_en" | "doubao" | "byteplus" | "qianfan"
  | "xiaomi_mimo" | "longcat" | "sensenova" | "sensenova_en"
  // ── 聚合平台 ──
  | "openrouter" | "siliconflow" | "siliconflow_en" | "aihubmix" | "dmxapi" | "modelscope"
  | "shengsuanyun" | "atlascloud" | "novita" | "therouter" | "cherryin"
  // ── 第三方平台 ──
  | "packycode" | "cubence" | "aigocode" | "rightcode" | "aicodemirror" | "nvidia"
  | "pateway" | "ccsub" | "apikeyfun" | "sudocode" | "claudeapi" | "claudecn"
  | "runapi" | "relaxycode" | "crazyrouter" | "sssaicode" | "compshare" | "compshare_coding"
  | "micu" | "ctok" | "eflowcode" | "lemondata" | "pipellm" | "opencode" | "opencode_zen"
  // ── 中转平台 ──
  | "newapi"
  // ── 订阅透传 ──
  | "claude_code"
  // ── CLI 代理（cpa-standalone-module）：platform_type 仅标识，wire/base_url/api_key/models 由 candidate resolve 时从 cli_proxy_provider 表注入 ──
  | "cli-proxy"
  // ── Devin（Cognition）：特殊平台，接入走 handler.rs 平台分支不经 wire 协议层，preset 无标准 endpoint ──
  | "devin"
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

// ─── 2. camelCase DTO（跨层 snake_case 硬约束下的唯一例外）──────────────
// 以下均对应 Rust `#[serde(rename_all = "camelCase")]` 结构（mcp.rs / cli-proxy 中间层 /
// cc-switch 导入 / codex 解析），禁加 #[derive(TS)]（会与 snake_case 契约冲突），维持手写。

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

export interface Sub2ApiAccount {
  name: string;
  /** sub2api 原始 platform 值（小写），前端做 Protocol 映射。 */
  platform: string;
  apiKey?: string;
  baseUrl?: string;
}

export interface Sub2ApiReadResult {
  accounts: Sub2ApiAccount[];
}

// ─── 3. 非 aidog_core::gateway::models crate（越界不碰）────────────────
// 以下 Rust 定义已核实位置，均不在本 subtask 授权编辑范围（gateway/models/**）内：
// SharePlatform/FetchModelsError → commands_platform；AboutInfo → commands_system；
// CliInstallation/CliToolStatus/CliConflict → commands_cli_env；
// CliProxyImportFailure/CliProxyImportResult → commands_cli_proxy；
// CodingToolsSettings → commands_ai_tools；ProxySettings/TodayStats/TodayPlatformStat/
// AppLogSettings/MockConfig/BackupSettings/BackupResult/QuotaTier/BalanceInfo/
// CodingPlanInfo/PlatformQuota → aidog_core 内但在 gateway/models 之外
// （shared.rs / gateway/db/stats_today.rs / logging.rs / gateway/adapter/mock/config.rs /
// gateway/backup/mod.rs / gateway/quota/http.rs）。
// Skills/MCP 子系统（SkillInfo 等）→ commands_ai_tools，同样越界，一并归此节手写维护。

export interface SharePlatform {
  aidog_platform_share: number;
  name: string;
  platform_type: Protocol;
  base_url: string;
  api_key: string;
  extra: string;
  models: import("./generated/PlatformModels").PlatformModels;
  available_models: string[];
  endpoints: import("./generated/PlatformEndpoint").PlatformEndpoint[];
  manual_budgets: import("./generated/ManualBudget").ManualBudget[];
}

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

/** 单处安装（`which -a` / `where` 枚举 + canonicalize 去重 + source 推断）。 */
export interface CliInstallation {
  path: string;
  version: string | null;
  runnable: boolean;
  /** 安装来源：nvm / homebrew / volta / fnm / mise / bun / pnpm / scoop / pip / native / npm-global / system。 */
  source: string;
  /** 是否为 PATH 默认命中的那处（`which` / `where` 第一行）。 */
  is_path_default: boolean;
}

/** aidog 管理安装/升级的 CLI 客户端。与 Rust `cli_env::TOOLS` 一一对应。 */
export type CliTool = "claude" | "codex" | "pi";

/** 工具状态（claude / codex / pi）。 */
export interface CliToolStatus {
  name: string;
  installed: boolean;
  version: string | null;
  path: string | null;
  /** 装了但 `--version` 跑不起来（平台二进制损坏等）。 */
  broken: boolean;
  /** 多处安装且版本分歧或运行态混合（严阈值）。 */
  conflict: boolean;
  /** npm registry 最新版本（检测失败/离线时为 undefined）。 */
  latest_version?: string;
  /** 是否有更新可用（undefined=检测失败/离线，true=有更新，false=已是最新）。 */
  has_update?: boolean;
}

/** 冲突诊断结果。 */
export interface CliConflict {
  tool: string;
  installations: CliInstallation[];
  is_conflicting: boolean;
  /** 仅报告 + 建议，不自动卸载（破坏性操作禁主动执行）。 */
  suggestion: string;
}

/** `cli_proxy_import` 单条失败原因（非原子：成功入库，失败收集）。 */
export interface CliProxyImportFailure {
  name: string;
  error: string;
}

/** `cli_proxy_import` 跳过项（rar/7z、解析失败、无 cpa 段等）。 */
export interface CliProxyImportSkipReason {
  path: string;
  reason: string;
}

/** `cli_proxy_import` 返回。 */
export interface CliProxyImportResult {
  created: import("./generated/CliProxyProvider").CliProxyProvider[];
  failed: CliProxyImportFailure[];
  skipped: CliProxyImportSkipReason[];
  source_files: string[];
}

export interface CodingToolsSettings {
  apply_to_claude_plugin: boolean;
  skip_claude_onboarding: boolean;
}

/** 应用日志设置（对应 aidog_core::logging::AppLogSettings，logging.rs 在 gateway/models 之外）。 */
export interface AppLogSettings {
  file_enabled: boolean;
  level: string;
  retention_hours: number;
}

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

/** 已装 skill（直接读 `~/.agents/.skill-lock.json` + 探测本地 agent symlink 解析，一条/skill）。
 *  锁文件独有字段（source/sourceType/sourceUrl/skillFolderHash/pluginName/installedAt/updatedAt）
 *  从锁文件反序列化透出，旧缓存可能缺（值为 null，下次 refresh 回填）。 */
export interface SkillInfo {
  name: string;
  /** 已在哪些目标 agent（claude/codex 子集）启用。 */
  enabled_agents: SkillAgent[];
  scope: SkillScope;
  installed_path: string | null;
  description: string | null;
  /** 来源 owner/repo（锁文件 `source`）。第三方/手动 symlink skill（锁文件无条目）→ null。 */
  source: string | null;
  /** 来源类型（锁文件 `sourceType`，如 "github"/"gitlab"）。锁文件无 / 旧缓存 → null。 */
  source_type: string | null;
  /** 来源 git URL（锁文件 `sourceUrl`）。锁文件无 / 旧缓存 → null。 */
  source_url: string | null;
  /** skill 文件夹 hash（锁文件 `skillFolderHash`，sha1 hex，诊断用）。锁文件无 / 旧缓存 → null。 */
  skill_folder_hash: string | null;
  /** plugin 名（锁文件 `pluginName`，仅 plugin 安装来源有）。锁文件无 / 旧缓存 → null。 */
  plugin_name: string | null;
  /** 首次安装时间（锁文件 `installedAt`，ISO 8601）。锁文件无 / 旧缓存 → null。 */
  installed_at: string | null;
  /** 最近更新时间（锁文件 `updatedAt`，ISO 8601）。锁文件无 / 旧缓存 → null。 */
  updated_at: string | null;
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
 * - load_failed：true = list_refresh 中 npx 失败 / HOME 缺失，缓存未被更新（保留旧 items），
 *   前端应显「加载失败，显示上次缓存」提示（旧后端未返此字段，默认 false 兼容）。
 */
export interface CachedSkills {
  items: SkillInfo[];
  stale: boolean;
  load_failed?: boolean;
}

// ─── 4. 前端派生 / 无 Rust 同名结构 ─────────────────────────────────────
// 字面量联合细化（PopoverItemType 等）、或数据来自 serde_json::Value 内嵌子对象
// （platform.extra 的 mock/newapi/devin 子配置、manual_budget 的 kind/unit 字段），
// Rust 侧无强类型 struct/enum 承载，grep 全 crate 未命中同名定义，只能手写。

export type ClientType = string;

export type ModelSlot = "default" | "sonnet" | "opus" | "haiku" | "gpt";

/** 时段模型规则：按时段窗口切换主力模型档（time_windows） */
export interface TimeModelRule {
  /** 时段窗口列表（复用 TimeWindow 定义，multiplier 字段忽略） */
  windows: import("../../../domains/platforms/defaults").TimeWindow[];
  /** 5 槽模型配置（default/opus/sonnet/haiku/gpt） */
  models: import("./generated/PlatformModels").PlatformModels;
}

export type MockErrorMode = "none" | "http_error" | "rate_limit_429" | "timeout";

/** Mock 平台模拟配置（持久化在 platform.extra 的 `mock` 子对象内） */
export interface MockConfig {
  status_code: number;
  delay_ms: number;
  /** 首包时延（TTFT，毫秒）。undefined = 回落 delay_ms（向后兼容） */
  ttft_ms?: number;
  /** 流式 chunk 间隔（毫秒）。undefined = 回落 delay_ms（向后兼容） */
  inter_chunk_ms?: number;
  /** null = 跟随请求的 stream；true/false = 强制流式/非流式 */
  stream_override: boolean | null;
  response_text: string;
  finish_reason: string;
  input_tokens: number;
  output_tokens: number;
  cache_tokens: number;
  error_mode: MockErrorMode;
  /** 触发概率 0.0-1.0。undefined = 不启用（向后兼容） */
  error_rate?: number;
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

/** Devin（Cognition）平台配置（持久化在 platform.extra 的 `devin` 子对象内）。
 *  - org_id：编辑已移交配额查询脚本 requires 表单（quota-scripts T6），本类型不再承载；
 *    持久化仍在 `extra.devin.org_id`（proxy devin 路由 / 脚本嵌套优先读取）。
 *  - devin_timeout：可选，session 轮询超时秒数（默认 300，s6 后端读取）。
 *  - devin_mode：可选，默认 session 模式（normal/fast/lite/ultra/fusion）。 */
export interface DevinConfig {
  devin_timeout: string;
  devin_mode: string;
}

export type ManualBudgetKind = "total" | "rolling" | "fixed" | "daily";
/** 手动预算计量单位。count = 每请求扣 1（coding 套餐「N 次请求」口径）。 */
export type ManualBudgetUnit = "usd" | "token" | "count";

export interface ProxySettings {
  port: number;
  autostart: boolean;
  silent_launch: boolean;
  bind_lan: boolean;
}

/**
 * proxy_start 失败时 invoke() 的 reject 值（Rust `ProxyStartError`，proxy_cmd/proxy.rs）。
 * `kind` 区分「端口占用」与「其他绑定失败」；`message` 是英文调试信息，禁直接展示给用户
 * （用户可见文案走 i18n，按 kind + port 拼模板，见 proxy-port-no-drift/design.md）。
 */
export type ProxyStartErrorKind = "addr_in_use" | "other";
export interface ProxyStartError {
  kind: ProxyStartErrorKind;
  port: number;
  message: string;
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

/** 单平台当日使用（popover「各平台当日」+ 设置预览）。 */
export interface TodayPlatformStat {
  platform_id: number;
  platform_name: string;
  tokens: number;
  cost: number;
  requests: number;
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
 * 跨层字段名与 Rust serde（PopoverItem，无 rename）保持 snake_case 一致；
 * 注：Rust 端 item_type/scope/time_window/size 字段类型均为 String（非强类型 enum），
 * ts-rs 生成版会退化为 string，本手写版保留字面量联合供前端使用，两者字段名一致、类型细化不同。 */
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
  color?: import("./generated/TrayColor").TrayColor;
}

export type HookClient = "claude_code" | "codex";

export type ScriptExecutor = "uv" | "python3";

export interface DbCompactResult {
  before_bytes: number;
  after_bytes: number;
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

// ─── 5. aidog_core::gateway::models 内已知 drift / 不兼容豁免 ──────────
// 二者均在授权编辑范围内，但故意不加 #[derive(TS)]：
// - ProxyLogDetail：Rust 侧同义结构名为 `ProxyLog`（proxy_log.rs），且比 TS 手写版本多 3 字段
//   （blocked_by/blocked_reason/cli_proxy_provider_id），属 c1-typedrift 已知遗留 drift，本轮不碰。
// - RequestLogSummary：对应 Rust `#[serde(flatten)] ProxyLogSummary` + 2 字段，ts-rs 不支持
//   flatten 映射到 TS `extends`，故保留手写（用 TS `extends` 表达 flatten 语义）。

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
  attempts: import("./generated/ProxyAttempt").ProxyAttempt[];
  /** 重试次数 = attempts.length - 1（0 表示一次成功） */
  retry_count: number;
  created_at: number;
  updated_at: number;
  deleted_at: number;
}

/**
 * 请求日志页摘要行（对应 Rust `RequestLogSummary`）。
 * `#[serde(flatten)] ProxyLogSummary` + cli-proxy provider 归属信息。
 */
export interface RequestLogSummary extends ProxyLogSummary {
  /** proxy_log.cli_proxy_provider_id（走传统 platform 路由为 null） */
  cli_proxy_provider_id?: number | null;
  /** LEFT JOIN cli_proxy_provider.name；provider 已删 / 走 platform 路由均为 null */
  cli_proxy_provider_name?: string | null;
}

// ─── 6. 待核实（import/export 子系统，尚未定位 Rust 侧确切来源，占位手写）──

export type ImportExportScope =
  | "platform"
  | "group"
  | "group_platform"
  | "setting"
  | "codex"
  | "claude_code"
  | "model_price"
  | "mcp"
  | "middleware"
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

/** 单个可导入条目（前端逐项勾选）。scope+key 组合唯一标识，apply 时按白名单过滤。 */
export interface ImportItem {
  scope: string;
  key: string;
  /** 人类可读标签（平台名 / 分组名 / 设置键 / 文件名）。 */
  label: string;
  /** 是否与现有数据冲突（关联到 conflicts 决策子流程）。 */
  conflict: boolean;
}

export interface ImportPreview {
  manifest: ImportExportManifest;
  scopes: string[];
  conflicts: ConflictItem[];
  counts: Record<string, number>;
  /** 全部可导入条目（按 scope 分组逐项勾选）。 */
  items: ImportItem[];
}

export interface ImportReport {
  applied: Record<string, number>;
  skipped: Record<string, number>;
  errors: string[];
}
