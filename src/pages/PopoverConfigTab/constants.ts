// ─── 浮窗配置页常量（自 PopoverConfigTab.tsx 外迁，arch 阶段6 S5）──
// 预定义指标集 / 多实例类型 / group 类型 / i18n 标签表 / 时间窗 / 尺寸 / 列数上限 / 颜色预设。

import type {
  PopoverItemType,
  PopoverItemSize,
  PopoverTrendWindow,
  TrayColor,
} from "../../services/api";

/** 预定义指标集（顺序即添加菜单顺序）。 */
export const ALL_ITEM_TYPES: PopoverItemType[] = [
  "proxy_status",
  "platform_balance",
  "today_cost",
  "today_cache_rate",
  "today_tokens",
  "platform_today",
  "platform_metric",
  "group_cost",
  "group_tokens",
  "group_requests",
  "group_balance",
  "cost_trend",
];

/** 可重复添加的多实例类型（各自独立配置）。 */
export const MULTI_INSTANCE_TYPES: ReadonlySet<PopoverItemType> = new Set<PopoverItemType>([
  "cost_trend", "platform_metric", "group_cost", "group_tokens", "group_requests", "group_balance",
]);

/** group_* 系列：scope 锁 "group"，配置 UI 显示分组下拉。 */
export const GROUP_TYPES: ReadonlySet<PopoverItemType> = new Set<PopoverItemType>([
  "group_cost", "group_tokens", "group_requests", "group_balance",
]);

/** 指标类型 → i18n key + 默认中文标签。 */
export const TYPE_LABELS: Record<PopoverItemType, { key: string; fallback: string }> = {
  proxy_status: { key: "popover.itemProxyStatus", fallback: "代理状态" },
  platform_balance: { key: "popover.itemPlatformBalance", fallback: "平台余额/配额" },
  today_cost: { key: "popover.todayCost", fallback: "今日金额" },
  today_cache_rate: { key: "popover.todayCacheRate", fallback: "今日缓存率" },
  today_tokens: { key: "popover.todayTokens", fallback: "今日 Token" },
  platform_today: { key: "popover.platformToday", fallback: "各平台今日" },
  platform_metric: { key: "popover.itemPlatformMetric", fallback: "指定平台指标" },
  group_cost: { key: "popover.itemGroupCost", fallback: "分组金额" },
  group_tokens: { key: "popover.itemGroupTokens", fallback: "分组今日Token" },
  group_requests: { key: "popover.itemGroupRequests", fallback: "分组今日请求" },
  group_balance: { key: "popover.itemGroupBalance", fallback: "分组余额" },
  cost_trend: { key: "popover.itemCostTrend", fallback: "消费趋势" },
};

export const TREND_WINDOWS: PopoverTrendWindow[] = ["today", "7d", "30d"];

export const SIZE_OPTIONS: PopoverItemSize[] = ["s", "m", "l"];
export const MAX_COLS = 3;

/** 颜色编辑预设（follow + 3 预设；custom 走 hex input）。 */
export const COLOR_PRESETS: { mode: "follow" | "preset"; value: string; css: string }[] = [
  { mode: "follow", value: "", css: "var(--text-primary)" },
  { mode: "preset", value: "red", css: "var(--color-danger, #ff3b30)" },
  { mode: "preset", value: "green", css: "#32d74b" },
  { mode: "preset", value: "orange", css: "var(--color-warning, #ff9500)" },
];

export function defaultColor(): TrayColor {
  return { mode: "follow", value: "" };
}

/** 6 位 hex 校验（容许带 #）。 */
export function isValidHex(s: string): boolean {
  return /^#?[0-9a-fA-F]{6}$/.test(s.trim());
}
