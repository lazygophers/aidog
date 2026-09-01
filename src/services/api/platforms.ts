// platforms.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { Protocol, PlatformStatus, PlatformEndpoint, PlatformModels, MockConfig, NewApiConfig, DevinConfig, ManualBudget, Platform, SharePlatform, PlatformUsageStats, LastTestResult, PlatformBreaker, ModelTestRequest, ModelTestResult, PlatformQuota, PriceSyncResult, TimeModelRule } from "./types";
import type { TimeWindow } from "../../domains/platforms/defaults";
import { normalizeWindow } from "../../utils/timeWindow";

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

/** Devin 平台默认配置（timeout/mode 可选；org_id 编辑已移交配额脚本 requires 表单）。
 *  timeout 用 string 与 number input 兼容。 */
export const DEFAULT_DEVIN_CONFIG: DevinConfig = {
  devin_timeout: "",
  devin_mode: "",
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

/** 从 platform.extra JSON 解析 Devin 配置（devin_timeout / devin_mode）。
 *  形态：`{"devin":{"org_id":"<id>","devin_timeout":300,"devin_mode":"normal"}}`。
 *  org_id 的编辑已移交配额查询脚本 requires 表单（quota-scripts T6），本对 parse/serialize
 *  不再读写它 —— serialize 保留存量嵌套 org_id 原样透传（proxy devin 路由仍读该键）。 */
export function parseDevinConfig(extra: string): DevinConfig {
  if (!extra.trim()) return { ...DEFAULT_DEVIN_CONFIG };
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "devin" in parsed) {
      const d = (parsed as { devin: unknown }).devin;
      if (d && typeof d === "object") {
        const o = d as Record<string, unknown>;
        return {
          devin_timeout: typeof o.devin_timeout === "number" ? String(o.devin_timeout) : (typeof o.devin_timeout === "string" ? o.devin_timeout : ""),
          devin_mode: typeof o.devin_mode === "string" ? o.devin_mode : "",
        };
      }
    }
  } catch { /* ignore */ }
  return { ...DEFAULT_DEVIN_CONFIG };
}

/** 把 Devin 配置写回 extra JSON（保留其余键）。
 *  存量嵌套 org_id 原样保留（本表单不再编辑）；全空且无存量 org_id → 移除整个 `devin` 键。 */
export function serializeDevinConfig(extra: string, cfg: DevinConfig): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* ignore */ }
  }
  const prev = obj.devin && typeof obj.devin === "object" && !Array.isArray(obj.devin)
    ? obj.devin as Record<string, unknown> : null;
  const orgId = prev && typeof prev.org_id === "string" ? prev.org_id : "";
  const timeoutNum = Math.max(0, Math.floor(Number(cfg.devin_timeout) || 0));
  const mode = cfg.devin_mode.trim();
  if (!orgId.trim() && timeoutNum === 0 && !mode) {
    delete obj.devin;
  } else {
    const devin: Record<string, unknown> = {};
    if (orgId.trim()) devin.org_id = orgId;
    if (timeoutNum > 0) devin.devin_timeout = timeoutNum;
    if (mode) devin.devin_mode = mode;
    obj.devin = devin;
  }
  return JSON.stringify(obj);
}

// ─── 配额查询脚本（quota-scripts T6）：变体选择 + requires 参数 ────────────

/** requires 参数的旧嵌套家（key → extra 子对象名）。脚本读值嵌套优先、顶层兜底（t3c 两层
 *  兜底），且 T4 基线 proxy devin 路由只读嵌套 —— 表单写顶层（schema 约定）同时镜像写嵌套，
 *  保证脚本 / proxy / 表单三处取值一致，旧数据不漂移。 */
const LEGACY_REQUIRES_NEST: Record<string, string> = {
  org_id: "devin",
  balance_base_url: "newapi",
  balance_api_key: "newapi",
};

/** quota 脚本表单态（platform.extra 持久化，见 parse/serializeQuotaScriptConfig）。 */
export interface QuotaScriptFormConfig {
  /** 选中变体 id（registry quota_scripts[].id；"" = 未显式选择，后端物化回落首条）。 */
  variantId: string;
  /** 自定义脚本正文（非空 = 自定义伪变体，物化时覆盖 id 选择）。 */
  customScript: string;
  /** requires 参数值（key → 用户输入；含 newapi user_id 等非 requires 附加键）。 */
  requires: Record<string, string>;
}

/** 从 platform.extra 解析变体选择（id / 自定义脚本正文；requires 值走 readRequiresValue）。 */
export function parseQuotaScriptConfig(extra: string): { variantId: string; customScript: string } {
  if (!extra.trim()) return { variantId: "", customScript: "" };
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const o = parsed as Record<string, unknown>;
      return {
        variantId: typeof o.quota_script_id === "string" ? o.quota_script_id : "",
        customScript: typeof o.quota_custom_script === "string" ? o.quota_custom_script : "",
      };
    }
  } catch { /* ignore */ }
  return { variantId: "", customScript: "" };
}

/** extra 是否带非空自定义配额脚本（伪变体）。 */
export function hasCustomQuotaScript(extra: string): boolean {
  return parseQuotaScriptConfig(extra).customScript.trim() !== "";
}

/** 读 requires 参数值：嵌套优先（extra.newapi / extra.devin 子对象）→ 顶层兜底，
 *  同脚本/Rust 取值语义（registry 脚本与 gateway/proxy/devin.rs resolve_devin_org_id）：
 *  嵌套缺失 / 空串 / 非字符串都回落顶层。全缺失 / 非法 → ""。 */
export function readRequiresValue(extra: string, key: string): string {
  if (!extra.trim()) return "";
  try {
    const parsed: unknown = JSON.parse(extra);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return "";
    const o = parsed as Record<string, unknown>;
    for (const nest of ["newapi", "devin"]) {
      const home = o[nest];
      if (home && typeof home === "object" && !Array.isArray(home)) {
        const v = (home as Record<string, unknown>)[key];
        if (typeof v === "string" && v !== "") return v;
      }
    }
    const top = o[key];
    return typeof top === "string" ? top : "";
  } catch { /* ignore */ }
  return "";
}

/** 把变体选择 + requires 参数写回 extra JSON（保留其余键）。互斥规则（与后端物化对齐）：
 *  - customScript 非空 → 写 quota_custom_script、删 quota_script_id（custom 优先）；
 *  - 否则 → 删 quota_custom_script；id 命中 variants 才写 quota_script_id，
 *    缺省 / 失效 id 不写（后端 resolve/materialize 回落首条）。
 *  requires 仅写选中变体（custom 无元数据，不动 requires 键）：值写顶层 + 镜像写旧嵌套家；
 *  置空则顶层与嵌套同删（清掉旧嵌套脏值，防嵌套优先读到陈旧值）。
 *  newapi 的 user_id（非 requires、回填目标字段，spec user story 7）单独写 extra.newapi.user_id。 */
export function serializeQuotaScriptConfig(
  extra: string,
  cfg: QuotaScriptFormConfig,
  variants: Array<{ id: string; requires: string[] }>,
  protocol: string,
): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* ignore */ }
  }
  const nestOf = (obj2: Record<string, unknown>, nest: string): Record<string, unknown> | null => {
    const home = obj2[nest];
    if (home && typeof home === "object" && !Array.isArray(home)) return home as Record<string, unknown>;
    return null;
  };
  const custom = cfg.customScript.trim();
  if (custom) {
    obj.quota_custom_script = cfg.customScript;
    delete obj.quota_script_id;
  } else {
    delete obj.quota_custom_script;
    const sel = variants.find(v => v.id === cfg.variantId) ?? variants[0];
    if (cfg.variantId && variants.some(v => v.id === cfg.variantId)) {
      obj.quota_script_id = cfg.variantId;
    } else {
      delete obj.quota_script_id;
    }
    if (sel) {
      for (const key of sel.requires) {
        const val = (cfg.requires[key] ?? "").trim();
        const nest = LEGACY_REQUIRES_NEST[key];
        if (val) {
          obj[key] = val;
          if (nest) {
            const home = (obj[nest] ?? {}) as Record<string, unknown>;
            obj[nest] = { ...home, [key]: val };
          }
        } else {
          delete obj[key];
          if (nest) {
            const home = nestOf(obj, nest);
            if (home) delete home[key];
          }
        }
      }
    }
  }
  // newapi user_id：旧表单字段保留（查询结果回填目标，读取方暂缺 — 回填链现状见 notes/01）。
  if (protocol === "newapi") {
    const uid = (cfg.requires.user_id ?? "").trim();
    if (uid) {
      const home = (obj.newapi ?? {}) as Record<string, unknown>;
      obj.newapi = { ...home, user_id: uid };
    } else {
      const home = nestOf(obj, "newapi");
      if (home) delete home.user_id;
    }
  }
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

/** 从 platform.extra JSON 解析 peak 窗口（用户覆盖）。
 *  缺失 / 非法 / 空数组 → []（caller 退 preset 默认或 1.0）。 */
export function parsePlatformPeak(extra: string): TimeWindow[] {
  if (!extra.trim()) return [];
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "peak" in parsed) {
      const arr = (parsed as { peak: unknown }).peak;
      if (Array.isArray(arr)) return (arr as TimeWindow[]).map(normalizeWindow);
    }
  } catch { /* ignore */ }
  return [];
}

/** 把 peak 窗口写回 extra JSON（保留其余键）。空数组 → 移除 peak 键（无覆盖→用 preset 默认）。 */
export function serializePlatformPeak(extra: string, windows: TimeWindow[]): string {
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
    delete obj.peak;
  } else {
    obj.peak = windows;
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


/** 从 platform.extra JSON 解析 time_windows 规则（用户级配置，preset 不带）。
 *  缺失 / 非法 / 空数组 → []（无时段规则，用 platform.models default）。 */
export function parsePlatformTimeWindows(extra: string): TimeModelRule[] {
  if (!extra.trim()) return [];
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "time_windows" in parsed) {
      const arr = (parsed as { time_windows: unknown }).time_windows;
      if (Array.isArray(arr)) {
        return (arr as TimeModelRule[]).map((rule) => ({ ...rule, windows: rule.windows.map(normalizeWindow) }));
      }
    }
  } catch { /* ignore */ }
  return [];
}

/** 把 time_windows 规则写回 extra JSON（保留其余键）。空数组 → 移除 time_windows 键（无规则→用 default）。 */
export function serializePlatformTimeWindows(extra: string, rules: TimeModelRule[]): string {
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
    delete obj.time_windows;
  } else {
    obj.time_windows = rules;
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

  /** 只读预览「一键清理失效平台」将处理的候选清单，与 purgeDisabled 共用同一筛选条件。
   *  reason: "auth_failed"（401/403 认证失效）| "expired"（已过期）
   *  action: "delete"（永久删除）| "unassign"（仅移出本分组，仅分组模式可能出现） */
  purgeDisabledPreview: (groupId?: number) =>
    invoke<PurgeCandidate[]>("platform_purge_disabled_preview", {
      groupId: groupId ?? null,
    }),

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

/** 一键清理候选行（对应 Rust PurgeCandidate，serde rename_all = "camelCase"）。 */
export interface PurgeCandidate {
  id: number;
  name: string;
  reason: "auth_failed" | "expired";
  action: "delete" | "unassign";
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
  queryDevin: (baseUrl: string, apiKey: string, extra: string, platformId?: number) =>
    invoke<PlatformQuota>("platform_query_quota_devin", { baseUrl, apiKey, extra, platformId: platformId ?? null }),
};

// ─── Model Price Types & API ──────────────────────────────


/** registry 同步（platform.json 品牌/端点 + 逐平台模型条目整份拉取入库）。
 *  command 名保留 `model_price_` 前缀的理由见 `platform_cmd/price.rs` 顶部注释
 *  （与之成组的 setting key `price_sync` 已持久化在用户 DB，单改命令名反而更不一致）。
 *  旧的 list / count / search / listFiltered / countFiltered / resolve 随 `model_price`
 *  表 DROP 一并删除；模型清单与价格改走 `modelEntryApi` / `modelInfoApi`。 */
export const modelPriceApi = {
  sync: () =>
    invoke<PriceSyncResult>("model_price_sync"),
};

/** 平台默认配置与品牌字段（endpoints / models / model_list / client_type / name / logo_url /
 *  color / homepage / keywords / source_urls），唯一真值源 = registry
 *  （`src-tauri/defaults/registry/`，编译期 include，见 `aidog_db::registry::presets_json`）。
 *  返回原始 JSON 字符串，前端解析缓存。 */
export function getDefaultsJson(): Promise<string> {
  return invoke<string>("get_defaults_json");
}

/** 客户端类型字典（13 entry，name/desc 多 locale），内置 const
 *  （Rust `client_types_const.rs`）。返回原始 JSON 字符串，前端解析缓存（禁直读文件系统，一律 invoke）。 */
export function getClientTypesJson(): Promise<string> {
  return invoke<string>("get_client_types_json");
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

