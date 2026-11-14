// platforms.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "../transport";
import type { Protocol, PlatformStatus, PlatformEndpoint, PlatformModels, MockConfig, NewApiConfig, DevinConfig, PassthroughAuth, ManualBudget, Platform, SharePlatform, PlatformUsageStats, LastTestResult, PlatformBreaker, ModelTestRequest, ModelTestResult, PlatformQuota, TimeModelRule, PlatformErrorStatus, ClientVocabCategory } from "./types";
import type { TimeWindow } from "../../domains/platforms/defaults";
import { normalizeWindow } from "../../utils/timeWindow";
import QUOTA_SCRIPT_SAMPLE_RAW from "../../domains/platforms/quotaScriptSample.txt?raw";

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

/** 自定义透传默认认证：Authorization: Bearer {key}（共识稿 2026-10-08 §2）。 */
export const DEFAULT_PASSTHROUGH_AUTH: PassthroughAuth = {
  auth_header: "Authorization",
  auth_template: "Bearer {key}",
};

/** 从 platform.extra JSON 解析透传认证配置（顶层 auth_header / auth_template 键）。 */
export function parsePassthroughAuth(extra: string): PassthroughAuth {
  const fallback = { ...DEFAULT_PASSTHROUGH_AUTH };
  if (!extra.trim()) return fallback;
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const o = parsed as Record<string, unknown>;
      return {
        auth_header: typeof o.auth_header === "string" && o.auth_header.trim() ? o.auth_header : fallback.auth_header,
        auth_template: typeof o.auth_template === "string" && o.auth_template.trim() ? o.auth_template : fallback.auth_template,
      };
    }
  } catch { /* 非法 JSON → 回退默认 */ }
  return fallback;
}

/** 把透传认证配置写回 extra JSON 顶层（保留其余键；值即默认时删键，保持 extra 干净）。 */
export function serializePassthroughAuth(extra: string, cfg: PassthroughAuth): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* 非法 JSON → 重建 */ }
  }
  const header = cfg.auth_header.trim();
  const template = cfg.auth_template.trim();
  const isDefault = header === DEFAULT_PASSTHROUGH_AUTH.auth_header && template === DEFAULT_PASSTHROUGH_AUTH.auth_template;
  if (!header || isDefault) {
    delete obj.auth_header;
    delete obj.auth_template;
  } else {
    obj.auth_header = header;
    obj.auth_template = template;
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

/** 自定义脚本编辑器的默认样例（与 Rust 测试共用同一文件，保证样例真能跑）。 */
export const QUOTA_SCRIPT_SAMPLE = QUOTA_SCRIPT_SAMPLE_RAW;

/** 用户是否真写了自定义脚本：非空且不等于未改动的样例（样例原样 = 视为没配置）。
 *  取舍：与样例逐字相同（trim 后）的脚本被当作未配置。与 hasCustomQuotaScript 同口径，改一处要看另一处。 */
export function isUserQuotaScript(script: string): boolean {
  const s = script.trim();
  return s !== "" && s !== QUOTA_SCRIPT_SAMPLE.trim();
}

/** 自定义伪变体下拉哨兵值（radix Select 禁空串 value；选中时表单切到 customScript 编辑）。 */
export const QUOTA_CUSTOM_VARIANT = "__custom__";

/** 脚本侧选中值：自定义正文优先 → 显式变体 id → 回落首个变体；无变体恒自定义。 */
export function resolveQuotaSelection(customScript: string, variantId: string, variants: { id: string }[]) {
  const hasCustom = isUserQuotaScript(customScript);
  const idValid = variants.some(v => v.id === variantId);
  const customOnly = variants.length === 0;
  const selection = hasCustom || customOnly || variantId === QUOTA_CUSTOM_VARIANT
    ? QUOTA_CUSTOM_VARIANT
    : idValid ? variantId : (variants[0]?.id ?? "");
  const fellBack = !hasCustom && variantId !== QUOTA_CUSTOM_VARIANT && variantId !== "" && !idValid;
  return { selection, customOnly, fellBack };
}

/** extra 是否带非空自定义配额脚本（伪变体）。判定口径见 isUserQuotaScript，两者须同步。 */
export function hasCustomQuotaScript(extra: string): boolean {
  return isUserQuotaScript(parseQuotaScriptConfig(extra).customScript);
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
  if (isUserQuotaScript(cfg.customScript)) {
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
  const zero: PlatformBreaker = { failure_threshold: 0, half_open_max: 0 };
  if (!extra.trim()) return zero;
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "breaker" in parsed) {
      const b = (parsed as { breaker: unknown }).breaker;
      if (b && typeof b === "object") {
        const o = b as Record<string, unknown>;
        return {
          failure_threshold: typeof o.failure_threshold === "number" ? o.failure_threshold : 0,
          half_open_max: typeof o.half_open_max === "number" ? o.half_open_max : 0,
        };
      }
    }
  } catch {
    /* 非法 JSON → 回退全 0 */
  }
  return zero;
}

/** 把 breaker 覆盖写回 extra JSON（保留其余键）。两值全 0 → 移除 breaker 键（无覆盖=继承全局）。 */


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
  if (b.failure_threshold === 0 && b.half_open_max === 0) {
    delete obj.breaker;
  } else {
    obj.breaker = b;
  }
  return JSON.stringify(obj);
}

/** 从 platform.extra 解析用户覆盖的节假日日历；缺失时继承 registry 默认。 */
export function parsePlatformHolidayCalendar(extra?: string): string | undefined {
  if (!extra?.trim()) return undefined;
  try {
    const value = (JSON.parse(extra) as Record<string, unknown>).holiday_calendar;
    return typeof value === "string" && value.trim() ? value : undefined;
  } catch {
    return undefined;
  }
}

export function serializePlatformHolidayCalendar(extra: string, calendar: string | undefined): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) obj = parsed as Record<string, unknown>;
    } catch { /* ignore */ }
  }
  if (calendar) obj.holiday_calendar = calendar;
  else delete obj.holiday_calendar;
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

/** 从 platform.extra JSON 解析直通模型列表（直接模型转发）。缺失 / 非法 / 含非字符串 → 过滤后返回。 */
export function parseDirectModels(extra: string): string[] {
  if (!extra.trim()) return [];
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "direct_models" in parsed) {
      const arr = (parsed as { direct_models: unknown }).direct_models;
      if (Array.isArray(arr)) return arr.filter((x): x is string => typeof x === "string" && x.length > 0);
    }
  } catch { /* ignore */ }
  return [];
}

/** 把直通模型列表写回 extra JSON（保留其余键）。空数组 → 移除键（未声明 = 走默认链）。 */
export function serializeDirectModels(extra: string, models: string[]): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* ignore */ }
  }
  const clean = models.map(m => m.trim()).filter(m => m.length > 0);
  if (clean.length === 0) {
    delete obj.direct_models;
  } else {
    obj.direct_models = clean;
  }
  return JSON.stringify(obj);
}

/** 从 platform.extra JSON 解析客户端白名单（词表码，大类/变体混合）。缺失 / 非法 → []（= 全放行）。 */
export function parseAllowedClients(extra: string): string[] {
  if (!extra.trim()) return [];
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "allowed_clients" in parsed) {
      const arr = (parsed as { allowed_clients: unknown }).allowed_clients;
      if (Array.isArray(arr)) return arr.filter((x): x is string => typeof x === "string" && x.length > 0);
    }
  } catch { /* ignore */ }
  return [];
}

/** 把客户端白名单写回 extra JSON（保留其余键）。空数组 → 移除键（= 取消白名单）。 */
export function serializeAllowedClients(extra: string, codes: string[]): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* ignore */ }
  }
  if (codes.length === 0) {
    delete obj.allowed_clients;
  } else {
    obj.allowed_clients = codes;
  }
  return JSON.stringify(obj);
}

export type ClientUaRuleKind = "contains" | "prefix" | "exact" | "regex";
export interface ClientUaRule { kind: ClientUaRuleKind; pattern: string }

const UA_RULE_KINDS: readonly string[] = ["contains", "prefix", "exact", "regex"];

/** 从 platform.extra 解析白名单 UA 规则；缺失 / 非法 → []，形状不对的项丢弃。 */
export function parseClientUaRules(extra: string): ClientUaRule[] {
  if (!extra.trim()) return [];
  try {
    const arr = (JSON.parse(extra) as { client_ua_rules?: unknown } | null)?.client_ua_rules;
    if (!Array.isArray(arr)) return [];
    return arr.flatMap((r): ClientUaRule[] =>
      r && typeof r === "object" && UA_RULE_KINDS.includes((r as ClientUaRule).kind) && typeof (r as ClientUaRule).pattern === "string"
        ? [{ kind: (r as ClientUaRule).kind, pattern: (r as ClientUaRule).pattern }]
        : []);
  } catch { return []; }
}

/** 把 UA 规则写回 extra（保留其余键）；空数组 → 删键。 */
export function serializeClientUaRules(extra: string, rules: ClientUaRule[]): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) obj = parsed as Record<string, unknown>;
    } catch { /* ignore */ }
  }
  if (rules.length === 0) delete obj.client_ua_rules;
  else obj.client_ua_rules = rules;
  return JSON.stringify(obj);
}

/** 即时校验：空 pattern -> "empty"；非法正则 -> "regex"。
 *  shortcut: 正则用 JS RegExp 校验，与后端 Rust regex 语法有细微差异（如 lookahead），后端保存校验为准。 */
export function validateClientUaRule(r: ClientUaRule): "empty" | "regex" | null {
  if (!r.pattern.trim()) return "empty";
  if (r.kind === "regex") {
    try { new RegExp(r.pattern, "i"); } catch { return "regex"; }
  }
  return null;
}

/** 大类勾选态：大类码在 = 全选；否则按变体命中数 some/none（变体全在也算 all）。 */
export function clientCategoryState(sel: string[], cat: ClientVocabCategory): "all" | "some" | "none" {
  if (sel.includes(cat.code)) return "all";
  const n = cat.variants.filter(v => sel.includes(v.code)).length;
  if (n === 0) return "none";
  return n === cat.variants.length ? "all" : "some";
}

/** 勾 / 取消大类：勾 = 以大类码替换其变体（含全部变体）；取消 = 大类与变体都清掉。 */
export function toggleClientCategory(sel: string[], cat: ClientVocabCategory, on: boolean): string[] {
  const drop = new Set([cat.code, ...cat.variants.map(v => v.code)]);
  const rest = sel.filter(c => !drop.has(c));
  return on ? [...rest, cat.code] : rest;
}

/** 勾 / 取消单个变体：大类态下取消一个 = 展开为其余变体；勾满全部变体 = 折叠为大类码。 */
export function toggleClientVariant(sel: string[], cat: ClientVocabCategory, variant: string, on: boolean): string[] {
  const all = cat.variants.map(v => v.code);
  const cur = sel.includes(cat.code) ? all : all.filter(c => sel.includes(c));
  const next = on ? [...new Set([...cur, variant])] : cur.filter(c => c !== variant);
  const rest = sel.filter(c => c !== cat.code && !all.includes(c));
  return next.length === all.length ? [...rest, cat.code] : [...rest, ...next];
}

/** 从 platform.extra JSON 解析 qianfan AppBuilder key（独立搜索转发，票 20）。缺失 / 非法 → ""。 */
export function parseAppbuilderKey(extra: string): string {
  if (!extra.trim()) return "";
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "appbuilder_key" in parsed) {
      const v = (parsed as { appbuilder_key: unknown }).appbuilder_key;
      if (typeof v === "string") return v;
    }
  } catch { /* ignore */ }
  return "";
}

/** 把 qianfan AppBuilder key 写回 extra JSON（保留其余键）。空白 → 移除键（不留空串）。 */
export function serializeAppbuilderKey(extra: string, key: string): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* ignore */ }
  }
  const clean = key.trim();
  if (clean.length === 0) {
    delete obj.appbuilder_key;
  } else {
    obj.appbuilder_key = clean;
  }
  return JSON.stringify(obj);
}


/** 从 platform.extra JSON 解析自定义槽位值（extra.custom_slots[槽名] = 模型链）。非数组值丢弃，链去空去重。 */
export function parseCustomSlots(extra: string): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  try {
    const cs = (JSON.parse(extra) as { custom_slots?: unknown } | null)?.custom_slots;
    if (cs && typeof cs === "object" && !Array.isArray(cs)) {
      for (const [k, v] of Object.entries(cs)) {
        if (!Array.isArray(v)) continue;
        const chain = [...new Set(v.filter((x): x is string => typeof x === "string").map((x) => x.trim()).filter(Boolean))];
        if (chain.length > 0) out[k] = chain;
      }
    }
  } catch { /* ignore */ }
  return out;
}

/** 把自定义槽位值写回 extra JSON（保留其余键）。只保留 defNames 内且链非空的键（孤儿键 / 空链丢弃）；无剩余 → 删 custom_slots 键。 */
export function serializeCustomSlots(extra: string, values: Record<string, string[]>, defNames: string[]): string {
  let obj: Record<string, unknown> = {};
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) obj = parsed as Record<string, unknown>;
  } catch { /* ignore */ }
  const kept: Record<string, string[]> = {};
  for (const n of defNames) if (values[n]?.length) kept[n] = values[n];
  if (Object.keys(kept).length > 0) obj.custom_slots = kept;
  else delete obj.custom_slots;
  return JSON.stringify(obj);
}

/** 从 platform.extra JSON 解析关闭的槽位名（extra.disabled_slots ∪ 旧键 soft_disabled_slots）。非数组 → 空；去空去重。 */
export function parseDisabledSlots(extra: string): string[] {
  try {
    const o = JSON.parse(extra) as Record<string, unknown> | null;
    const out: string[] = [];
    for (const key of ["disabled_slots", "soft_disabled_slots"]) {
      const v = o?.[key];
      if (Array.isArray(v)) {
        for (const x of v) {
          if (typeof x !== "string") continue;
          const n = x.trim();
          if (n && !out.includes(n)) out.push(n);
        }
      }
    }
    return out;
  } catch { /* ignore */ }
  return [];
}

/** 把关闭的槽位名写回 extra JSON（保留其余键）；只写 disabled_slots，旧键 soft_disabled_slots 一并删除；空数组删键。 */
export function serializeDisabledSlots(extra: string, slots: string[]): string {
  let obj: Record<string, unknown> = {};
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) obj = parsed as Record<string, unknown>;
  } catch { /* ignore */ }
  delete obj.soft_disabled_slots;
  if (slots.length > 0) obj.disabled_slots = slots;
  else delete obj.disabled_slots;
  return JSON.stringify(obj);
}

/** 从 platform.extra JSON 解析 mitm_stats 开关（cc-sub-mitm 票 12：Claude Code 订阅平台
 *  的 MITM 接入形态标记）。缺失 / 非法 / 非布尔 → false。 */
export function parseMitmStats(extra: string): boolean {
  if (!extra.trim()) return false;
  try {
    const parsed: unknown = JSON.parse(extra);
    if (parsed && typeof parsed === "object" && "mitm_stats" in parsed) {
      return (parsed as { mitm_stats: unknown }).mitm_stats === true;
    }
  } catch { /* ignore */ }
  return false;
}

/** 把 mitm_stats 写回 extra JSON（保留其余键）。false → 移除键（默认行为）。 */
export function serializeMitmStats(extra: string, enabled: boolean): string {
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
    obj.mitm_stats = true;
  } else {
    delete obj.mitm_stats;
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
    /** 配额方式（quota-ia spec）：'manual' = 手动预算；'auto'/省略 = 自动脚本（后端按协议能力推断）。
     *  显式提交时后端按 source 变化互斥清对侧。 */
    quota_source?: "auto" | "manual";
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
    /** 配额方式（quota-ia spec）：省略=不动；显式提交按 source 变化互斥清对侧。 */
    quota_source?: "auto" | "manual";
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
  /** 平台错误状态清单（plat-select-status）：调度态优先（429/402/401/503），回落 24h 日志最近失败码。 */
  errorStatus: () =>
    invoke<PlatformErrorStatus[]>("platform_error_status"),

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
  magpieNames: (baseUrl: string, apiKey: string) =>
    invoke<string[]>("platform_magpie_quota_names", { baseUrl, apiKey }),
  query: (baseUrl: string, apiKey: string, platformId?: number, draftRequires?: Record<string, string>) =>
    invoke<PlatformQuota>("platform_query_quota", { baseUrl, apiKey, platformId: platformId ?? null, draftRequires: draftRequires ?? null }),
  queryNewapi: (baseUrl: string, apiKey: string, extra: string, platformId?: number) =>
    invoke<PlatformQuota>("platform_query_quota_newapi", { baseUrl, apiKey, extra, platformId: platformId ?? null }),
  queryDevin: (baseUrl: string, apiKey: string, extra: string, platformId?: number) =>
    invoke<PlatformQuota>("platform_query_quota_devin", { baseUrl, apiKey, extra, platformId: platformId ?? null }),
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

/** 两级客户端词表（大类 → 变体，Rust `client_vocab.rs` 单一真值源）。显示名走 `nameKey`（`flow.client.<code>`）。 */
export function getClientVocab(): Promise<ClientVocabCategory[]> {
  return invoke<ClientVocabCategory[]>("get_client_vocab");
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


// ─── Rate Limits（rate-limit-aware 票 02/04：限频配置，存 platform.extra）─────────

/** 模型级限频覆盖（extra.rate_limits.models 的条目）。 */
export interface RateLimitModelOverride {
  concurrency?: number;
  rpm?: number;
  tpm?: number;
}

/** 限频规则（extra.rate_limits.rules[] / registry rate_limit_policies[].rules[] 同形）：
 *  至少一个条件（models 非空，或带时段字段）；没填的上限 = 不限。 */
export interface RateLimitRule {
  /** 模型名精确值或 `前缀*` 通配；每个模型各自计数。 */
  models?: string[];
  start_hour?: number;
  end_hour?: number;
  days_of_week?: number[];
  day_type?: "workday" | "holiday";
  holiday_calendar?: string;
  timezone?: string;
  concurrency?: number;
  rpm?: number;
  tpm?: number;
}

/** 规则保存前校验：无条件（models 空且无时段字段）→ noCondition；通配符只允许末尾单个 `*` 且前缀非空 → badWildcard。 */
export function rateLimitRuleError(r: RateLimitRule): "noCondition" | "badWildcard" | null {
  const models = r.models ?? [];
  const hasTime = r.start_hour !== undefined || r.end_hour !== undefined || (r.days_of_week?.length ?? 0) > 0 || r.day_type !== undefined;
  if (models.length === 0 && !hasTime) return "noCondition";
  if (models.some(m => m.includes("*") && (!m.endsWith("*") || m.length === 1 || m.indexOf("*") !== m.length - 1))) return "badWildcard";
  return null;
}

/** 平台级限频（extra.rate_limits）：顶层三项 = 平台级上限，rules = 带条件的规则。 */
export interface RateLimitsConfig {
  concurrency?: number;
  rpm?: number;
  tpm?: number;
  rules?: RateLimitRule[];
  /** 导入自 registry 内置策略时记录的来源策略 id（网关只透传不读）。 */
  source_policy_id?: string;
  /** 导入时该平台 platform.json 的 last_updated（Unix 秒）。 */
  source_last_updated?: number;
}

/** 额度窗口（extra.quota_windows 条目）：window 秒数 + 预算三选一（token / 金额 $ / 请求次数）。
 *  budget_points 已从编辑 UI 移除（2026-10-23 用户拍板「token/金额，不要点数」）：
 *  字段仅存量兼容（读取归一层照常透传），在限频表单里编辑并保存后即被清除。 */
export interface QuotaWindowConfig {
  window: number;
  budget_tokens?: number;
  /** 金额预算（$，允许小数）。 */
  budget_usd?: number;
  /** 请求次数预算。 */
  budget_count?: number;
  /** @deprecated 存量兼容，UI 不再暴露；编辑保存后清除。 */
  budget_points?: number;
}

/** 限频配置整体（表单编辑对象）。 */
export interface RateLimitsBundle {
  rate_limits?: RateLimitsConfig;
  quota_windows?: QuotaWindowConfig[];
}

const isRateLimitsEmpty = (r: RateLimitsConfig | undefined) =>
  !r ||
  ((r.rpm ?? 0) === 0 &&
    (r.tpm ?? 0) === 0 &&
    (r.concurrency ?? 0) === 0 &&
    (!r.rules || r.rules.length === 0));

/** 从 platform.extra 解析限频配置（缺失/非法 → 空 bundle）。与 Rust 侧调度消费同一 JSON 形状（spec 票 02）。 */
export function parseRateLimitsBundle(extra: string): RateLimitsBundle {
  if (!extra.trim()) return {};
  try {
    const parsed: unknown = JSON.parse(extra);
    if (!parsed || typeof parsed !== "object") return {};
    const o = parsed as Record<string, unknown>;
    const out: RateLimitsBundle = {};
    if (o.rate_limits && typeof o.rate_limits === "object") {
      out.rate_limits = o.rate_limits as RateLimitsConfig;
    }
    if (Array.isArray(o.quota_windows)) {
      out.quota_windows = o.quota_windows as QuotaWindowConfig[];
    }
    return out;
  } catch {
    return {};
  }
}

/** 把限频配置写回 extra JSON（保留其余键）。空值（无 rpm/tpm/concurrency/rules）→ 删键（无覆盖）。 */
export function serializeRateLimitsBundle(extra: string, bundle: RateLimitsBundle): string {
  let obj: Record<string, unknown> = {};
  if (extra.trim()) {
    try {
      const parsed: unknown = JSON.parse(extra);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch { /* ignore */ }
  }
  if (isRateLimitsEmpty(bundle.rate_limits)) {
    delete obj.rate_limits;
  } else {
    obj.rate_limits = bundle.rate_limits;
  }
  if (!bundle.quota_windows || bundle.quota_windows.length === 0) {
    delete obj.quota_windows;
  } else {
    obj.quota_windows = bundle.quota_windows;
  }
  return JSON.stringify(obj);
}

/** 限频配置摘要（表单行 / 卡片徽标共用）：如 "60 RPM · 2.0M TPM · 2 窗口"。全空 → 空串。 */
export function rateLimitsSummary(b: RateLimitsBundle, windowsLabel: (n: number) => string): string {
  const parts: string[] = [];
  const r = b.rate_limits;
  if (r?.concurrency) parts.push(`${r.concurrency} CONC`);
  if (r?.rpm) parts.push(`${r.rpm} RPM`);
  if (r?.tpm) parts.push(`${r.tpm >= 1_000_000 ? `${(r.tpm / 1_000_000).toFixed(1)}M` : formatTpm(r.tpm)} TPM`);
  const w = b.quota_windows?.length ?? 0;
  if (w > 0) parts.push(windowsLabel(w));
  return parts.join(" · ");
}

function formatTpm(v: number): string {
  return v >= 1_000 ? `${(v / 1_000).toFixed(0)}K` : String(v);
}
