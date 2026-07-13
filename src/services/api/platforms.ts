// platforms.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { Protocol, PlatformStatus, PlatformEndpoint, PlatformModels, MockConfig, NewApiConfig, ManualBudget, Platform, SharePlatform, PlatformUsageStats, LastTestResult, PlatformBreaker, ModelTestRequest, ModelTestResult, PlatformQuota, ModelPriceSummary, ResolvedPrice, PriceSyncResult, ModelPriceFilter, TimeModelRule, MappedPlatform, CpaImportParseResult, CpaBatchReport } from "./types";
import type { PeakWindow } from "../../domains/platforms/defaults";

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

/** 从 platform.extra JSON 解析 peak_hours 窗口（用户覆盖）。
 *  缺失 / 非法 / 空数组 → []（caller 退 preset 默认或 1.0）。 */
export function parsePlatformPeakHours(extra: string): PeakWindow[] {
  if (!extra.trim()) return [];
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "peak_hours" in parsed) {
      const arr = (parsed as { peak_hours: unknown }).peak_hours;
      if (Array.isArray(arr)) return arr as PeakWindow[];
    }
  } catch { /* ignore */ }
  return [];
}

/** 把 peak_hours 窗口写回 extra JSON（保留其余键）。空数组 → 移除 peak_hours 键（无覆盖→用 preset 默认）。 */
export function serializePlatformPeakHours(extra: string, windows: PeakWindow[]): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* ignore */ }
  }
  if (windows.length === 0) {
    delete obj.peak_hours;
  } else {
    obj.peak_hours = windows;
  }
  return JSON.stringify(obj);
}

/** 从 platform.extra JSON 解析 disable_during_peak 开关（用户覆盖）。
 *  缺失 / 非法 / 非布尔 → false（默认）。与 Rust parse_disable_during_peak 对称。 */
export function parseDisableDuringPeak(extra: string): boolean {
  if (!extra.trim()) return false;
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "disable_during_peak" in parsed) {
      const v = (parsed as { disable_during_peak: unknown }).disable_during_peak;
      return v === true; // 严格布尔：数字/字符串不误判
    }
  } catch { /* ignore */ }
  return false;
}

/** 把 disable_during_peak 写回 extra JSON（保留其余键）。false → 移除键（默认行为，无覆盖）。 */
export function serializeDisableDuringPeak(extra: string, enabled: boolean): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* ignore */ }
  }
  if (enabled) {
    obj.disable_during_peak = true;
  } else {
    delete obj.disable_during_peak;
  }
  return JSON.stringify(obj);
}

/** 从 platform.extra JSON 解析 time_models 规则（用户级配置，preset 不带）。
 *  缺失 / 非法 / 空数组 → []（无时段规则，用 platform.models default）。 */
export function parsePlatformTimeModels(extra: string): TimeModelRule[] {
  if (!extra.trim()) return [];
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "time_models" in parsed) {
      const arr = (parsed as { time_models: unknown }).time_models;
      if (Array.isArray(arr)) return arr as TimeModelRule[];
    }
  } catch { /* ignore */ }
  return [];
}

/** 把 time_models 规则写回 extra JSON（保留其余键）。空数组 → 移除 time_models 键（无规则→用 default）。 */
export function serializePlatformTimeModels(extra: string, rules: TimeModelRule[]): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* ignore */ }
  }
  if (rules.length === 0) {
    delete obj.time_models;
  } else {
    obj.time_models = rules;
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
    /** 自动创建的默认分组的 level_priority 初值（1~10）；仅当归属唯一分组时由表单传入。 */
    default_level_priority?: number;
    /** 过期时间（毫秒 unix 时间戳，0 = 永不过期；>0 到期后路由排除）。 */
    expires_at?: number;
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
    /** 过期时间（毫秒 unix 时间戳）。0 = 清空（永不过期）；>0 到期后路由排除。 */
    expires_at?: number;
  }) => invoke<Platform>("platform_update", { input }),

  delete: (id: number) => invoke<void>("platform_delete", { id }),

  /** 一键清理失效（auto_disabled）平台。
   *  - 不传 groupId：全局，删全库 auto_disabled 平台（永久删除，复用后端 delete_platform）。
   *  - 传 groupId：分组级，独占本分组的永久删除，共享（属多分组）的仅从本分组移除关联（platform 行保留）。
   *  返回 { deletedIds, unassignedIds }：deletedIds = 被永久删除的平台 id；unassignedIds = 仅移除本分组关联的平台 id。 */
  purgeDisabled: (groupId?: number) =>
    invoke<{ deletedIds: number[]; unassignedIds: number[] }>(
      "platform_purge_disabled",
      { groupId: groupId ?? null },
    ),

  /** 为平台补建默认 auto 分组（若已存在则跳过）。供批量导入回挂复用（cc-switch / 导入）。 */
  ensureAutoGroup: (id: number) => invoke<void>("platform_ensure_auto_group", { id }),

  /** 拖拽排序：传入按新顺序排列的 platform id 列表 */
  reorder: (orderedIds: number[]) =>
    invoke<void>("platform_reorder", { orderedIds }),

  fetchModels: (protocol: Protocol, baseUrl: string, apiKey: string) =>
    invoke<string[]>("platform_fetch_models", { protocol, baseUrl, apiKey }),

  usageStats: (platformId: number) =>
    invoke<PlatformUsageStats>("platform_usage_stats", { platformId }),

  // 批量：单次 invoke 返回所有平台 → 聚合 map（platform_id → stats），消除前端逐平台 N+1 往返。
  // 后端 GROUP BY eff_pid，含 platform_id=0 自动分组日志按 group_key 回溯归属源平台；
  // 回溯不到的（未知平台）不入 map。JSON 对象键为字符串，按 number 平台 id 索引。
  usageStatsAll: () =>
    invoke<Record<number, PlatformUsageStats>>("all_platform_usage_stats"),

  /** 取该平台最近一次 model_test 结果（无测试记录返回 null）。 */
  lastTestResult: (platformId: number) =>
    invoke<LastTestResult | null>("get_last_test_result", { platformId }),

  /** 导出单平台可分享配置（结构化对象，含明文 api_key）。前端按 YAML / JSON / Base64 转换。 */
  shareExport: (platformId: number) =>
    invoke<SharePlatform>("platform_share_export", { platformId }),

  /** 解析分享串（YAML / JSON 通吃）；非合法 aidog 分享串 throw → 调用方 fallback 原杂乱文本解析。 */
  shareParse: (text: string) =>
    invoke<SharePlatform>("platform_share_parse", { text }),

  /** 批量删除平台（物理删 = 软删 platform + 清所有 group_platform 关联）。
   *  原子事务：任一失败 → 全部 rollback（applied=0 或全 N）。 */
  batchDelete: (ids: number[]) =>
    invoke<BatchReport>("batch_delete_platforms", { ids }),

  /** 批量覆盖平台 models（5 槽整体覆盖；原子事务：任一失败 → 全部 rollback）。 */
  batchOverrideModels: (ids: number[], models: PlatformModels) =>
    invoke<BatchReport>("batch_override_models", { ids, models }),

  /** 批量设置平台 status（仅 enabled/disabled，拒 auto_disabled；原子事务）。 */
  batchSetStatus: (ids: number[], status: "enabled" | "disabled") =>
    invoke<BatchReport>("batch_set_status", { ids, status }),

  /** 批量移组/加组（原子事务：任一失败 → 全部 rollback）。
   *  mode="move": 从所有现组移除 + 加目标组；mode="add": 仅加目标组保留现组。 */
  batchMoveGroup: (ids: number[], targetGroupId: number, mode: "move" | "add") =>
    invoke<BatchReport>("batch_move_group", { ids, targetGroupId, mode }),
};

/** 批量操作结果（对应 Rust BatchReport，serde rename_all = "camelCase"）。 */
export interface BatchReport {
  applied: number;
  skipped: { id: number; reason: string }[];
}

/** 系统托盘 quota 展示（互斥单平台） */


export const modelTestApi = {
  test: (req: ModelTestRequest) =>
    invoke<ModelTestResult>("model_test", { req }),
};

// ─── Platform Quota Types & API ────────────────────────────


export const quotaApi = {
  query: (baseUrl: string, apiKey: string, platformId?: number) =>
    invoke<PlatformQuota>("platform_query_quota", { baseUrl, apiKey, platformId: platformId ?? null }),
  queryNewapi: (baseUrl: string, apiKey: string, extra: string, platformId?: number) =>
    invoke<PlatformQuota>("platform_query_quota_newapi", { baseUrl, apiKey, extra, platformId: platformId ?? null }),
};

// ─── CPA(CLIProxyAPI) 配置导入 ────────────────────────────
// 三段式：parse(解析+映射，纯读) → preview_quota(临时查余额，不落库) → apply(逐个建平台，非原子)。
// 字段对齐 Rust `commands_platform/src/cpa_import.rs` serde。

export const cpaImportApi = {
  /** 解析 CPA 配置文件/压缩包/目录（+ 可选 auth-dir）→ MappedPlatform[]。纯读，不建平台。 */
  parse: (path: string, authDir?: string) =>
    invoke<CpaImportParseResult>("cpa_import_parse", { path, authDir: authDir ?? null }),
  /** 预览期临时查余额，不落库（platform_id=0 → persist_quota_to_db None-guard）。
   *  仅 9 provider 有值，不支持者 success=false 前端显「—」。 */
  previewQuota: (baseUrl: string, apiKey: string) =>
    invoke<PlatformQuota>("cpa_import_preview_quota", { baseUrl, apiKey }),
  /** 批量创建平台（非原子尽力：逐个 platform_create，失败收集不中断）。 */
  apply: (platforms: MappedPlatform[]) =>
    invoke<CpaBatchReport>("cpa_import_apply", { platforms }),
};

// ─── Model Price Types & API ──────────────────────────────


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

/** 平台默认配置（endpoints / models / model_list / client_type），来自 bundled
 *  `defaults/platform-presets.json`，运行时可被 `~/.aidog/platform-presets.json` 覆盖（同步链写入）。
 *  返回原始 JSON 字符串，前端解析缓存。 */
export function getDefaultsJson(): Promise<string> {
  return invoke<string>("get_defaults_json");
}

export type DefaultsSyncResult = {
  updated: boolean;
  lastUpdated: number;
  source: "jsdelivr" | "raw" | "local";
  error?: string;
  /** 启动 hook 检测到用户手工修改 app data 后跳过同步时为 true；
   *  手动按钮路径恒 false。R3.5 / R4.1，Rust serde camelCase 对齐。
   *  可选：旧后端响应无此字段 → undefined（向后兼容）。 */
  userModified?: boolean;
};

/** 手动触发 platform-presets.json 同步（无视节流，jsDelivr 主 + raw fallback）。
 *  返回 {updated, lastUpdated, source, error} — Rust side serde camelCase 已对齐。 */
export function syncDefaultsJson(): Promise<DefaultsSyncResult> {
  return invoke<DefaultsSyncResult>("sync_defaults_json");
}

/** 客户端类型字典（13 entry，name/desc 多 locale），来自 bundled
 *  `defaults/client-types.json`，运行时可被 `~/.aidog/client-types.json` 覆盖（同步链写入）。
 *  返回原始 JSON 字符串，前端解析缓存（禁直读 github / 文件系统，一律 invoke）。 */
export function getClientTypesJson(): Promise<string> {
  return invoke<string>("get_client_types_json");
}

export type ClientTypesSyncResult = {
  updated: boolean;
  lastUpdated: number;
  source: "jsdelivr" | "raw" | "local";
  error?: string;
  /** 启动 hook 检测到用户手工修改 app data 后跳过同步时为 true；手动按钮路径恒 false。 */
  userModified?: boolean;
};

/** 手动触发 client-types.json 同步（无视节流）。 */
export function syncClientTypesJson(): Promise<ClientTypesSyncResult> {
  return invoke<ClientTypesSyncResult>("sync_client_types_json");
}

/** 返回 protocol logo 缓存文件绝对路径（前端 `convertFileSrc` 用）。
 *  文件不存在 / size=0 返空串（调用方 fallback 首字母圆圈）。 */
export function getProtocolLogoPath(protocol: Protocol): Promise<string> {
  return invoke<string>("get_protocol_logo_path", { protocol });
}

/** 触发单 protocol 后台 logo 同步（懒加载 miss 时调）。非阻塞 spawn。 */
export function syncProtocolLogo(protocol: Protocol): Promise<void> {
  return invoke<void>("sync_protocol_logo", { protocol });
}

