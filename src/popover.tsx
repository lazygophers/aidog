import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize, LogicalPosition } from "@tauri-apps/api/window";
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
} from "./services/api";
import { statsApi, groupApi, groupDetailApi } from "./services/api";
import { applyTheme, DEFAULT_STYLE, DEFAULT_COLOR, DEFAULT_MODE } from "./themes";
import type { ThemeStyle, ThemeColor, ThemeMode } from "./themes/types";
import { formatNumber, formatCostUsd, formatPercent } from "./utils/formatters";
import { CostTrendChart } from "./components/shared/CostTrendChart";
import i18n, { ensureLocaleLoaded, type Locale } from "./locales";
import "./styles/popover.css";

// ─── Types ──────────────────────────────────────────────────

interface TrayColor {
  mode: string;
  value: string;
}

interface PopoverEntry {
  name: string;
  value: string;
  color: TrayColor;
}

interface PopoverData {
  config: PopoverConfig;
  entries: PopoverEntry[];
  today_stats: TodayStats;
  platform_today: TodayPlatformStat[];
  proxy_running: boolean;
  proxy_port: number;
}

// ─── Theme + Locale ─────────────────────────────────────────

interface Settings {
  locale?: Locale;
  themeStyle: ThemeStyle;
  themeColor: ThemeColor;
  themeMode: ThemeMode;
}

/** 旧 themeName → 新 {style,color} 迁移映射（与 AppContext 保持一致）。 */
const LEGACY_THEME_MAP: Record<string, { style: ThemeStyle; color: ThemeColor }> = {
  liquidGlass: { style: "liquidGlass", color: "appleBlue" },
  nord: { style: "flat", color: "nord" },
  dracula: { style: "flat", color: "dracula" },
  catppuccin: { style: "flat", color: "catppuccin" },
  solarized: { style: "flat", color: "solarized" },
};

interface RawSettings {
  locale?: Locale;
  themeStyle?: ThemeStyle;
  themeColor?: ThemeColor;
  themeMode?: ThemeMode;
  themeName?: string;
}

function loadSettings(): Settings {
  let raw: RawSettings = {};
  try {
    const s = localStorage.getItem("aidog-settings");
    if (s) raw = JSON.parse(s) as RawSettings;
  } catch { /* ignore */ }

  const locale = raw.locale;
  const themeMode: ThemeMode = raw.themeMode ?? DEFAULT_MODE;
  if (raw.themeStyle && raw.themeColor) {
    return { locale, themeStyle: raw.themeStyle, themeColor: raw.themeColor, themeMode };
  }
  const migrated = raw.themeName ? LEGACY_THEME_MAP[raw.themeName] : undefined;
  return {
    locale,
    themeStyle: migrated?.style ?? DEFAULT_STYLE,
    themeColor: migrated?.color ?? DEFAULT_COLOR,
    themeMode,
  };
}

// ─── Helpers ────────────────────────────────────────────────

function resolveColor(color: TrayColor): string {
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

function PlatformBalance({ data }: { data: PopoverData }) {
  if (data.entries.length === 0) return null;
  return (
    <div className="popover-section">
      {data.entries.map((e, i) => (
        <div className="popover-entry" key={i}>
          <span className="popover-entry-dot" style={{ background: resolveColor(e.color) }} />
          <span className="popover-entry-name">{e.name}</span>
          <span className="popover-entry-value" style={{ color: resolveColor(e.color) }}>
            {e.value}
          </span>
        </div>
      ))}
    </div>
  );
}

function MetricRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="popover-section">
      <div className="popover-metric-row">
        <span className="popover-metric-label">{label}</span>
        <span className="popover-metric-value">{value}</span>
      </div>
    </div>
  );
}

function PlatformToday({ data }: { data: PopoverData }) {
  const { t } = useTranslation();
  return (
    <div className="popover-section">
      <div className="popover-stats-title">{t("popover.platformToday", "各平台今日")}</div>
      {data.platform_today.length === 0 ? (
        <div className="popover-empty">{t("popover.noUsageToday", "今日暂无用量")}</div>
      ) : (
        data.platform_today.map((p) => (
          <div className="popover-platform-row" key={p.platform_id}>
            <span className="popover-platform-name">
              {p.platform_name || t("popover.unknownPlatform", "未知平台")}
            </span>
            <span className="popover-platform-value">{formatCostUsd(p.cost)}</span>
            <span className="popover-platform-sub">{formatNumber(p.tokens)} tok</span>
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
function trendTitle(
  item: PopoverItem,
  data: PopoverData,
  t: (k: string, d: string, opts?: Record<string, unknown>) => string,
): string {
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
  t,
}: {
  item: PopoverItem;
  data: PopoverData;
  t: (k: string, d: string, opts?: Record<string, unknown>) => string;
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

  return (
    <div className="popover-section">
      <div className="popover-stats-title">{trendTitle(item, data, t)}</div>
      {failed ? (
        <div className="popover-empty">{t("popover.trendLoadError", "加载失败")}</div>
      ) : buckets === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : buckets.length === 0 ? (
        <div className="popover-empty">{t("popover.noUsageToday", "今日暂无用量")}</div>
      ) : (
        <CostTrendChart buckets={buckets} />
      )}
    </div>
  );
}

// ─── Platform metric card ───────────────────────────────────

/** platform_metric 卡片标题（取平台名，兜底 scope_ref）。 */
function platformMetricTitle(
  item: PopoverItem,
  data: PopoverData,
  t: (k: string, d: string, opts?: Record<string, unknown>) => string,
): string {
  const ref = item.scope_ref ?? "";
  const pid = Number(ref);
  const p = data.platform_today.find((x) => x.platform_id === pid);
  const name = p?.platform_name || ref || t("popover.unknownPlatform", "未知平台");
  return t("popover.platformMetricTitle", "{{name}} 用量", { name });
}

function PlatformMetricCard({
  item,
  data,
  t,
}: {
  item: PopoverItem;
  data: PopoverData;
  t: (k: string, d: string, opts?: Record<string, unknown>) => string;
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
    <div className="popover-section">
      <div className="popover-stats-title">{platformMetricTitle(item, data, t)}</div>
      {failed ? (
        <div className="popover-empty">{t("popover.trendLoadError", "加载失败")}</div>
      ) : overview === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : (
        <div className="popover-platform-row">
          <span className="popover-platform-value">{formatCostUsd(overview.total_cost)}</span>
          <span className="popover-platform-sub">{formatNumber(tokens)} tok</span>
        </div>
      )}
    </div>
  );
}

// ─── Group metric cards ─────────────────────────────────────

/** group_* 卡片分组名（按 group_key 查 groups，兜底 scope_ref）。 */
function groupName(
  item: PopoverItem,
  groups: Group[],
  t: (k: string, d: string, opts?: Record<string, unknown>) => string,
): string {
  const ref = item.scope_ref ?? "";
  const g = groups.find((x) => x.group_key === ref);
  return g?.name || ref || t("popover.trendScopeGroup", "分组");
}

/** group_cost：分组金额（带时间窗，overview.total_cost）。 */
function GroupCostCard({
  item,
  groups,
  t,
}: {
  item: PopoverItem;
  groups: Group[];
  t: (k: string, d: string, opts?: Record<string, unknown>) => string;
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
    <div className="popover-section">
      <div className="popover-stats-title">{t("popover.groupCostTitle", "{{name}} 金额", { name: groupName(item, groups, t) })}</div>
      {failed ? (
        <div className="popover-empty">{t("popover.trendLoadError", "加载失败")}</div>
      ) : overview === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : (
        <div className="popover-metric-row">
          <span className="popover-metric-value">{formatCostUsd(overview.total_cost)}</span>
        </div>
      )}
    </div>
  );
}

/** group_tokens：分组今日 Token（input+output，固定今日窗）。 */
function GroupTokensCard({
  item,
  groups,
  t,
}: {
  item: PopoverItem;
  groups: Group[];
  t: (k: string, d: string, opts?: Record<string, unknown>) => string;
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
    <div className="popover-section">
      <div className="popover-stats-title">{t("popover.groupTokensTitle", "{{name}} 今日 Token", { name: groupName(item, groups, t) })}</div>
      {failed ? (
        <div className="popover-empty">{t("popover.trendLoadError", "加载失败")}</div>
      ) : overview === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : (
        <div className="popover-metric-row">
          <span className="popover-metric-value">{formatNumber(tokens)} tok</span>
        </div>
      )}
    </div>
  );
}

/** group_requests：分组今日请求数（固定今日窗，overview.total_requests）。 */
function GroupRequestsCard({
  item,
  groups,
  t,
}: {
  item: PopoverItem;
  groups: Group[];
  t: (k: string, d: string, opts?: Record<string, unknown>) => string;
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
    <div className="popover-section">
      <div className="popover-stats-title">{t("popover.groupRequestsTitle", "{{name}} 今日请求", { name: groupName(item, groups, t) })}</div>
      {failed ? (
        <div className="popover-empty">{t("popover.trendLoadError", "加载失败")}</div>
      ) : overview === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : (
        <div className="popover-metric-row">
          <span className="popover-metric-value">{formatNumber(overview.total_requests)}</span>
        </div>
      )}
    </div>
  );
}

/** group_balance：分组余额（组内平台 est_balance_remaining 求和，点时值）。 */
function GroupBalanceCard({
  item,
  groups,
  groupDetails,
  t,
}: {
  item: PopoverItem;
  groups: Group[];
  groupDetails: GroupDetail[] | null;
  t: (k: string, d: string, opts?: Record<string, unknown>) => string;
}) {
  const ref = item.scope_ref ?? "";
  const detail = groupDetails?.find((d) => d.group.group_key === ref);
  const balance = detail
    ? detail.platforms.reduce((sum, p) => sum + (p.platform.est_balance_remaining || 0), 0)
    : 0;

  return (
    <div className="popover-section">
      <div className="popover-stats-title">{t("popover.groupBalanceTitle", "{{name}} 余额", { name: groupName(item, groups, t) })}</div>
      {groupDetails === null ? (
        <div className="popover-empty">{t("common.loading", "加载中...")}</div>
      ) : detail === undefined ? (
        <div className="popover-empty">{t("popover.trendNoGroup", "无分组")}</div>
      ) : (
        <div className="popover-metric-row">
          <span className="popover-metric-value">{formatCostUsd(balance)}</span>
        </div>
      )}
    </div>
  );
}

function renderItem(
  item: PopoverItem,
  data: PopoverData,
  groups: Group[],
  groupDetails: GroupDetail[] | null,
  t: (k: string, d: string, opts?: Record<string, unknown>) => string,
): React.ReactNode {
  switch (item.item_type) {
    case "proxy_status":
      return <ProxyStatus key={item.id} data={data} />;
    case "platform_balance":
      return <PlatformBalance key={item.id} data={data} />;
    case "today_cost":
      return <MetricRow key={item.id} label={t("popover.todayCost", "今日金额")} value={formatCostUsd(data.today_stats.cost)} />;
    case "today_cache_rate":
      return <MetricRow key={item.id} label={t("popover.todayCacheRate", "今日缓存率")} value={formatPercent(data.today_stats.cache_rate, 0)} />;
    case "today_tokens":
      return <MetricRow key={item.id} label={t("popover.todayTokens", "今日 Token")} value={formatNumber(data.today_stats.tokens)} />;
    case "platform_today":
      return <PlatformToday key={item.id} data={data} />;
    case "cost_trend":
      return <CostTrendCard key={item.id} item={item} data={data} t={t} />;
    case "platform_metric":
      return <PlatformMetricCard key={item.id} item={item} data={data} t={t} />;
    case "group_cost":
      return <GroupCostCard key={item.id} item={item} groups={groups} t={t} />;
    case "group_tokens":
      return <GroupTokensCard key={item.id} item={item} groups={groups} t={t} />;
    case "group_requests":
      return <GroupRequestsCard key={item.id} item={item} groups={groups} t={t} />;
    case "group_balance":
      return <GroupBalanceCard key={item.id} item={item} groups={groups} groupDetails={groupDetails} t={t} />;
    default:
      return null;
  }
}

// ─── Component ──────────────────────────────────────────────

// ─── Auto-size constants ────────────────────────────────────
const MIN_W = 300;
const MAX_W = 480;
const MIN_H = 80;
const MAX_H = 600;
const DELTA = 1; // 尺寸/位置 delta ≤ 1px 不触发，防抖动循环。

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

function Popover() {
  const { t } = useTranslation();
  const [data, setData] = useState<PopoverData | null>(null);
  const [groups, setGroups] = useState<Group[]>([]);
  const [groupDetails, setGroupDetails] = useState<GroupDetail[] | null>(null);
  const rootRef = React.useRef<HTMLDivElement>(null);
  // tray 下方居中锚点（首次测得后恒定）；当前应用的窗口尺寸（去抖比较用）。
  const centerXRef = React.useRef<number | null>(null);
  const appliedRef = React.useRef<{ w: number; h: number } | null>(null);

  useEffect(() => {
    const s = loadSettings();
    applyTheme(s.themeStyle, s.themeColor, s.themeMode);
    if (s.locale) {
      ensureLocaleLoaded(s.locale).then(() => i18n.changeLanguage(s.locale)).catch(() => {});
    }
    invoke<PopoverData>("popover_data")
      .then(setData)
      .catch(console.error);
    // 分组名 + 分组余额数据（group_* 卡片用）。顶层一次性 fetch，避免每卡重复请求。
    groupApi.list().then(setGroups).catch(() => {});
    groupDetailApi.list().then(setGroupDetails).catch(() => setGroupDetails([]));
  }, []);

  // 失焦自动关闭
  useEffect(() => {
    const current = getCurrentWindow();
    const unlisten = current.onFocusChanged(({ payload: focused }) => {
      if (!focused) current.destroy().catch(() => {});
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // 窗口尺寸随内容自适应 + 保持 tray 下方居中。
  useEffect(() => {
    if (!data) return;
    const el = rootRef.current;
    if (!el) return;
    const win = getCurrentWindow();
    let cancelled = false;

    const applySize = async () => {
      const w = clamp(Math.ceil(el.offsetWidth), MIN_W, MAX_W);
      const h = clamp(Math.ceil(el.offsetHeight), MIN_H, MAX_H);
      const prev = appliedRef.current;
      if (prev && Math.abs(prev.w - w) <= DELTA && Math.abs(prev.h - h) <= DELTA) return;
      try {
        // 首次：以当前窗口几何推导居中锚点 center_x（logical），全程恒定。
        if (centerXRef.current === null) {
          const pos = await win.outerPosition(); // Physical
          const scale = await win.scaleFactor();
          if (cancelled) return;
          const curW = prev?.w ?? w;
          centerXRef.current = pos.x / scale + curW / 2;
        }
        appliedRef.current = { w, h };
        await win.setSize(new LogicalSize(w, h));
        if (cancelled) return;
        // resize 后按恒定 center_x 重算 x，顶部 y 不变。
        const pos = await win.outerPosition();
        const scale = await win.scaleFactor();
        if (cancelled) return;
        const yLogical = pos.y / scale;
        const newX = (centerXRef.current as number) - w / 2;
        await win.setPosition(new LogicalPosition(Math.round(newX), Math.round(yLogical)));
      } catch { /* 窗口可能已销毁 */ }
    };

    void applySize();
    const ro = new ResizeObserver(() => { void applySize(); });
    ro.observe(el);
    return () => { cancelled = true; ro.disconnect(); };
    // 依赖 data：渲染稳定后首测；后续内容异步加载由 ResizeObserver 兜。
  }, [data]);

  if (!data) {
    return <div ref={rootRef} className="popover-root popover-loading">{t("common.loading", "加载中...")}</div>;
  }

  const visibleItems = data.config.items
    .filter((i) => i.visible)
    .sort((a, b) => a.order - b.order);

  return (
    <div ref={rootRef} className="popover-root">
      {visibleItems.map((item) => renderItem(item, data, groups, groupDetails, t))}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Popover />
  </React.StrictMode>,
);
