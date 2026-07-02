// types/part3.ts — 类型分片 3/4（arch-redesign），纯移动。
// 由 types.ts barrel 统一 re-export；外部应 `import type { X } from "../types"`，
// 不直接 import 本文件（分片边界为实现细节）。


// ─── Stats Settings (聚合表 retention) ────────────────────

export interface StatsSettings {
  /** 聚合统计行保留天数；0 = 永久保留。默认 365。 */
  retention_days: number;
}



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



// ─── 导入导出子系统 ───────────────────────────────────────

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

