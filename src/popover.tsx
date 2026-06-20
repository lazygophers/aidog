import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTranslation } from "react-i18next";
import type {
  TodayStats,
  PopoverConfig,
  PopoverItem,
  TodayPlatformStat,
  StatsBucket,
  StatsQuery,
} from "./services/api";
import { statsApi } from "./services/api";
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

function renderItem(
  item: PopoverItem,
  data: PopoverData,
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
    default:
      return null;
  }
}

// ─── Component ──────────────────────────────────────────────

function Popover() {
  const { t } = useTranslation();
  const [data, setData] = useState<PopoverData | null>(null);

  useEffect(() => {
    const s = loadSettings();
    applyTheme(s.themeStyle, s.themeColor, s.themeMode);
    if (s.locale) {
      ensureLocaleLoaded(s.locale).then(() => i18n.changeLanguage(s.locale)).catch(() => {});
    }
    invoke<PopoverData>("popover_data")
      .then(setData)
      .catch(console.error);
  }, []);

  // 失焦自动关闭
  useEffect(() => {
    const current = getCurrentWindow();
    const unlisten = current.onFocusChanged(({ payload: focused }) => {
      if (!focused) current.destroy().catch(() => {});
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  if (!data) {
    return <div className="popover-root popover-loading">{t("common.loading", "加载中...")}</div>;
  }

  const visibleItems = data.config.items
    .filter((i) => i.visible)
    .sort((a, b) => a.order - b.order);

  return (
    <div className="popover-root">
      {visibleItems.map((item) => renderItem(item, data, t))}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Popover />
  </React.StrictMode>,
);
