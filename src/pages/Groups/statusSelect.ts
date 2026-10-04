// ── 多选「按状态选」纯函数（plat-select-status）：选项构建与平台筛选 ──
// 组件外置以便单测；数据源 = platformApi.errorStatus()（调度态优先 + 日志回落，后端拍板口径）。
import type { PlatformErrorStatus } from "../../services/api";

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
