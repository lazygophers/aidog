// ── 多选「按状态选」纯函数（plat-select-status）：选项构建与平台筛选 ──
// 组件外置以便单测；数据源 = platformApi.errorStatus()（调度态优先 + 日志回落，后端拍板口径）。
// 错误码不再逐码列出，按语义分类（密钥失效 / 余额不足 / 限频 / 服务异常）。
import type { PlatformErrorStatus, PlatformStatus } from "../../services/api";

export type ErrorCategory =
  | "key_invalid" | "token_expired" | "balance" | "rate_limit" | "quota" | "server"
  | "timeout" | "connect" | "expired" | "censorship";

/** 参与分类的平台字段（Platform 的子集）。 */
export interface CategorizablePlatform {
  id: number;
  status: PlatformStatus;
  last_error?: string;
  /** 毫秒 unix 时间戳，0 = 永不过期 */
  expires_at?: number;
}

/** 错误码语义分类：401/403 密钥失效、402 余额不足、429（529 视同 429）限频、451 审核拦截、5xx 服务异常；其余 null。 */
export function errorCategory(code: number): ErrorCategory | null {
  if (code === 529) code = 429;
  if (code === 401 || code === 403) return "key_invalid";
  if (code === 402) return "balance";
  if (code === 429) return "rate_limit";
  if (code === 451) return "censorship";
  if (code >= 500 && code <= 599) return "server";
  return null;
}

const RE_TOKEN_EXPIRED = /已过期|expired/i;
const RE_QUOTA = /quota|用量上限|额度/i;
const RE_TIMEOUT = /timed out|timeout/i;
const RE_CONNECT = /connect|connection closed|dns/i;

/**
 * 平台命中的全部错误分类：状态码类（密钥/令牌/余额/限频/额度/审核/服务）至多一个，
 * 超时 / 连接失败 / 平台到期 / 审核拦截（last_error 标记）是独立信号，可与之并存。
 */
export function platformCategories(
  p: CategorizablePlatform,
  item: PlatformErrorStatus | undefined,
  nowMs: number,
): Set<ErrorCategory> {
  const out = new Set<ErrorCategory>();
  const err = p.last_error ?? "";
  const code = item?.code === 529 ? 429 : item?.code ?? 0;
  const errCode = Number(/^HTTP (\d{3})/.exec(err)?.[1] ?? 0);
  let cat = errorCategory(code);
  if ((code === 401 || errCode === 401) && RE_TOKEN_EXPIRED.test(err)) cat = "token_expired";
  else if (cat === "rate_limit" && RE_QUOTA.test(err)) cat = "quota";
  if (cat) out.add(cat);
  if (err.includes("censorship_blocked") || errCode === 451) out.add("censorship");
  const timeout = RE_TIMEOUT.test(err);
  if (timeout) out.add("timeout");
  else if (err.startsWith("upstream error:") && RE_CONNECT.test(err)) out.add("connect");
  if ((p.expires_at ?? 0) > 0 && (p.expires_at as number) <= nowMs) out.add("expired");
  return out;
}

/** 错误分类固定选项（值前缀 `c:`），顺序即下拉顺序。 */
export const ERROR_CATEGORY_OPTIONS: { value: string; category: ErrorCategory; labelKey: string; fallback: string }[] = [
  { value: "c:key_invalid", category: "key_invalid", labelKey: "group.errCat.keyInvalid", fallback: "密钥失效" },
  { value: "c:token_expired", category: "token_expired", labelKey: "group.errCat.tokenExpired", fallback: "令牌已过期" },
  { value: "c:balance", category: "balance", labelKey: "group.errCat.balance", fallback: "余额不足" },
  { value: "c:rate_limit", category: "rate_limit", labelKey: "group.errCat.rateLimit", fallback: "限频" },
  { value: "c:quota", category: "quota", labelKey: "group.errCat.quota", fallback: "套餐/额度用尽" },
  { value: "c:server", category: "server", labelKey: "group.errCat.server", fallback: "服务异常" },
  { value: "c:timeout", category: "timeout", labelKey: "group.errCat.timeout", fallback: "请求超时" },
  { value: "c:connect", category: "connect", labelKey: "group.errCat.connect", fallback: "连接失败" },
  { value: "c:expired", category: "expired", labelKey: "group.errCat.expired", fallback: "平台已到期" },
  { value: "c:censorship", category: "censorship", labelKey: "group.errCat.censorship", fallback: "内容审核拦截" },
];

/** 组内平台 → 命中数 >0 的错误分类选项（含组内命中数）。组外平台的错误状态不计入。 */
export function buildStatusOptions(
  items: PlatformErrorStatus[],
  plats: CategorizablePlatform[],
  nowMs: number = Date.now(),
): (typeof ERROR_CATEGORY_OPTIONS[number] & { count: number })[] {
  const byId = new Map(items.map((i) => [i.platform_id, i]));
  const counts = new Map<ErrorCategory, number>();
  for (const p of plats) {
    for (const c of platformCategories(p, byId.get(p.id), nowMs)) counts.set(c, (counts.get(c) ?? 0) + 1);
  }
  return ERROR_CATEGORY_OPTIONS
    .map((o) => ({ ...o, count: counts.get(o.category) ?? 0 }))
    .filter((o) => o.count > 0);
}

/** 平台启用状态固定选项（值前缀 `s:` 与 HTTP 错误码选项区分）。 */
export const PLATFORM_STATUS_OPTIONS: { value: string; status: PlatformStatus; labelKey: string; fallback: string }[] = [
  { value: "s:enabled", status: "enabled", labelKey: "platform.statusEnabled", fallback: "启用" },
  { value: "s:disabled", status: "disabled", labelKey: "platform.statusDisabled", fallback: "禁用" },
  { value: "s:auto_disabled", status: "auto_disabled", labelKey: "platform.statusAutoDisabled", fallback: "熔断" },
];

/** 下拉选中值 → 要选中的平台 id 集：`s:<status>` 按平台 status，`c:<分类>` 按错误分类。 */
export function idsForSelection(
  value: string,
  items: PlatformErrorStatus[],
  plats: CategorizablePlatform[],
  nowMs: number = Date.now(),
): Set<number> {
  if (value.startsWith("s:")) {
    return new Set(plats.filter((p) => p.status === value.slice(2)).map((p) => p.id));
  }
  const cat = value.slice(2) as ErrorCategory;
  const byId = new Map(items.map((i) => [i.platform_id, i]));
  return new Set(plats.filter((p) => platformCategories(p, byId.get(p.id), nowMs).has(cat)).map((p) => p.id));
}
