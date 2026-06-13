import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTranslation } from "react-i18next";
import type { TodayStats, PopoverConfig, PopoverItem, TodayPlatformStat } from "./services/api";
import { applyTheme } from "./themes";
import type { ThemeName, ThemeMode } from "./themes/types";
import { formatNumber, formatCostUsd, formatPercent } from "./utils/formatters";
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
  themeName: ThemeName;
  themeMode: ThemeMode;
}

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem("aidog-settings");
    if (raw) return JSON.parse(raw) as Settings;
  } catch { /* ignore */ }
  return { themeName: "liquidGlass", themeMode: "light" };
}

// ─── Helpers ────────────────────────────────────────────────

function resolveColor(color: TrayColor): string {
  if (color.mode === "preset") {
    const map: Record<string, string> = {
      red: "var(--status-error, #ff3b30)",
      green: "var(--status-success, #34c759)",
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
        style={{ background: data.proxy_running ? "var(--status-success, #34c759)" : "var(--text-tertiary)" }}
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

function renderItem(item: PopoverItem, data: PopoverData, t: (k: string, d: string) => string): React.ReactNode {
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
    applyTheme(s.themeName ?? "liquidGlass", s.themeMode ?? "light");
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
