// platformDraft — 平台配置草稿（issue #58 票 06，形状 B）：表单纯数据字段 + 来源转换 +
//   payload 序列化 + reducer。纯 TS，零 React、零 invoke；与 slotValue.ts 同层。
// 单一真值：三入口水合（edit / duplicate / fullShare）与序列化链从 usePlatformForm.ts /
//   platformPasteApply.ts 逐行平移（行为锁定见票 05 契约测试 + 本文件纯函数测试）。
// 本票只交付 module 本体，生产调用方在票 12-14 接入（hook / paste / 控件迁移）。
import type {
  ManualBudget, MockConfig, DevinConfig, PassthroughAuth, Platform, Protocol,
  PlatformEndpoint, RateLimitsBundle, SharePlatform, ModelSlot, TimeModelRule, ClientUaRule,
} from "../../services/api";
import {
  DEFAULT_MOCK_CONFIG, DEFAULT_DEVIN_CONFIG, DEFAULT_PASSTHROUGH_AUTH,
  parseMockConfig, serializeMockConfig,
  parseDevinConfig, serializeDevinConfig,
  parseQuotaScriptConfig, serializeQuotaScriptConfig,
  parsePlatformBreaker, serializePlatformBreaker,
  parseMitmStats, serializeMitmStats,
  parseRateLimitsBundle, serializeRateLimitsBundle,
  parsePassthroughAuth, serializePassthroughAuth,
  parseAppbuilderKey, serializeAppbuilderKey,
  parseCustomSlots, serializeCustomSlots,
  parseDisabledSlots, serializeDisabledSlots,
  parseProxyEnabled, serializeProxyEnabled,
  parsePlatformPeak, serializePlatformPeak,
  parseDisableDuringPeak, serializeDisableDuringPeak,
  parseDirectModels, serializeDirectModels,
  parseAllowedClients, serializeAllowedClients,
  parseClientUaRules, serializeClientUaRules,
  parsePlatformTimeWindows, serializePlatformTimeWindows,
  parseSubscriptionStartDate, validSubscriptionStartDate,
  validateClientUaRule, rateLimitRuleError,
} from "../../services/api";
import type { TimeWindow, QuotaScriptVariant } from "./defaults";
import type { CustomSlotDef } from "../../services/api/customSlots";
import { MODEL_SLOTS } from "./constants";
import { slotsToForm, slotList } from "./slotValue";
import { getPrimaryBaseUrl } from "../../pages/platforms/usePlatformQuota";

/** 表单里的纯数据字段（基础字段 + 17 个 extra 键的草稿形状 + 6 个时段键）。
 *  UI 瞬时态（busy、弹窗开关、windowsTz、quotaSourcePristine、expiryEnabled 等
 *  「只影响展示、不写 extra」态）不进 draft，留在 hook（spec 5.1）。 */
export interface PlatformDraft {
  /** 源平台 id（edit 绑定 / duplicate / share / 新建 = null）。仅标识绑定，不改字段水合。 */
  sourceId: number | null;
  name: string;
  protocol: Protocol;
  codingPlan: boolean;
  apiKey: string;
  models: Record<ModelSlot, string[]>;
  availableModels: string[];
  endpoints: PlatformEndpoint[];
  /** 用户原始 extra 串（草稿子字段各自 parse；序列化以本串为 base，未知键原样保留）。 */
  extra: string;
  mockConfig: MockConfig;
  quotaVariantId: string;
  quotaCustomScript: string;
  quotaRequires: Record<string, string>;
  quotaSource: "auto" | "manual";
  manualBudgets: ManualBudget[];
  expiresAt: number;
  breakerFailureThreshold: string;
  breakerHalfOpenMax: string;
  /** 时段档 6 键（usePlatformTimeModels 的数据面；windowsTz 是展示态，不进 draft）。 */
  peak: TimeWindow[];
  disableDuringPeak: boolean;
  timeModels: TimeModelRule[];
  directModels: string[];
  allowedClients: string[];
  uaRules: ClientUaRule[];
  rateLimits: RateLimitsBundle;
  mitmStats: boolean;
  devinConfig: DevinConfig;
  passAuth: PassthroughAuth;
  appbuilderKey: string;
  customSlotValues: Record<string, string[]>;
  disabledSlots: string[];
  proxyEnabled: boolean;
  autoGroup: boolean;
  joinGroupIds: number[];
  lockedGroupId: number | null;
}

export type DraftAction =
  | { type: "patch"; patch: Partial<PlatformDraft> }
  | { type: "load"; draft: PlatformDraft }
  | { type: "reset" };

/** draftToPayload 的环境依赖：异步派生、非草稿持有（spec 5.1 interface）。 */
export interface DraftPayloadEnv {
  quotaVariants: QuotaScriptVariant[];
  /** null = 全局槽位定义未加载（保存不清孤儿键，与现行 hook 行为一致）。 */
  customSlotDefs?: CustomSlotDef[] | null;
}

/** SharedCreateFields = 除 name/api_key 外的全部创建字段（单平台保存与批量创建共用）。 */
export interface SharedCreateFields {
  platform_type: Protocol;
  quota_source: "auto" | "manual";
  base_url: string;
  extra?: string;
  models?: Platform["models"];
  available_models?: string[];
  endpoints?: PlatformEndpoint[];
  manual_budgets?: ManualBudget[];
  auto_group: boolean;
  join_group_ids: number[];
  expires_at: number;
}

export function emptyDraft(): PlatformDraft {
  return {
    sourceId: null,
    name: "",
    protocol: "openai",
    codingPlan: false,
    apiKey: "",
    models: slotsToForm(undefined),
    availableModels: [],
    endpoints: [],
    extra: "",
    mockConfig: { ...DEFAULT_MOCK_CONFIG },
    quotaVariantId: "",
    quotaCustomScript: "",
    quotaRequires: {},
    quotaSource: "auto",
    manualBudgets: [],
    expiresAt: 0,
    breakerFailureThreshold: "",
    breakerHalfOpenMax: "",
    peak: [],
    disableDuringPeak: false,
    timeModels: [],
    directModels: [],
    allowedClients: [],
    uaRules: [],
    rateLimits: {},
    mitmStats: false,
    devinConfig: { ...DEFAULT_DEVIN_CONFIG },
    passAuth: { ...DEFAULT_PASSTHROUGH_AUTH },
    appbuilderKey: "",
    customSlotValues: {},
    disabledSlots: [],
    proxyEnabled: false,
    autoGroup: true,
    joinGroupIds: [],
    lockedGroupId: null,
  };
}

/** 纯 reducer：patch 只覆盖给出的键；load 整体替换；reset = load(emptyDraft())。 */
export function draftReducer(s: PlatformDraft, a: DraftAction): PlatformDraft {
  switch (a.type) {
    case "patch":
      return { ...s, ...a.patch };
    case "load":
      return a.draft;
    case "reset":
      return emptyDraft();
  }
}

/** 透传平台保存侧允许 endpoints 空、只落顶层 base_url；编辑/复制/导入时回填进 endpoints[0]，
 *  否则表单 Base URL 输入框（绑定 endpoints[0]?.base_url）显示为空、保存丢 base_url。
 *  拷贝一份再改，不动调用方数组（原 usePlatformForm.ts 三入口同款，票 03/04）。 */
function backfillPassthroughEndpoints(
  platformType: Protocol, baseUrl: string | undefined, eps: PlatformEndpoint[],
): PlatformEndpoint[] {
  const out = [...eps];
  if (platformType === "passthrough" && baseUrl) {
    if (out.length === 0) {
      out.push({ protocol: "passthrough", base_url: baseUrl, client_type: "default", coding_plan: false });
    } else if (!out[0].base_url) {
      out[0] = { ...out[0], base_url: baseUrl };
    }
  }
  return out;
}

/** extra 串 → 草稿中「由 extra 派生」的字段（非法 JSON 各 parse 回空值，原串在 draft.extra）。 */
function hydrateExtra(ex: string): Pick<PlatformDraft,
  "mockConfig" | "quotaVariantId" | "quotaCustomScript" | "quotaRequires"
  | "breakerFailureThreshold" | "breakerHalfOpenMax"
  | "peak" | "disableDuringPeak" | "timeModels" | "directModels" | "allowedClients" | "uaRules"
  | "rateLimits" | "mitmStats" | "devinConfig" | "passAuth" | "appbuilderKey"
  | "customSlotValues" | "disabledSlots" | "proxyEnabled"
> {
  const qs = parseQuotaScriptConfig(ex);
  const brk = parsePlatformBreaker(ex);
  return {
    mockConfig: parseMockConfig(ex),
    quotaVariantId: qs.variantId,
    quotaCustomScript: qs.customScript,
    quotaRequires: {},
    breakerFailureThreshold: brk.failure_threshold > 0 ? String(brk.failure_threshold) : "",
    breakerHalfOpenMax: brk.half_open_max > 0 ? String(brk.half_open_max) : "",
    peak: parsePlatformPeak(ex),
    disableDuringPeak: parseDisableDuringPeak(ex),
    timeModels: parsePlatformTimeWindows(ex),
    directModels: parseDirectModels(ex),
    allowedClients: parseAllowedClients(ex),
    uaRules: parseClientUaRules(ex),
    rateLimits: parseRateLimitsBundle(ex),
    mitmStats: parseMitmStats(ex),
    devinConfig: parseDevinConfig(ex),
    passAuth: parsePassthroughAuth(ex),
    appbuilderKey: parseAppbuilderKey(ex),
    customSlotValues: parseCustomSlots(ex),
    disabledSlots: parseDisabledSlots(ex),
    proxyEnabled: parseProxyEnabled(ex),
  };
}

/** 存量平台 → 草稿。mode 只决定 sourceId 绑定，字段水合 edit 与 duplicate 完全一致
 *  （spec 5.1 不变量④）。joinGroupIds 由 hook 反查 groupDetails 后异步填充，此处恒 []。 */
export function draftFromPlatform(p: Platform, mode: "edit" | "duplicate"): PlatformDraft {
  const ex = p.extra ?? "";
  return {
    sourceId: mode === "edit" ? p.id : null,
    name: p.name,
    protocol: p.platform_type,
    codingPlan: (p.endpoints || []).some(ep => ep.coding_plan),
    apiKey: p.api_key,
    models: slotsToForm(p.models),
    availableModels: p.available_models ?? [],
    endpoints: backfillPassthroughEndpoints(p.platform_type, p.base_url, p.endpoints ?? []),
    extra: ex,
    ...hydrateExtra(ex),
    quotaSource: p.quota_source === "manual" ? "manual" : "auto",
    manualBudgets: p.manual_budgets ?? [],
    expiresAt: p.expires_at ?? 0,
    autoGroup: true,
    joinGroupIds: [],
    lockedGroupId: null,
  };
}

/** 分享串 → 草稿。SharePlatform 不携带 quota_source / expires_at：quota_source 由
 *  manual_budgets 非空推导（Rust 保存侧 source=auto 会清 manual_budgets）；expires_at 恒 0。 */
export function draftFromShare(s: SharePlatform): PlatformDraft {
  const ex = s.extra ?? "";
  return {
    sourceId: null,
    name: s.name,
    protocol: s.platform_type,
    codingPlan: (s.endpoints || []).some(ep => ep.coding_plan),
    apiKey: s.api_key,
    models: slotsToForm(s.models),
    availableModels: s.available_models ?? [],
    endpoints: backfillPassthroughEndpoints(s.platform_type, s.base_url, s.endpoints ?? []),
    extra: ex,
    ...hydrateExtra(ex),
    quotaSource: (s.manual_budgets ?? []).length > 0 ? "manual" : "auto",
    manualBudgets: s.manual_budgets ?? [],
    expiresAt: 0,
    autoGroup: true,
    joinGroupIds: [],
    lockedGroupId: null,
  };
}

/** 草稿 → SharedCreateFields（除 name/api_key 外全部；含整个 extra 序列化）。
 *  序列化顺序固定（平移自 usePlatformForm.buildSharedCreateFields）：mock → devin →
 *  passthrough 认证 → appbuilder → custom_slots → disabled_slots → quota → breaker →
 *  时段 6 键 → mitm → proxy → rate_limits。devin 必须先于 quota（org_id 镜像写 extra.devin）。
 *  校验抛错与序列化同时机，留在此处（spec 5.1 错误模式）；unknown 协议拒绝保存属交互
 *  校验，留在 hook。 */
export function draftToPayload(d: PlatformDraft, env: DraftPayloadEnv): SharedCreateFields {
  if (d.protocol === "qianfan_coding" && !validSubscriptionStartDate(parseSubscriptionStartDate(d.extra))) {
    throw new Error("Enter a valid date no later than today in Beijing.");
  }
  const isPassthrough = d.protocol === "claude_code";
  const isPassThrough = d.protocol === "passthrough";
  let extraPayload = d.extra;
  // mock 平台：把配置写回 extra；其余协议 mock 键原样保留（verbatim）。
  if (d.protocol === "mock") extraPayload = serializeMockConfig(extraPayload, d.mockConfig);
  if (d.protocol === "devin") extraPayload = serializeDevinConfig(extraPayload, d.devinConfig);
  if (isPassThrough) extraPayload = serializePassthroughAuth(extraPayload, d.passAuth);
  if (d.protocol === "qianfan") extraPayload = serializeAppbuilderKey(extraPayload, d.appbuilderKey);
  // 自定义槽位（extra.custom_slots）：定义未加载时不动，避免误清存量值。
  if (env.customSlotDefs) {
    extraPayload = serializeCustomSlots(extraPayload, d.customSlotValues, env.customSlotDefs.map(x => x.name));
  }
  extraPayload = serializeDisabledSlots(extraPayload, d.disabledSlots);
  // quota 脚本变体选择 + requires 参数（custom↔id 互斥、requires 顶层+旧嵌套镜像）。
  extraPayload = serializeQuotaScriptConfig(
    extraPayload,
    { variantId: d.quotaVariantId, customScript: d.quotaCustomScript, requires: d.quotaRequires },
    env.quotaVariants.map(v => ({ id: v.id, requires: (v.requires ?? []).map(r => r.key) })),
    d.protocol,
  );
  // 熔断覆盖写入 extra.breaker：空 = 继承（0 → 移除键）；负值钳为 0。
  const toBreakerNum = (v: string) => Math.max(0, Math.floor(Number(v) || 0));
  extraPayload = serializePlatformBreaker(extraPayload, {
    failure_threshold: toBreakerNum(d.breakerFailureThreshold),
    half_open_max: toBreakerNum(d.breakerHalfOpenMax),
  });
  if (d.uaRules.some(r => validateClientUaRule(r))) {
    throw new Error("Invalid UA rule: pattern must not be empty and regex must be valid");
  }
  if (d.rateLimits.rate_limits?.rules?.some(r => rateLimitRuleError(r))) {
    throw new Error("Invalid rate-limit rule: each rule needs at least one model or a time window; wildcard must be a single trailing *");
  }
  extraPayload = serializePlatformPeak(extraPayload, d.peak);
  extraPayload = serializeDisableDuringPeak(extraPayload, d.disableDuringPeak);
  extraPayload = serializeDirectModels(extraPayload, d.directModels);
  extraPayload = serializeAllowedClients(extraPayload, d.allowedClients);
  extraPayload = serializeClientUaRules(extraPayload, d.uaRules);
  extraPayload = serializePlatformTimeWindows(extraPayload, d.timeModels);
  // mitm_stats：false → 移除键（默认行为）；true → 写入。
  extraPayload = serializeMitmStats(extraPayload, d.mitmStats);
  // 使用代理：显式写 bool（两态都落库）。
  extraPayload = serializeProxyEnabled(extraPayload, d.proxyEnabled);
  // 限频配置：空 bundle → 删键（无覆盖）。
  extraPayload = serializeRateLimitsBundle(extraPayload, d.rateLimits);

  const modelsPayload: Record<string, string[] | undefined> = {};
  for (const slot of MODEL_SLOTS) {
    const v = slotList(d.models[slot.key]);
    modelsPayload[slot.key] = v.length > 0 ? v : undefined;
  }
  // 全空也返回对象（JSON 序列化为 {}）：后端 update 对 models 缺省=保留旧值。
  const mb: ManualBudget[] = isPassthrough || isPassThrough ? [] : d.manualBudgets;
  return {
    platform_type: d.protocol,
    quota_source: d.quotaSource,
    base_url: getPrimaryBaseUrl(d.protocol, d.endpoints),
    extra: extraPayload ? extraPayload : undefined,
    models: modelsPayload as Platform["models"],
    available_models: d.availableModels.length > 0 ? d.availableModels : undefined,
    endpoints: d.endpoints.length > 0 ? d.endpoints : undefined,
    manual_budgets: mb.length > 0 ? mb : undefined,
    auto_group: d.autoGroup,
    join_group_ids: d.joinGroupIds,
    expires_at: d.expiresAt,
  };
}

// 限频草稿助手（bundleToDraft / draftToBundle）：迁移期由本 module re-export 单一实现
// （实现暂居 formSections.tsx，票 14 迁控件时随迁；rateLimitsDraft.test.ts import 不变）。
export { bundleToDraft, draftToBundle } from "../../pages/platforms/formSections";
export type { RateLimitsDraft, QuotaBudgetMode } from "../../pages/platforms/formSections";
