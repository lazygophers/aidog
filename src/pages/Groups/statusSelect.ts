// ── 多选「按状态选」纯函数（plat-select-status）：选项构建与平台筛选 ──
// 组件外置以便单测；数据源 = platformApi.errorStatus()（调度态优先 + 日志回落，后端拍板口径）。
// 错误码不再逐码列出，按语义分类（密钥失效 / 余额不足 / 限频 / 服务异常）。
import type { PlatformErrorStatus, PlatformStatus } from "../../services/api";

export type ErrorCategory = "key_invalid" | "balance" | "rate_limit" | "server";

/** 错误码语义分类：401/403 密钥失效、402 余额不足、429 限频、5xx 服务异常；其余 null。 */
export function errorCategory(code: number): ErrorCategory | null {
  if (code === 401 || code === 403) return "key_invalid";
  if (code === 402) return "balance";
  if (code === 429) return "rate_limit";
  if (code >= 500 && code <= 599) return "server";
  return null;
}

/** 错误分类固定选项（值前缀 `c:`），顺序即下拉顺序。 */
export const ERROR_CATEGORY_OPTIONS: { value: string; category: ErrorCategory; labelKey: string; fallback: string }[] = [
  { value: "c:key_invalid", category: "key_invalid", labelKey: "group.errCat.keyInvalid", fallback: "密钥失效" },
  { value: "c:balance", category: "balance", labelKey: "group.errCat.balance", fallback: "余额不足" },
  { value: "c:rate_limit", category: "rate_limit", labelKey: "group.errCat.rateLimit", fallback: "限频" },
  { value: "c:server", category: "server", labelKey: "group.errCat.server", fallback: "服务异常" },
];

/** 组内平台 → 命中数 >0 的错误分类选项（含组内命中数）。组外平台的错误状态不计入。 */
export function buildStatusOptions(
  items: PlatformErrorStatus[],
  groupPlatformIds: number[],
): (typeof ERROR_CATEGORY_OPTIONS[number] & { count: number })[] {
  const inGroup = new Set(groupPlatformIds);
  const counts = new Map<ErrorCategory, number>();
  for (const i of items) {
    const c = inGroup.has(i.platform_id) ? errorCategory(i.code) : null;
    if (c) counts.set(c, (counts.get(c) ?? 0) + 1);
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
  plats: { id: number; status: PlatformStatus }[],
): Set<number> {
  if (value.startsWith("s:")) {
    return new Set(plats.filter((p) => p.status === value.slice(2)).map((p) => p.id));
  }
  const ids = new Set(plats.map((p) => p.id));
  return new Set(items.filter((i) => ids.has(i.platform_id) && `c:${errorCategory(i.code)}` === value).map((i) => i.platform_id));
}
