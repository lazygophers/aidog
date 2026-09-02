import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize, LogicalPosition } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import type { Group, GroupDetail, PopoverConfig, StatsResult } from "./services/api";
import { groupApi, groupDetailApi, statsApi, onProxyLogUpdated } from "./services/api";
import { clamp } from "./utils/formatters";
import { applyTheme, DEFAULT_MODE } from "./themes";
import type { ThemeMode } from "./themes/types";
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
  themeMode: ThemeMode;
}

interface RawSettings {
  locale?: Locale;
  themeMode?: ThemeMode;
}

// 上次成功拉到的浮窗 config 本地缓存：下次弹出时立即据此并行发起 stats_query_batch，
// 不必等 popover_data resolve 才知道要查哪些卡（消两跳串行为并行）。
const CONFIG_CACHE_KEY = "aidog-popover-config-cache";

function loadCachedConfig(): PopoverConfig | null {
  try {
    const s = localStorage.getItem(CONFIG_CACHE_KEY);
    return s ? (JSON.parse(s) as PopoverConfig) : null;
  } catch { return null; }
}

function saveCachedConfig(config: PopoverConfig) {
  try { localStorage.setItem(CONFIG_CACHE_KEY, JSON.stringify(config)); } catch { /* ignore */ }
}

function loadSettings(): Settings {
  let raw: RawSettings = {};
  try {
    const s = localStorage.getItem("aidog-settings");
    if (s) raw = JSON.parse(s) as RawSettings;
  } catch { /* ignore */ }

  return {
    locale: raw.locale,
    themeMode: raw.themeMode ?? DEFAULT_MODE,
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
  // tray 下方居中锚点 + 顶部 y（首次测得后恒定，resize 不改变 y）；当前应用的窗口尺寸（去抖比较用）。
  const centerXRef = React.useRef<number | null>(null);
  const yLogicalRef = React.useRef<number | null>(null);
  const appliedRef = React.useRef<{ w: number; h: number } | null>(null);
  // 窗口复用后 scaleFactor 恒定，首测缓存复用，省去每次 resize 的 IPC 往返。
  const scaleRef = React.useRef<number | null>(null);

  // 重拉 popover_data + 统计批量 + 分组列表。mount 首拉 + proxy-log-updated 事件触发复用。
  // cancel 守卫防慢后端晚到 resolve 覆盖 newer 状态（参考 [[mount-fetch-late-resolve-overwrites-optimistic]]）。
  const reloadData = React.useCallback(() => {
    let cancelled = false;

    // 用上次缓存的 config 立即并行发起 batch，不等 popover_data resolve（两跳串行→并行）。
    const cachedConfig = loadCachedConfig();
    const cached = cachedConfig ? collectStatsQueries(cachedConfig) : null;
    const cachedBatch: Promise<PopoverStatsMap> | null = cached && cached.queries.length > 0
      ? statsApi.queryBatch(cached.queries)
        .then((results) => {
          const m: PopoverStatsMap = new Map();
          results.forEach((r, i) => m.set(cached.itemIds[i], r));
          return m;
        })
        .catch(() => new Map<string, StatsResult>())
      : null;

    invoke<PopoverData>("popover_data")
      .then(async (d) => {
        if (cancelled) return;
        setData(d);
        saveCachedConfig(d.config);
        const { itemIds, queries } = collectStatsQueries(d.config);
        if (queries.length === 0) {
          setStatsLoaded(true);
          return;
        }
        // config 与缓存一致（items 顺序/内容未变）：直接复用并行发起的 batch 结果。
        const sameAsCached = cached !== null && itemIds.length === cached.itemIds.length
          && itemIds.every((id, i) => id === cached.itemIds[i]);
        if (sameAsCached && cachedBatch) {
          const m = await cachedBatch;
          if (cancelled) return;
          setStatsMap(m);
          setStatsLoaded(true);
          return;
        }
        // config 已变（items 增删/换）：以缓存结果打底，补查兜底拿真实 config 对应的全量数据。
        const baseMap = cachedBatch ? await cachedBatch : new Map<string, StatsResult>();
        if (cancelled) return;
        statsApi
          .queryBatch(queries)
          .then((results) => {
            if (cancelled) return;
            const m: PopoverStatsMap = new Map(baseMap);
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
    applyTheme("dark"); // 浮窗固定深色，不跟随主窗 themeMode
    if (s.locale) {
      ensureLocaleLoaded(s.locale).then(() => i18n.changeLanguage(s.locale)).catch(() => {});
    }
    const cancel = reloadData();
    // popover = 按需创建的 Tauri webview window（托盘点击时建，收起即 destroy）；
    // 后端 log.rs app.emit 广播所有 webview，可达 → 事件订阅。1000ms debounce 避免高频 re-render。
    // 窗口销毁后本模块的监听器随 webview 一起消失（per-webview 模块状态），无需额外清理。
    const unlisten = onProxyLogUpdated(() => { reloadData(); }, 1000);
    // Rust show() 后 emit "popover-shown"：新建窗口下 mount 已自拉一次，此路径覆盖
    // 「窗口已在但需重新定位」的竞态。同时清 centerX，让下次 applySize 从 Rust 定位后的
    // 当前几何重新推导居中锚点（tray 位置若变化亦能对齐）。
    const shownPromise = listen("popover-shown", () => {
      centerXRef.current = null;
      yLogicalRef.current = null;
      reloadData();
    });
    return () => { cancel(); unlisten(); shownPromise.then((f) => f()); };
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
        // scaleFactor 恒定：首测缓存，后续 resize 复用（省 IPC 往返）。
        if (scaleRef.current === null) scaleRef.current = await win.scaleFactor();
        if (cancelled) return;
        const scale = scaleRef.current;
        // 首次（或 show 后重置）：一次 outerPosition 同时推导居中锚点 center_x 与恒定顶部 y
        // （resize 后 y 不变，二次查询系冗余，合一省一趟 IPC）。
        if (centerXRef.current === null || yLogicalRef.current === null) {
          const pos = await win.outerPosition(); // Physical
          if (cancelled) return;
          const curW = prev?.w ?? w;
          centerXRef.current = pos.x / scale + curW / 2;
          yLogicalRef.current = pos.y / scale;
        }
        appliedRef.current = { w, h };
        const newX = (centerXRef.current as number) - w / 2;
        const yLogical = yLogicalRef.current as number;
        await win.setSize(new LogicalSize(w, h));
        if (cancelled) return;
        await win.setPosition(new LogicalPosition(Math.round(newX), Math.round(yLogical)));
      } catch { /* 窗口可能已隐藏/不可用 */ }
    };

    // rAF 合并 ResizeObserver 多次触发（同帧内多次布局变化只 applySize 一次），消 resize thrash 连跳。
    let rafId: number | null = null;
    const scheduleApplySize = () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(() => { rafId = null; void applySize(); });
    };

    scheduleApplySize();
    const ro = new ResizeObserver(() => { scheduleApplySize(); });
    ro.observe(el);
    return () => {
      cancelled = true;
      if (rafId !== null) cancelAnimationFrame(rafId);
      ro.disconnect();
    };
    // 依赖 data：渲染稳定后首测；后续内容异步加载由 ResizeObserver 兜。
  }, [data]);

  // statsCtx/grid 用 useMemo 稳定引用：statsMap/statsLoaded 未变时（如仅 hover 等无关 state 变化）
  // 不重算 renderGrid（renderGrid 内含卡片 JSX 构建，卡片多时非平凡开销）。
  const statsCtx: PopoverStatsCtx = React.useMemo(
    () => ({ map: statsMap, loaded: statsLoaded }),
    [statsMap, statsLoaded],
  );
  const grid = React.useMemo(() => {
    if (!data) return null;
    return renderGrid(data.config, data, groups, groupDetails, t, statsCtx);
  }, [data, groups, groupDetails, t, statsCtx]);

  if (!data) {
    return <div ref={rootRef} className="popover-root popover-loading">{t("common.loading", "加载中...")}</div>;
  }

  return (
    <div ref={rootRef} className="popover-root">
      {grid}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Popover />
  </React.StrictMode>,
);
