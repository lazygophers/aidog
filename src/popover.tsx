import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { TodayStats } from "./services/api";
import { applyTheme } from "./themes";
import type { ThemeName, ThemeMode } from "./themes/types";
import { formatNumber, formatCostUsd, formatPercent } from "./utils/formatters";
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
  entries: PopoverEntry[];
  today_stats: TodayStats;
  proxy_running: boolean;
  proxy_port: number;
}

// ─── Theme ──────────────────────────────────────────────────

interface Settings {
  themeName: ThemeName;
  themeMode: ThemeMode;
}

function loadTheme() {
  try {
    const raw = localStorage.getItem("aidog-settings");
    if (raw) {
      const s = JSON.parse(raw) as Settings;
      applyTheme(s.themeName, s.themeMode);
      return;
    }
  } catch { /* ignore */ }
  applyTheme("liquidGlass", "light");
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

// ─── Component ──────────────────────────────────────────────

function Popover() {
  const [data, setData] = useState<PopoverData | null>(null);

  useEffect(() => {
    loadTheme();
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
    return <div className="popover-root popover-loading">Loading…</div>;
  }

  return (
    <div className="popover-root">
      {/* Header: proxy status */}
      <div className="popover-header">
        <span
          className="popover-status-dot"
          style={{ background: data.proxy_running ? "var(--status-success, #34c759)" : "var(--text-tertiary)" }}
        />
        <span className="popover-header-text">
          {data.proxy_running ? `Running :${data.proxy_port}` : "Stopped"}
        </span>
      </div>

      {/* Platform entries */}
      {data.entries.length > 0 && (
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
      )}

      {/* Today stats */}
      <div className="popover-section">
        <div className="popover-stats-title">Today</div>
        <div className="popover-stats-grid">
          <div className="popover-stat">
            <span className="popover-stat-value">{formatNumber(data.today_stats.tokens)}</span>
            <span className="popover-stat-label">tokens</span>
          </div>
          <div className="popover-stat">
            <span className="popover-stat-value">{formatCostUsd(data.today_stats.cost)}</span>
            <span className="popover-stat-label">cost</span>
          </div>
          <div className="popover-stat">
            <span className="popover-stat-value">{formatPercent(data.today_stats.cache_rate, 0)}</span>
            <span className="popover-stat-label">cache</span>
          </div>
          <div className="popover-stat">
            <span className="popover-stat-value">{formatNumber(data.today_stats.total_requests)}</span>
            <span className="popover-stat-label">reqs</span>
          </div>
        </div>
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Popover />
  </React.StrictMode>,
);
