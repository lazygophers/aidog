import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize, LogicalPosition } from "@tauri-apps/api/window";
import { useTranslation } from "react-i18next";
import type { Group, GroupDetail } from "./services/api";
import { groupApi, groupDetailApi, statsApi, onProxyLogUpdated } from "./services/api";
import { clamp } from "./utils/formatters";
import { applyTheme, DEFAULT_STYLE, DEFAULT_COLOR, DEFAULT_MODE } from "./themes";
import type { ThemeStyle, ThemeColor, ThemeMode } from "./themes/types";
import {
  renderGrid,
  collectStatsQueries,
  type PopoverData,
  type PopoverStatsMap,
  type PopoverStatsCtx,
} from "./components/PopoverCards";
import i18n, { ensureLocaleLoaded, type Locale } from "./locales";
import "./styles/popover.css";

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

// ─── Component ──────────────────────────────────────────────

// ─── Auto-size constants ────────────────────────────────────
const MIN_W = 300;
const MAX_W = 480;
const MIN_H = 80;
const MAX_H = 600;
const DELTA = 1; // 尺寸/位置 delta ≤ 1px 不触发，防抖动循环。

function Popover() {
  const { t } = useTranslation();
  const [data, setData] = useState<PopoverData | null>(null);
  const [groups, setGroups] = useState<Group[]>([]);
  const [groupDetails, setGroupDetails] = useState<GroupDetail[] | null>(null);
  // 各统计卡数据：一次批量 IPC 拉全部（item.id → StatsResult），消除每卡 fan-out。
  const [statsMap, setStatsMap] = useState<PopoverStatsMap>(new Map());
  const [statsLoaded, setStatsLoaded] = useState(false);
  const rootRef = React.useRef<HTMLDivElement>(null);
  // tray 下方居中锚点（首次测得后恒定）；当前应用的窗口尺寸（去抖比较用）。
  const centerXRef = React.useRef<number | null>(null);
  const appliedRef = React.useRef<{ w: number; h: number } | null>(null);

  // 重拉 popover_data + 统计批量 + 分组列表。mount 首拉 + proxy-log-updated 事件触发复用。
  // cancel 守卫防慢后端晚到 resolve 覆盖 newer 状态（参考 [[mount-fetch-late-resolve-overwrites-optimistic]]）。
  const reloadData = React.useCallback(() => {
    let cancelled = false;
    invoke<PopoverData>("popover_data")
      .then((d) => {
        if (cancelled) return;
        setData(d);
        // config 到手后一次性批量拉所有统计卡数据（cost_trend / platform_metric / group_*）。
        const { itemIds, queries } = collectStatsQueries(d.config);
        if (queries.length === 0) {
          setStatsLoaded(true);
          return;
        }
        statsApi
          .queryBatch(queries)
          .then((results) => {
            if (cancelled) return;
            const m: PopoverStatsMap = new Map();
            results.forEach((r, i) => m.set(itemIds[i], r));
            setStatsMap(m);
            setStatsLoaded(true);
          })
          .catch(() => { if (!cancelled) setStatsLoaded(true); });
      })
      .catch(console.error);
    // 分组名 + 分组余额数据（group_* 卡片用）。顶层一次性 fetch，避免每卡重复请求。
    groupApi.list().then((v) => { if (!cancelled) setGroups(v); }).catch(() => {});
    groupDetailApi.list().then((v) => { if (!cancelled) setGroupDetails(v); }).catch(() => { if (!cancelled) setGroupDetails([]); });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    const s = loadSettings();
    applyTheme(s.themeStyle, s.themeColor, s.themeMode);
    if (s.locale) {
      ensureLocaleLoaded(s.locale).then(() => i18n.changeLanguage(s.locale)).catch(() => {});
    }
    const cancel = reloadData();
    // popover = 独立 Tauri webview window（WebviewWindowBuilder label="popover"，app_setup.rs:209）；
    // 后端 log.rs:153 app.emit 广播所有 webview，可达 → 事件订阅。1000ms debounce 避免高频 re-render。
    const unlisten = onProxyLogUpdated(() => { reloadData(); }, 1000);
    return () => { cancel(); unlisten(); };
  }, [reloadData]);

  // 失焦自动关闭由 Rust 端处理（startup.rs on_window_event Focused(false)），
  // 不在 webview 内监听：依赖 JS→Rust IPC 的写法在 macOS 下偶发失效。

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

  const statsCtx: PopoverStatsCtx = { map: statsMap, loaded: statsLoaded };
  return (
    <div ref={rootRef} className="popover-root">
      {renderGrid(data.config, data, groups, groupDetails, t, statsCtx)}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Popover />
  </React.StrictMode>,
);
