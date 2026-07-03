// ─── 浮窗配置项工厂 + 规整（自 PopoverConfigTab.tsx 外迁，arch 阶段6 S5）──

import type {
  PopoverConfig,
  PopoverItem,
  PopoverItemType,
  RowMeta,
  Group,
  Platform,
} from "../../services/api";
import { GROUP_TYPES, defaultColor } from "./constants";

export function makeItem(
  type: PopoverItemType,
  order: number,
  row: number,
  platforms: Platform[],
  groups: Group[],
): PopoverItem {
  const base: PopoverItem = {
    id: `popover-${type}-${Date.now()}`,
    item_type: type,
    visible: true,
    order,
    row,
    size: "m",
    color: defaultColor(),
  };
  if (type === "cost_trend") {
    return { ...base, scope: "overall", scope_ref: null, time_window: "7d" };
  }
  if (type === "platform_metric") {
    return { ...base, scope: "platform", scope_ref: platforms[0] ? String(platforms[0].id) : null, time_window: "today" };
  }
  if (GROUP_TYPES.has(type)) {
    const ref = groups[0]?.group_key ?? null;
    if (type === "group_cost") return { ...base, scope: "group", scope_ref: ref, time_window: "7d" };
    if (type === "group_balance") return { ...base, scope: "group", scope_ref: ref };
    return { ...base, scope: "group", scope_ref: ref, time_window: "today" };
  }
  return base;
}

/** effectiveRow：row 缺省回退 order（与渲染层一致）。 */
export function effRow(item: PopoverItem): number {
  return item.row ?? item.order;
}

/** 把 items 规整为「按 row 升序分组、行号连续从 0、行内按 order 排」的结构，
 * 并重写 row/order 为规范值，rows[] 与之对齐。返回新 config（幂等）。 */
export function normalizeConfig(items: PopoverItem[], rows: RowMeta[] | undefined): PopoverConfig {
  const map = new Map<number, PopoverItem[]>();
  for (const it of items) {
    const r = effRow(it);
    const list = map.get(r);
    if (list) list.push(it);
    else map.set(r, [it]);
  }
  const rowNums = [...map.keys()].sort((a, b) => a - b);
  const nextItems: PopoverItem[] = [];
  const nextRows: RowMeta[] = [];
  rowNums.forEach((oldRow, newRow) => {
    const list = map.get(oldRow)!.sort((a, b) => a.order - b.order);
    list.forEach((it, idx) => {
      nextItems.push({ ...it, row: newRow, order: idx });
    });
    nextRows.push({ cols: rows?.[oldRow]?.cols ?? 1 });
  });
  return { items: nextItems, rows: nextRows };
}
