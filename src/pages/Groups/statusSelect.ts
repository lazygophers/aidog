// ── 多选「按状态选」纯函数（plat-select-status）：选项构建与平台筛选 ──
// 组件外置以便单测；数据源 = platformApi.errorStatus()（调度态优先 + 日志回落，后端拍板口径）。
import type { PlatformErrorStatus, PlatformStatus } from "../../services/api";

/** 组内平台 → 可选错误码选项（按码升序，含组内命中数）。组外平台的错误状态不计入。 */
export function buildStatusOptions(
  items: PlatformErrorStatus[],
  groupPlatformIds: number[],
): { code: number; count: number }[] {
  const byId = new Map(items.map((i) => [i.platform_id, i.code] as const));
  const counts = new Map<number, number>();
  for (const id of groupPlatformIds) {
    const code = byId.get(id);
    if (code !== undefined) counts.set(code, (counts.get(code) ?? 0) + 1);
  }
  return [...counts.entries()]
    .map(([code, count]) => ({ code, count }))
    .sort((a, b) => a.code - b.code);
}

/** 组内命中某错误码的平台 id 集（选中即 setSelectedIds）。 */
export function idsForCode(
  items: PlatformErrorStatus[],
  groupPlatformIds: number[],
  code: number,
): Set<number> {
  const byId = new Map(items.map((i) => [i.platform_id, i.code] as const));
  return new Set(groupPlatformIds.filter((id) => byId.get(id) === code));
}

/** 平台启用状态固定选项（值前缀 `s:` 与 HTTP 错误码选项区分）。 */
export const PLATFORM_STATUS_OPTIONS: { value: string; status: PlatformStatus; labelKey: string; fallback: string }[] = [
  { value: "s:enabled", status: "enabled", labelKey: "platform.statusEnabled", fallback: "启用" },
  { value: "s:disabled", status: "disabled", labelKey: "platform.statusDisabled", fallback: "禁用" },
  { value: "s:auto_disabled", status: "auto_disabled", labelKey: "platform.statusAutoDisabled", fallback: "熔断" },
];

/** 下拉选中值 → 要选中的平台 id 集：`s:<status>` 按平台 status，纯数字按错误码。 */
export function idsForSelection(
  value: string,
  items: PlatformErrorStatus[],
  plats: { id: number; status: PlatformStatus }[],
): Set<number> {
  if (value.startsWith("s:")) {
    return new Set(plats.filter((p) => p.status === value.slice(2)).map((p) => p.id));
  }
  return idsForCode(items, plats.map((p) => p.id), Number(value));
}
