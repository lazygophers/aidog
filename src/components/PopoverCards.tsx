// ── 浮窗卡片渲染（共享）──
// 浮窗 (src/popover.tsx) 与配置页实时预览 (src/pages/PopoverConfigTab.tsx) 共用，
// 单一事实源，避免预览与实际渲染漂移。
// 导出 renderGrid（按 row 二维网格）+ PopoverData / PopoverEntry / PopoverTrayColor 类型。

import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  TodayStats,
  PopoverConfig,
  PopoverItem,
  TodayPlatformStat,
  StatsBucket,
  StatsOverview,
  StatsQuery,
  Group,
  GroupDetail,
} from "../services/api";
import { statsApi } from "../services/api";
import { formatNumber, formatCostUsd, formatPercent } from "../utils/formatters";
import { CostTrendChart } from "./shared/CostTrendChart";

// ─── Types ──────────────────────────────────────────────────

/** 浮窗内联颜色（兼容后端 TrayColor，mode 宽松为 string）。 */
export interface PopoverTrayColor {
  mode: string;
  value: string;
}

export interface PopoverEntry {
  name: string;
  value: string;
  color: PopoverTrayColor;
}

export interface PopoverData {
  config: PopoverConfig;
  entries: PopoverEntry[];
  today_stats: TodayStats;
  platform_today: TodayPlatformStat[];
  proxy_running: boolean;
  proxy_port: number;
}

type TFn = (k: string, d: string, opts?: Record<string, unknown>) => string;

// ─── Helpers ────────────────────────────────────────────────

export function resolveColor(color: PopoverTrayColor): string {
  if (color.mode === "preset") {
    const map: Record<string, string> = {
      red: "var(--status-error, #ff3b30)",
      green: "var(--status-success, var(--color-success))",
      orange: "var(--status-warning, #ff9500)",
    };
    return map[color.value] || "var(--text-primary)";
  }
  if (color.mode === "custom" && color.value) {
    const hex = color.value.trim().replace(/^#/, "");
    if (hex.length === 6) return `#${hex}`;
  }
  return "var(--text-primary)";
}

/** 尺寸归一：缺省 / 非法 → "m"。 */
type Size = "s" | "m" | "l";
function normSize(size: PopoverItem["size"]): Size {
  return size === "s" || size === "l" ? size : "m";
}

/** item.color → 数值上色样式；follow / 缺省 → 无内联色（继承主题）。 */
function valueColorStyle(item: PopoverItem): React.CSSProperties | undefined {
  const c = item.color;
  if (!c || c.mode === "follow") return undefined;
  const col = resolveColor(c as PopoverTrayColor);
  return col === "var(--text-primary)" ? undefined : { color: col };
}

// ─── Item renderers ─────────────────────────────────────────

function ProxyStatus({ data }: { data: PopoverData }) {
  return (
    <div className="popover-header">
      <span
        className="popover-status-dot"
        style={{ background: data.proxy_running ? "var(--status-success, var(--color-success))" : "var(--text-tertiary)" }}
      />
      <span className="popover-header-text">
        {data.proxy_running ? `Running :${data.proxy_port}` : "Stopped"}
      </span>
    </div>
  );
}

function PlatformBalance({ data, size }: { data: PopoverData; size: Size }) {
  if (data.entries.length === 0) return null;
  return (
    <div className={`popover-section pc-${size}`}>
      {data.entries.map((e, i) => (
        <div className="popover-entry" key={i}>
          <span className="popover-entry-dot" style={{ background: resolveColor(e.color) }} />
          {size !== "s" && <span className="popover-entry-name">{e.name}</span>}
          <span className="popover-entry-value" style={{ color: resolveColor(e.color) }}>
            {e.value}
          </span>
        </div>
      ))}
    </div>
  );
}

function MetricRow({
  label,
  value,
  sub,
  size,
  colorStyle,
}: {
  label: string;
  value: string;
  sub?: string;
  size: Size;
  colorStyle?: React.CSSProperties;
}) {
  // s: 仅大数值（无标签）；m: 标签+值；l: 标签+值+副标。
  if (size === "s") {
    return (
      <div className="popover-section">
        <div className="popover-metric-row pc-s">
          <span className="popover-metric-value" style={colorStyle}>{value}</span>
        </div>
      </div>
    );
  }
  return (
    <div className={`popover-section pc-${size}`}>
      <div className="popover-metric-row">
        <span className="popover-metric-label">{label}</span>
        <span className="popover-metric-value" style={colorStyle}>{value}</span>
      </div>
      {size === "l" && sub && <div className="popover-metric-sub">{sub}</div>}
    </div>
  );
}

function PlatformToday({ data, size, colorStyle }: { data: PopoverData; size: Size; colorStyle?: React.CSSProperties }) {
  const { t } = useTranslation();
  return (
    <div className={`popover-section pc-${size}`}>
      <div className="popover-stats-title">{t("popover.platformToday", "各平台今日")}</div>
      {data.platform_today.length === 0 ? (
        <div className="popover-empty">{t("popover.noUsageToday", "今日暂无用量")}</div>
      ) : (
        data.platform_today.map((p) => (
          <div className="popover-platform-row" key={p.platform_id}>
            <span className="popover-platform-name">
              {p.platform_name || t("popover.unknownPlatform", "未知平台")}
            </span>
            <span className="popover-platform-value" style={colorStyle}>{formatCostUsd(p.cost)}</span>
            {/* s: 仅平台名+金额；m: +token；l: +token+请求数 */}
            {size !== "s" && <span className="popover-platform-sub">{formatNumber(p.tokens)} tok</span>}
            {size === "l" && (
              <span className="popover-platform-sub">{formatNumber(p.requests)} {t("popover.reqUnit", "req")}</span>
            )}
          </div>
        ))
      )}
    </div>
  );
}

// ─── Cost trend card ────────────────────────────────────────

const DAY_MS = 86_400_000;

/** scope/time_window → StatsQuery（overall=不传 filter；today=hourly，7d/30d=daily）。 */
function buildTrendQuery(item: PopoverItem): StatsQuery {
  const now = Date.now();
  const window = item.time_window ?? "7d";
  let start: number;
  let granularity: StatsQuery["granularity"];
  if (window === "today") {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    start = d.getTime();
    granularity = "hourly";
  } else {
    const days = window === "30d" ? 30 : 7;
    start = now - days * DAY_MS;
    granularity = "daily";
  }
  const q: StatsQuery = { start, end: now, granularity };
  const scope = item.scope ?? "overall";
  if (scope === "group" && item.scope_ref) {
    q.filter_group = item.scope_ref;
  } else if (scope === "platform" && item.scope_ref) {
    q.filter_platform = item.scope_ref;
  }
  return q;
}

/** cost_trend 卡片标题（体现 scope）。 */
function trendTitle(item: PopoverItem, data: PopoverData, t: TFn): string {
  const scope = item.scope ?? "overall";
  if (scope === "platform" && item.scope_ref) {
    const pid = Number(item.scope_ref);
    const p = data.platform_today.find((x) => x.platform_id === pid);
    const name = p?.platform_name || item.scope_ref;
    return t("popover.trendPlatformTitle", "{{name}} 消费趋势", { name });
  }
  if (scope === "group") {
    return t("popover.trendGroupTitle", "分组消费趋势");
  }
  return t("popover.trendOverallTitle", "整体消费趋势");
}

function CostTrendCard({
  item,
  data,
  size,
  colorStyle,
  t,
}: {
  item: PopoverItem;
  data: PopoverData;
  size: Size;
  colorStyle?: React.CSSProperties;
  t: TFn;
}) {
  const [buckets, setBuckets] = useState<StatsBucket[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    statsApi
      .query(buildTrendQuery(item))
      .then((r) => {
        if (!cancelled) setBuckets(r.buckets);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
    // item 配置变化时重查（浮窗生命周期内通常不变）。
  }, [item.scope, item.scope_ref, item.time_window]);

  const total = buckets ? buckets.reduce((s, b) => s + b.total_cost, 0) : 0;

  return (
    <div className={`popover-section pc-${size}`}>
      {/* s: 无标题，仅迷你曲线；m/l: 标题；l: 额外汇总行 */}
      {size !== "s" && <div className="popover-stats-title">{trendTitle(item, data, t)}</div>}
      {failed ? (
        <div className="popover-empty">{t("popover.trendLoadError", "加载失败")}</div>
      ) : buckets === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : buckets.length === 0 ? (
        <div className="popover-empty">{t("popover.noUsageToday", "今日暂无用量")}</div>
      ) : (
        <>
          <CostTrendChart buckets={buckets} />
          {size === "l" && (
            <div className="popover-metric-sub">
              {t("popover.trendTotal", "合计")} <span style={colorStyle}>{formatCostUsd(total)}</span>
            </div>
          )}
        </>
      )}
    </div>
  );
}

// ─── Platform metric card ───────────────────────────────────

/** platform_metric 卡片标题（取平台名，兜底 scope_ref）。 */
function platformMetricTitle(item: PopoverItem, data: PopoverData, t: TFn): string {
  const ref = item.scope_ref ?? "";
  const pid = Number(ref);
  const p = data.platform_today.find((x) => x.platform_id === pid);
  const name = p?.platform_name || ref || t("popover.unknownPlatform", "未知平台");
  return t("popover.platformMetricTitle", "{{name}} 用量", { name });
}

function PlatformMetricCard({
  item,
  data,
  size,
  colorStyle,
  t,
}: {
  item: PopoverItem;
  data: PopoverData;
  size: Size;
  colorStyle?: React.CSSProperties;
  t: TFn;
}) {
  const [overview, setOverview] = useState<StatsOverview | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    statsApi
      .query(buildTrendQuery(item))
      .then((r) => {
        if (!cancelled) setOverview(r.overview);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [item.scope, item.scope_ref, item.time_window]);

  // token 口径与 today_tokens 对齐（input + output，不含 cache）。
  const tokens = overview ? overview.total_input_tokens + overview.total_output_tokens : 0;

  return (
    <div className={`popover-section pc-${size}`}>
      {size !== "s" && <div className="popover-stats-title">{platformMetricTitle(item, data, t)}</div>}
      {failed ? (
        <div className="popover-empty">{t("popover.trendLoadError", "加载失败")}</div>
      ) : overview === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : (
        <>
          <div className="popover-platform-row">
            <span className="popover-platform-value" style={colorStyle}>{formatCostUsd(overview.total_cost)}</span>
            {/* s: 仅金额；m: +token；l: +token in/out 拆分 */}
            {size !== "s" && <span className="popover-platform-sub">{formatNumber(tokens)} tok</span>}
          </div>
          {size === "l" && (
            <div className="popover-metric-sub">
              {t("popover.tokenIn", "入")} {formatNumber(overview.total_input_tokens)} · {t("popover.tokenOut", "出")} {formatNumber(overview.total_output_tokens)}
            </div>
          )}
        </>
      )}
    </div>
  );
}

// ─── Group metric cards ─────────────────────────────────────

/** group_* 卡片分组名（按 group_key 查 groups，兜底 scope_ref）。 */
function groupName(item: PopoverItem, groups: Group[], t: TFn): string {
  const ref = item.scope_ref ?? "";
  const g = groups.find((x) => x.group_key === ref);
  return g?.name || ref || t("popover.trendScopeGroup", "分组");
}

/** group_cost：分组金额（带时间窗，overview.total_cost）。 */
function GroupCostCard({
  item,
  groups,
  size,
  colorStyle,
  t,
}: {
  item: PopoverItem;
  groups: Group[];
  size: Size;
  colorStyle?: React.CSSProperties;
  t: TFn;
}) {
  const [overview, setOverview] = useState<StatsOverview | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    statsApi
      .query(buildTrendQuery(item))
      .then((r) => { if (!cancelled) setOverview(r.overview); })
      .catch(() => { if (!cancelled) setFailed(true); });
    return () => { cancelled = true; };
  }, [item.scope, item.scope_ref, item.time_window]);

  return (
    <div className={`popover-section pc-${size}`}>
      {size !== "s" && <div className="popover-stats-title">{t("popover.groupCostTitle", "{{name}} 金额", { name: groupName(item, groups, t) })}</div>}
      {failed ? (
        <div className="popover-empty">{t("popover.trendLoadError", "加载失败")}</div>
      ) : overview === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : (
        <>
          <div className="popover-metric-row">
            <span className="popover-metric-value" style={colorStyle}>{formatCostUsd(overview.total_cost)}</span>
          </div>
          {size === "l" && (
            <div className="popover-metric-sub">
              {formatNumber(overview.total_requests)} {t("popover.reqUnit", "req")} · {formatNumber(overview.total_input_tokens + overview.total_output_tokens)} tok
            </div>
          )}
        </>
      )}
    </div>
  );
}

/** group_tokens：分组今日 Token（input+output，固定今日窗）。 */
function GroupTokensCard({
  item,
  groups,
  size,
  colorStyle,
  t,
}: {
  item: PopoverItem;
  groups: Group[];
  size: Size;
  colorStyle?: React.CSSProperties;
  t: TFn;
}) {
  const [overview, setOverview] = useState<StatsOverview | null>(null);
  const [failed, setFailed] = useState(false);

  // 时间窗强制今日（无视 item.time_window）。
  const query: PopoverItem = { ...item, time_window: "today" };
  useEffect(() => {
    let cancelled = false;
    statsApi
      .query(buildTrendQuery(query))
      .then((r) => { if (!cancelled) setOverview(r.overview); })
      .catch(() => { if (!cancelled) setFailed(true); });
    return () => { cancelled = true; };
  }, [item.scope, item.scope_ref]);

  const tokens = overview ? overview.total_input_tokens + overview.total_output_tokens : 0;

  return (
    <div className={`popover-section pc-${size}`}>
      {size !== "s" && <div className="popover-stats-title">{t("popover.groupTokensTitle", "{{name}} 今日 Token", { name: groupName(item, groups, t) })}</div>}
      {failed ? (
        <div className="popover-empty">{t("popover.trendLoadError", "加载失败")}</div>
      ) : overview === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : (
        <>
          <div className="popover-metric-row">
            <span className="popover-metric-value" style={colorStyle}>{formatNumber(tokens)} tok</span>
          </div>
          {size === "l" && (
            <div className="popover-metric-sub">
              {t("popover.tokenIn", "入")} {formatNumber(overview.total_input_tokens)} · {t("popover.tokenOut", "出")} {formatNumber(overview.total_output_tokens)}
            </div>
          )}
        </>
      )}
    </div>
  );
}

/** group_requests：分组今日请求数（固定今日窗，overview.total_requests）。 */
function GroupRequestsCard({
  item,
  groups,
  size,
  colorStyle,
  t,
}: {
  item: PopoverItem;
  groups: Group[];
  size: Size;
  colorStyle?: React.CSSProperties;
  t: TFn;
}) {
  const [overview, setOverview] = useState<StatsOverview | null>(null);
  const [failed, setFailed] = useState(false);

  const query: PopoverItem = { ...item, time_window: "today" };
  useEffect(() => {
    let cancelled = false;
    statsApi
      .query(buildTrendQuery(query))
      .then((r) => { if (!cancelled) setOverview(r.overview); })
      .catch(() => { if (!cancelled) setFailed(true); });
    return () => { cancelled = true; };
  }, [item.scope, item.scope_ref]);

  return (
    <div className={`popover-section pc-${size}`}>
      {size !== "s" && <div className="popover-stats-title">{t("popover.groupRequestsTitle", "{{name}} 今日请求", { name: groupName(item, groups, t) })}</div>}
      {failed ? (
        <div className="popover-empty">{t("popover.trendLoadError", "加载失败")}</div>
      ) : overview === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : (
        <>
          <div className="popover-metric-row">
            <span className="popover-metric-value" style={colorStyle}>{formatNumber(overview.total_requests)}</span>
          </div>
          {size === "l" && (
            <div className="popover-metric-sub">
              {t("popover.successRate", "成功率")} {formatPercent(overview.success_rate, 0)}
            </div>
          )}
        </>
      )}
    </div>
  );
}

/** group_balance：分组余额（组内平台 est_balance_remaining 求和，点时值）。 */
function GroupBalanceCard({
  item,
  groups,
  groupDetails,
  size,
  colorStyle,
  t,
}: {
  item: PopoverItem;
  groups: Group[];
  groupDetails: GroupDetail[] | null;
  size: Size;
  colorStyle?: React.CSSProperties;
  t: TFn;
}) {
  const ref = item.scope_ref ?? "";
  const detail = groupDetails?.find((d) => d.group.group_key === ref);
  const balance = detail
    ? detail.platforms.reduce((sum, p) => sum + (p.platform.est_balance_remaining || 0), 0)
    : 0;

  return (
    <div className={`popover-section pc-${size}`}>
      {size !== "s" && <div className="popover-stats-title">{t("popover.groupBalanceTitle", "{{name}} 余额", { name: groupName(item, groups, t) })}</div>}
      {groupDetails === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : detail === undefined ? (
        <div className="popover-empty">{t("popover.trendNoGroup", "无分组")}</div>
      ) : (
        <>
          <div className="popover-metric-row">
            <span className="popover-metric-value" style={colorStyle}>{formatCostUsd(balance)}</span>
          </div>
          {size === "l" && (
            <div className="popover-metric-sub">
              {detail.platforms.length} {t("popover.platformsUnit", "平台")}
            </div>
          )}
        </>
      )}
    </div>
  );
}

/** 单 item 渲染（按 type 分发，读 size/color）。 */
export function renderItem(
  item: PopoverItem,
  data: PopoverData,
  groups: Group[],
  groupDetails: GroupDetail[] | null,
  t: TFn,
): React.ReactNode {
  const size = normSize(item.size);
  const cs = valueColorStyle(item);
  switch (item.item_type) {
    case "proxy_status":
      return <ProxyStatus key={item.id} data={data} />;
    case "platform_balance":
      return <PlatformBalance key={item.id} data={data} size={size} />;
    case "today_cost":
      return <MetricRow key={item.id} label={t("popover.todayCost", "今日金额")} value={formatCostUsd(data.today_stats.cost)} sub={t("popover.todayCostSub", "今日累计金额")} size={size} colorStyle={cs} />;
    case "today_cache_rate":
      return <MetricRow key={item.id} label={t("popover.todayCacheRate", "今日缓存率")} value={formatPercent(data.today_stats.cache_rate, 0)} sub={t("popover.todayCacheRateSub", "缓存命中占比")} size={size} colorStyle={cs} />;
    case "today_tokens":
      return <MetricRow key={item.id} label={t("popover.todayTokens", "今日 Token")} value={formatNumber(data.today_stats.tokens)} sub={t("popover.todayTokensSub", "今日累计 Token")} size={size} colorStyle={cs} />;
    case "platform_today":
      return <PlatformToday key={item.id} data={data} size={size} colorStyle={cs} />;
    case "cost_trend":
      return <CostTrendCard key={item.id} item={item} data={data} size={size} colorStyle={cs} t={t} />;
    case "platform_metric":
      return <PlatformMetricCard key={item.id} item={item} data={data} size={size} colorStyle={cs} t={t} />;
    case "group_cost":
      return <GroupCostCard key={item.id} item={item} groups={groups} size={size} colorStyle={cs} t={t} />;
    case "group_tokens":
      return <GroupTokensCard key={item.id} item={item} groups={groups} size={size} colorStyle={cs} t={t} />;
    case "group_requests":
      return <GroupRequestsCard key={item.id} item={item} groups={groups} size={size} colorStyle={cs} t={t} />;
    case "group_balance":
      return <GroupBalanceCard key={item.id} item={item} groups={groups} groupDetails={groupDetails} size={size} colorStyle={cs} t={t} />;
    default:
      return null;
  }
}

/** 按 row 分组的二维网格渲染：effectiveRow = row ?? order；每行 cols = config.rows?.[row]?.cols ?? 1。 */
export function renderGrid(
  config: PopoverConfig,
  data: PopoverData,
  groups: Group[],
  groupDetails: GroupDetail[] | null,
  t: TFn,
): React.ReactNode {
  const visible = config.items.filter((i) => i.visible);
  // 行分组：缺省 row 回退 order（老配置各占一行）。
  const rowMap = new Map<number, PopoverItem[]>();
  for (const item of visible) {
    const row = item.row ?? item.order;
    const list = rowMap.get(row);
    if (list) list.push(item);
    else rowMap.set(row, [item]);
  }
  const rowNums = [...rowMap.keys()].sort((a, b) => a - b);
  return rowNums.map((row) => {
    const items = rowMap.get(row)!.sort((a, b) => a.order - b.order);
    const cols = config.rows?.[row]?.cols ?? 1;
    return (
      <div
        key={row}
        className="popover-grid-row"
        style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}
      >
        {items.map((item) => (
          <div key={item.id} className="popover-grid-cell">
            {renderItem(item, data, groups, groupDetails, t)}
          </div>
        ))}
      </div>
    );
  });
}
