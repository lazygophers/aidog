import { useState, useEffect, useMemo, useRef, Fragment } from "react";
import { useTranslation } from "react-i18next";
import {
  platformApi,
  trayConfigApi,
  type Platform,
  type TrayConfig,
  type TrayItem,
  type TrayColor,
  type TodayStats,
} from "../services/api";

const PRESET_COLORS: { value: string; cssVar: string }[] = [
  { value: "follow", cssVar: "var(--text-primary)" },
  { value: "red", cssVar: "#ff453a" },
  { value: "green", cssVar: "#32d74b" },
  { value: "orange", cssVar: "#ff9f0a" },
];

/** 去除尾部多余的零：0.111000 → 0.111, 10.10100 → 10.101, 0.000 → 0 */
function trimZeros(s: string): string {
  if (!s.includes(".")) return s;
  return s.replace(/\.?0+$/, "");
}

const DEFAULT_FONT_SIZE = 9;

const PRESET_SEPARATORS = [
  { label: "|", value: "|" },
  { label: "·", value: "·" },
  { label: "—", value: "—" },
  { label: "/", value: "/" },
  { label: "»", value: "»" },
  { label: "空格", value: " " },
];

const ALIGN_OPTIONS = [
  { value: "left", label: "←" },
  { value: "center", label: "↔" },
  { value: "right", label: "→" },
] as const;

const TODAY_METRICS = [
  { value: "tokens", label: "Tokens" },
  { value: "cache_rate", label: "Cache%" },
  { value: "cost", label: "花费$" },
  { value: "requests", label: "请求" },
] as const;

function defaultColor(): TrayColor {
  return { mode: "follow", value: "" };
}

function makePlatformItem(platformId: number, display: "balance" | "coding", order: number): TrayItem {
  return {
    item_type: "platform", platform_id: platformId, display, metric: null,
    color: defaultColor(), font_size: DEFAULT_FONT_SIZE, line_mode: "two",
    align: "left", align_row2: null, enabled: true, order,
  };
}

function makeTodayUsageItem(metric: string, order: number): TrayItem {
  return {
    item_type: "today_usage", platform_id: null, display: "", metric,
    color: defaultColor(), font_size: DEFAULT_FONT_SIZE, line_mode: "two",
    align: "left", align_row2: null, enabled: true, order,
  };
}

function makeSeparatorItem(separator: string, order: number): TrayItem {
  return {
    item_type: "separator", platform_id: null, display: separator, metric: null,
    color: defaultColor(), font_size: DEFAULT_FONT_SIZE, line_mode: "single",
    align: "center", align_row2: null, enabled: true, order,
  };
}

function isRiskyHex(hex: string): boolean {
  const m = /^#?([0-9a-fA-F]{6})$/.exec(hex.trim());
  if (!m) return false;
  const n = parseInt(m[1], 16);
  const r = (n >> 16) & 0xff;
  const g = (n >> 8) & 0xff;
  const b = n & 0xff;
  return 0.299 * r + 0.587 * g + 0.114 * b < 40 || 0.299 * r + 0.587 * g + 0.114 * b > 215;
}

/** 计算单个展示项的预览文本（与后端 tray_segments 对齐） */
function computeItemText(item: TrayItem, platform: Platform | undefined, todayStats: TodayStats | null): { label: string; value: string } {
  if (item.item_type === "today_usage") {
    const s = todayStats ?? { tokens: 0, cache_rate: 0, cost: 0, total_requests: 0 };
    switch (item.metric || "tokens") {
      case "cache_rate": return { label: "Cache", value: `${s.cache_rate.toFixed(0)}%` };
      case "cost": return { label: "花费", value: `$${trimZeros(s.cost.toFixed(4))}` };
      case "requests": return { label: "请求", value: `${s.total_requests}` };
      default: return { label: "今日", value: `${s.tokens} tok` };
    }
  }
  if (!platform) return { label: `#${item.platform_id}`, value: "--.--" };
  let isCoding = item.display === "coding";
  let util = 0;
  if (platform.est_coding_plan) {
    try {
      const p = JSON.parse(platform.est_coding_plan);
      if (p?.tiers?.length) { isCoding = true; util = p.tiers[0].est_utilization ?? 0; }
    } catch { /* */ }
  }
  return { label: platform.name, value: isCoding ? `${Math.max(0, 100 - util).toFixed(0)}%` : `$${trimZeros(platform.est_balance_remaining.toFixed(2))}` };
}

interface Column { item: TrayItem; label: string; value: string; isTwo: boolean; align: string; alignRow2: string }
interface Gap { separator: string | null; sepIndex: number | null }

export function TrayConfigTab() {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [config, setConfig] = useState<TrayConfig>({ separator: "  ", items: [] });
  const [todayStats, setTodayStats] = useState<TodayStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  const [expandedIdx, setExpandedIdx] = useState<number | null>(null);
  const [showAddMenu, setShowAddMenu] = useState(false);

  // Drag state (config list)
  const [drag, setDrag] = useState<{ from: number; to: number } | null>(null);
  const dragStartRef = useRef<{ y: number; index: number } | null>(null);
  const didDragRef = useRef(false);

  // Preview drag state (horizontal)
  const [previewDrag, setPreviewDrag] = useState<{ from: number; to: number } | null>(null);
  const previewDragRef = useRef<{ x: number; colIdx: number } | null>(null);
  const didPreviewDragRef = useRef(false);

  // Preview popover state
  const [popover, setPopover] = useState<{ colIdx: number; rect: DOMRect } | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const [list, cfg, stats] = await Promise.all([
          platformApi.list(), trayConfigApi.get(), trayConfigApi.todayStats(),
        ]);
        setPlatforms(list.filter((p) => p.enabled));
        setConfig(cfg);
        setTodayStats(stats);
      } catch (e) { console.error(e); }
      setLoading(false);
    })();
  }, []);

  useEffect(() => {
    const timer = setInterval(async () => {
      try { setTodayStats(await trayConfigApi.todayStats()); } catch { /* */ }
    }, 30_000);
    return () => clearInterval(timer);
  }, []);

  const persist = async (next: TrayConfig) => {
    setConfig(next);
    try { await trayConfigApi.set(next); } catch (e) { console.error(e); setMessage(String(e)); }
  };

  const withOrders = (items: TrayItem[]): TrayItem[] => items.map((it, idx) => ({ ...it, order: idx }));

  const updateItem = (index: number, patch: Partial<TrayItem>) => {
    const items = config.items.map((it, i) => (i === index ? { ...it, ...patch } : it));
    persist({ ...config, items: withOrders(items) });
  };

  const removeItem = (index: number) => {
    const items = config.items.filter((_, i) => i !== index);
    if (expandedIdx !== null && expandedIdx >= items.length) setExpandedIdx(null);
    persist({ ...config, items: withOrders(items) });
  };

  const addPlatform = (pid: number) => {
    const items = [...config.items, makePlatformItem(pid, "balance", config.items.length)];
    persist({ ...config, items: withOrders(items) });
    setShowAddMenu(false);
  };

  const addTodayUsage = (metric: string) => {
    const items = [...config.items, makeTodayUsageItem(metric, config.items.length)];
    persist({ ...config, items: withOrders(items) });
    setShowAddMenu(false);
  };

  // ── Decompose items into columns + gaps ──
  const layout = useMemo(() => {
    const enabled = config.items.filter((i) => i.enabled).sort((a, b) => a.order - b.order);

    const columns: Column[] = [];
    const gaps: Gap[] = [];
    let pendingSep: string | null = null;
    let pendingSepIdx: number | null = null;

    for (const item of enabled) {
      if (item.item_type === "separator") {
        pendingSep = item.display || "·";
        pendingSepIdx = config.items.indexOf(item);
      } else {
        if (columns.length > 0) {
          gaps.push({ separator: pendingSep, sepIndex: pendingSepIdx });
        }
        pendingSep = null;
        pendingSepIdx = null;
        const p = item.item_type === "platform" && item.platform_id
          ? platforms.find((pp) => pp.id === item.platform_id) : undefined;
        const { label, value } = computeItemText(item, p, todayStats);
        columns.push({
          item, label, value, isTwo: item.line_mode === "two",
          align: item.align, alignRow2: item.align_row2 || item.align,
        });
      }
    }

    // 多列并排：行数 = max(各列行数)，不是 sum
    const totalLines = columns.length === 0 ? 0 : Math.max(...columns.map((c) => c.isTwo ? 2 : 1));
    return { columns, gaps, totalLines, overBudget: totalLines > 2 };
  }, [config, platforms, todayStats]);

  // ── Preview drag handlers (horizontal) ──
  const handlePreviewPointerDown = (e: React.PointerEvent, colIdx: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    previewDragRef.current = { x: e.clientX, colIdx };
  };

  const handlePreviewPointerMove = (e: React.PointerEvent) => {
    const start = previewDragRef.current;
    if (!start) return;
    if (!previewDrag) {
      if (Math.abs(e.clientX - start.x) < 5) return;
      setPreviewDrag({ from: start.colIdx, to: start.colIdx });
      didPreviewDragRef.current = true;
    }
    // Find target column by horizontal position
    const els = document.querySelectorAll("[data-preview-col]");
    let closest = previewDrag?.from ?? start.colIdx;
    for (let i = 0; i < els.length; i++) {
      const rect = els[i].getBoundingClientRect();
      if (e.clientX > rect.left + rect.width / 2) closest = i;
      else break;
    }
    setPreviewDrag((prev) => (prev ? { ...prev, to: closest } : null));
  };

  const handlePreviewPointerUp = () => {
    if (previewDrag) {
      const { from, to } = previewDrag;
      const effectiveTo = from < to ? to - 1 : to;
      if (from !== effectiveTo) {
        // Map column indices back to config.items indices
        const fromItem = layout.columns[from]?.item;
        const toItem = layout.columns[effectiveTo]?.item;
        if (fromItem && toItem) {
          const items = [...config.items];
          const fi = items.indexOf(fromItem);
          const ti = items.indexOf(toItem);
          if (fi >= 0 && ti >= 0) {
            const [moved] = items.splice(fi, 1);
            items.splice(ti, 0, moved);
            persist({ ...config, items: withOrders(items) });
          }
        }
      }
      setPreviewDrag(null);
    }
    previewDragRef.current = null;
    setTimeout(() => { didPreviewDragRef.current = false; }, 50);
  };

  const handlePreviewClick = (colIdx: number, el: HTMLElement) => {
    if (didPreviewDragRef.current) return;
    if (popover?.colIdx === colIdx) { setPopover(null); return; }
    setPopover({ colIdx, rect: el.getBoundingClientRect() });
  };

  // ── Config list drag handlers ──
  const handlePointerDown = (e: React.PointerEvent, index: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragStartRef.current = { y: e.clientY, index };
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    const start = dragStartRef.current;
    if (!start) return;
    if (!drag) {
      if (Math.abs(e.clientY - start.y) < 5) return;
      setDrag({ from: start.index, to: start.index });
      didDragRef.current = true;
    }
    const el = document.querySelectorAll("[data-tray-item]");
    let closest = drag?.from ?? start.index;
    for (let i = 0; i < el.length; i++) {
      const rect = el[i].getBoundingClientRect();
      if (e.clientY > rect.top + rect.height / 2) closest = i;
      else break;
    }
    setDrag((prev) => (prev ? { ...prev, to: closest } : null));
  };

  const handlePointerUp = () => {
    if (drag) {
      const effectiveTo = drag.from < drag.to ? drag.to - 1 : drag.to;
      if (drag.from !== effectiveTo) {
        const items = [...config.items];
        const [moved] = items.splice(drag.from, 1);
        items.splice(effectiveTo, 0, moved);
        persist({ ...config, items: withOrders(items) });
      }
      setDrag(null);
    }
    dragStartRef.current = null;
    setTimeout(() => { didDragRef.current = false; }, 50);
  };

  const platformName = (id: number | null): string => {
    if (id === null) return "";
    const p = platforms.find((pp) => pp.id === id);
    return p ? p.name : `#${id}`;
  };

  const usedPlatformIds = new Set(config.items.filter((i) => i.item_type === "platform").map((i) => i.platform_id));
  const availablePlatforms = platforms.filter((p) => !usedPlatformIds.has(p.id));

  if (loading) {
    return <div className="text-secondary" style={{ fontSize: 13, padding: 20 }}>{t("common.loading", "加载中...")}</div>;
  }

  const gripSvg = (
    <svg width="14" height="20" viewBox="0 0 14 20" fill="currentColor">
      <circle cx="4" cy="3" r="1.8" /><circle cx="4" cy="10" r="1.8" /><circle cx="4" cy="17" r="1.8" />
      <circle cx="10" cy="3" r="1.8" /><circle cx="10" cy="10" r="1.8" /><circle cx="10" cy="17" r="1.8" />
    </svg>
  );

  const cssAlign = (a: string) => a === "center" ? "center" : a === "right" ? "right" : "left";

  const hasTwoLine = layout.columns.some((c) => c.isTwo);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, maxWidth: 720 }} onClick={() => setPopover(null)}>
      {/* ── Preview Bar ── */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("tray.preview", "实时预览")}</div>
          <div style={{
            display: "flex", alignItems: "center", gap: 6, fontSize: 11,
            color: layout.overBudget ? "#ff9f0a" : layout.totalLines === 2 ? "var(--accent)" : "var(--text-secondary)",
          }}>
            <span style={{ fontWeight: 600 }}>{t("tray.lineBudget", "行数")} {layout.totalLines}/2</span>
            {layout.overBudget && <span style={{ color: "#ff9f0a" }}>{t("tray.overBudgetHint", "超限")}</span>}
          </div>
        </div>

        {/* Simulated macOS menu bar — match tray rendering precisely */}
        <div style={{
          background: "rgba(30, 30, 30, 0.95)", borderRadius: 8,
          padding: hasTwoLine ? "4px 14px" : "2px 14px",
          minHeight: hasTwoLine ? 40 : 26, display: "flex", alignItems: "center",
          fontFamily: '-apple-system, "SF Pro Text", system-ui, sans-serif',
          fontSize: hasTwoLine ? 12 : 17, // 9pt ≈ 12px (two-line), 13pt ≈ 17px (single)
          color: "rgba(255,255,255,0.85)", userSelect: "none",
        }}>
          {layout.columns.length === 0 ? (
            <span style={{ color: "rgba(255,255,255,0.35)", fontStyle: "italic" }}>
              {t("tray.previewEmpty", "暂无展示项")}
            </span>
          ) : !hasTwoLine ? (
            /* ── Single-line: columns with gaps, draggable + clickable ── */
            <div style={{ display: "flex", alignItems: "center", gap: 0 }}>
              {layout.columns.map((col, i) => (
                <Fragment key={i}>
                  {i > 0 && (
                    <span style={{
                      display: "inline-flex", alignItems: "center", justifyContent: "center",
                      width: layout.gaps[i - 1]?.separator ? "auto" : 5,
                      padding: layout.gaps[i - 1]?.separator ? "0 5px" : 0,
                      fontSize: 12, color: "rgba(255,255,255,0.35)",
                    }}>
                      {layout.gaps[i - 1]?.separator || ""}
                    </span>
                  )}
                  <span
                    data-preview-col={i}
                    style={{
                      textAlign: cssAlign(col.align), whiteSpace: "pre",
                      cursor: "grab", padding: "2px 4px", borderRadius: 4,
                      opacity: previewDrag?.from === i ? 0.4 : 1,
                      outline: popover?.colIdx === i ? "2px solid var(--accent)" : "none",
                      transition: "opacity 0.15s",
                    }}
                    onPointerDown={(e) => handlePreviewPointerDown(e, i)}
                    onPointerMove={handlePreviewPointerMove}
                    onPointerUp={handlePreviewPointerUp}
                    onClick={(e) => handlePreviewClick(i, e.currentTarget)}
                  >
                    {col.label} {col.value}
                  </span>
                </Fragment>
              ))}
            </div>
          ) : (
            /* ── Two-line: grid with gaps, draggable + clickable ── */
            <div style={{ display: "grid", gridAutoFlow: "column", gap: 0, width: "100%" }}>
              {layout.columns.map((col, i) => (
                <Fragment key={i}>
                  {i > 0 && (
                    <div style={{
                      display: "flex", alignItems: "center", justifyContent: "center",
                      width: layout.gaps[i - 1]?.separator ? "auto" : 5,
                      padding: layout.gaps[i - 1]?.separator ? "0 5px" : 0,
                      fontSize: 12, color: "rgba(255,255,255,0.35)", height: "100%",
                    }}>
                      {layout.gaps[i - 1]?.separator || ""}
                    </div>
                  )}
                  <div
                    data-preview-col={i}
                    style={{
                      display: "flex", flexDirection: "column", alignItems: "stretch",
                      cursor: "grab", padding: "2px 4px", borderRadius: 4,
                      opacity: previewDrag?.from === i ? 0.4 : 1,
                      outline: popover?.colIdx === i ? "2px solid var(--accent)" : "none",
                      transition: "opacity 0.15s",
                    }}
                    onPointerDown={(e) => handlePreviewPointerDown(e, i)}
                    onPointerMove={handlePreviewPointerMove}
                    onPointerUp={handlePreviewPointerUp}
                    onClick={(e) => handlePreviewClick(i, e.currentTarget)}
                  >
                    <div style={{ textAlign: cssAlign(col.align), fontSize: 12, lineHeight: "13px", whiteSpace: "nowrap" }}>
                      {col.isTwo ? col.label : `${col.label} ${col.value}`}
                    </div>
                    {col.isTwo && (
                      <div style={{ textAlign: cssAlign(col.alignRow2), fontSize: 12, lineHeight: "13px", whiteSpace: "nowrap" }}>
                        {col.value}
                      </div>
                    )}
                  </div>
                </Fragment>
              ))}
            </div>
          )}
        </div>

        {/* ── Popover for preview item settings ── */}
        {popover && (() => {
          const col = layout.columns[popover.colIdx];
          if (!col) return null;
          const item = col.item;
          const isPlatform = item.item_type === "platform";
          // Position popover below the anchor
          const anchorCenter = popover.rect.left + popover.rect.width / 2;
          const anchorBottom = popover.rect.bottom + 8; // 8px gap + preview padding
          return (
            <div
              style={{ position: "fixed", top: anchorBottom, left: anchorCenter, transform: "translateX(-50%)", zIndex: 100 }}
              onClick={(e) => e.stopPropagation()}
            >
              {/* Arrow */}
              <div style={{
                width: 0, height: 0, margin: "0 auto",
                borderLeft: "6px solid transparent", borderRight: "6px solid transparent",
                borderBottom: "6px solid var(--glass-bg, rgba(255,255,255,0.12))",
              }} />
              <div className="glass-surface" style={{
                padding: 12, minWidth: 220, borderRadius: 10,
                backdropFilter: "blur(20px)", border: "1px solid var(--glass-border, rgba(255,255,255,0.08))",
              }}>
                <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 8 }}>
                  {isPlatform ? platformName(item.platform_id) : t("tray.todayStats", "今日统计")}
                </div>

                {/* Color */}
                <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 8 }}>
                  <span style={{ fontSize: 11, color: "var(--text-secondary)", width: 40 }}>{t("tray.color", "颜色")}</span>
                  {PRESET_COLORS.map((c) => (
                    <button key={c.value} title={c.value}
                      style={{
                        width: 18, height: 18, borderRadius: "50%", border: item.color.value === c.cssVar || (item.color.mode !== "custom" && c.value === "follow") ? "2px solid var(--accent)" : "1px solid var(--glass-border)",
                        background: c.value === "follow" ? "var(--text-primary)" : c.cssVar,
                        cursor: "pointer", padding: 0,
                      }}
                      onClick={() => {
                        const idx = config.items.indexOf(item);
                        if (idx >= 0) updateItem(idx, { color: { mode: c.value === "follow" ? "follow" as const : "custom" as const, value: c.value === "follow" ? "" : c.cssVar } });
                      }}
                    />
                  ))}
                </div>

                {/* Line mode */}
                <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 8 }}>
                  <span style={{ fontSize: 11, color: "var(--text-secondary)", width: 40 }}>{t("tray.lineMode", "行")}</span>
                  {(["single", "two"] as const).map((m) => (
                    <button key={m}
                      className={item.line_mode === m ? "accent-btn" : ""}
                      style={{
                        fontSize: 11, padding: "2px 8px", borderRadius: 4, cursor: "pointer",
                        border: item.line_mode === m ? "none" : "1px solid var(--glass-border)",
                        background: item.line_mode === m ? "var(--accent)" : "transparent",
                        color: item.line_mode === m ? "#fff" : "var(--text-secondary)",
                      }}
                      onClick={() => {
                        const idx = config.items.indexOf(item);
                        if (idx >= 0) updateItem(idx, { line_mode: m });
                      }}
                    >
                      {m === "single" ? t("tray.singleLine", "单行") : t("tray.twoLine", "两行")}
                    </button>
                  ))}
                </div>

                {/* Alignment */}
                <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 8 }}>
                  <span style={{ fontSize: 11, color: "var(--text-secondary)", width: 40 }}>{t("tray.align", "对齐")}</span>
                  {(["left", "center", "right"] as const).map((a) => (
                    <button key={a}
                      style={{
                        fontSize: 11, padding: "2px 8px", borderRadius: 4, cursor: "pointer",
                        border: item.align === a ? "none" : "1px solid var(--glass-border)",
                        background: item.align === a ? "var(--accent)" : "transparent",
                        color: item.align === a ? "#fff" : "var(--text-secondary)",
                      }}
                      onClick={() => {
                        const idx = config.items.indexOf(item);
                        if (idx >= 0) updateItem(idx, { align: a });
                      }}
                    >
                      {a === "left" ? "←" : a === "center" ? "↔" : "→"}
                    </button>
                  ))}
                </div>

                {/* Close */}
                <button style={{
                  fontSize: 11, color: "var(--text-tertiary)", cursor: "pointer",
                  background: "none", border: "none", padding: "2px 0", marginTop: 4,
                }} onClick={() => setPopover(null)}>
                  {t("common.close", "关闭")}
                </button>
              </div>
            </div>
          );
        })()}
      </div>

      {/* ── Items List ── */}
      <div style={{ display: "flex", flexDirection: "column", gap: 0 }}>
        {config.items.length === 0 && (
          <div className="glass-surface" style={{ padding: "24px 20px", textAlign: "center", color: "var(--text-tertiary)", fontSize: 13 }}>
            {t("tray.noItems", "暂无展示项，点击下方按钮添加")}
          </div>
        )}

        {config.items.map((item, i) => {
          const isSep = item.item_type === "separator";
          const isExpanded = expandedIdx === i;
          const isDragging = drag?.from === i;
          const isDragTarget = drag?.to === i && drag?.from !== i;
          const isPlatform = item.item_type === "platform";
          const riskyHex = item.color.mode === "custom" && isRiskyHex(item.color.value);

          const summary = isSep
            ? t("tray.separatorItem", "分隔符")
            : isPlatform
              ? item.display === "coding" ? t("tray.displayCoding", "Coding") : t("tray.displayBalance", "余额")
              : TODAY_METRICS.find((m) => m.value === (item.metric || "tokens"))?.label ?? "Tokens";

          // Ghost card at insertion target: grayscale preview of the dragged item
          const draggedItem = drag ? config.items[drag.from] : null;
          const draggedSummary = draggedItem
            ? draggedItem.item_type === "separator"
              ? t("tray.separatorItem", "分隔符")
              : draggedItem.item_type === "platform"
                ? draggedItem.display === "coding" ? t("tray.displayCoding", "Coding") : t("tray.displayBalance", "余额")
                : TODAY_METRICS.find((m) => m.value === (draggedItem.metric || "tokens"))?.label ?? "Tokens"
            : "";
          const draggedName = draggedItem
            ? draggedItem.item_type === "separator"
              ? `${t("tray.separatorItem", "分隔符")} "${draggedItem.display || "·"}"`
              : draggedItem.item_type === "platform"
                ? platformName(draggedItem.platform_id)
                : `${t("tray.todayUsage", "今日消耗")} (${TODAY_METRICS.find((m) => m.value === (draggedItem.metric || "tokens"))?.label ?? "Tokens"})`
            : "";

          return (
            <Fragment key={`${item.item_type}-${item.platform_id ?? "x"}-${item.metric ?? "s"}-${i}`}>
              {/* Ghost card: grayscale preview of dragged item at insertion point */}
              {drag && isDragTarget && drag.from !== i && (
                <div style={{
                  display: "flex", alignItems: "center", gap: 8, paddingLeft: 40,
                  padding: "6px 12px", margin: "2px 0", borderRadius: 8,
                  background: "var(--glass-bg, rgba(255,255,255,0.06))",
                  border: "1.5px dashed var(--accent)",
                  opacity: 0.6, filter: "grayscale(0.8)",
                  fontSize: 13, color: "var(--text-secondary)",
                  pointerEvents: "none", transition: "all 150ms ease",
                }}>
                  <span style={{ fontWeight: 600 }}>{draggedName}</span>
                  <span className="badge badge-muted" style={{ fontSize: 10 }}>{draggedSummary}</span>
                </div>
              )}

              <div
                data-tray-item
                className={`card-item${isDragging ? " is-dragging" : ""}`}
                style={{
                  position: "relative", display: "flex", flexDirection: "column", gap: 0,
                  // Drag: dragged item hidden, others gray out
                  opacity: drag
                    ? isDragging ? 0
                    : 0.4
                    : item.enabled ? 1 : 0.5,
                  paddingLeft: 40, transition: "all 200ms ease",
                }}
              >
                <div className={`drag-handle${drag?.from === i ? " is-active" : ""}`}
                  onPointerDown={(e) => handlePointerDown(e, i)}
                  onPointerMove={handlePointerMove}
                  onPointerUp={handlePointerUp}
                >{gripSvg}</div>

                <div
                  style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer", userSelect: "none" }}
                  onClick={() => { if (!didDragRef.current) setExpandedIdx(isExpanded ? null : i); }}
                >
                  <span style={{ fontSize: 13, fontWeight: 600, flex: 1 }}>
                    {isSep
                      ? `${t("tray.separatorItem", "分隔符")} "${item.display || "·"}"`
                      : isPlatform
                        ? platformName(item.platform_id)
                        : `${t("tray.todayUsage", "今日消耗")} (${TODAY_METRICS.find((m) => m.value === (item.metric || "tokens"))?.label ?? "Tokens"})`}
                  </span>
                  <span className="badge badge-muted" style={{ fontSize: 10 }}>{summary}</span>
                  {item.line_mode === "two" && <span className="badge badge-accent" style={{ fontSize: 10 }}>{t("tray.lineModeTwo", "两行")}</span>}
                  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"
                    style={{ transition: "transform 200ms ease", transform: isExpanded ? "rotate(180deg)" : "rotate(0deg)", flexShrink: 0 }}>
                    <path d="M3.5 5.25L7 8.75L10.5 5.25" />
                  </svg>
                  <div className={`toggle ${item.enabled ? "active" : ""}`}
                    onClick={(e) => { e.stopPropagation(); updateItem(i, { enabled: !item.enabled }); }}
                    role="switch" aria-checked={item.enabled} tabIndex={0} style={{ width: 32, height: 18, flexShrink: 0 }}
                  />
                  <button className="btn btn-ghost btn-icon"
                    style={{ fontSize: 12, color: "var(--danger, #ff453a)", width: 24, height: 24, padding: 0, flexShrink: 0 }}
                    onClick={(e) => { e.stopPropagation(); removeItem(i); }}>×</button>
                </div>

                {isExpanded && isSep && (
                  <div style={{ marginTop: 10, paddingTop: 10, borderTop: "1px solid var(--border)", display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}
                    onClick={(e) => e.stopPropagation()}>
                    <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.separatorChar", "分隔符")}</label>
                    <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                      {PRESET_SEPARATORS.map((s) => (
                        <button key={s.value} className="btn btn-ghost"
                          style={{ padding: "3px 10px", fontSize: 13, borderRadius: 0, minWidth: 28, background: item.display === s.value ? "var(--accent)" : "transparent", color: item.display === s.value ? "#fff" : "var(--text-secondary)" }}
                          onClick={() => updateItem(i, { display: s.value })}>{s.label}</button>
                      ))}
                    </div>
                    <input className="input" type="text" value={item.display} placeholder="自定义"
                      onChange={(e) => updateItem(i, { display: e.target.value })} style={{ width: 60, fontSize: 12, padding: "3px 8px" }} />
                  </div>
                )}

                {isExpanded && !isSep && (
                  <div style={{ marginTop: 10, paddingTop: 10, borderTop: "1px solid var(--border)", display: "flex", gap: 16, alignItems: "flex-start", flexWrap: "wrap" }}
                    onClick={(e) => e.stopPropagation()}>
                    {isPlatform && (
                      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                        <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.display", "展示")}</label>
                        <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                          {(["balance", "coding"] as const).map((d) => (
                            <button key={d} className="btn btn-ghost"
                              style={{ padding: "3px 10px", fontSize: 11, borderRadius: 0, background: item.display === d ? "var(--accent)" : "transparent", color: item.display === d ? "#fff" : "var(--text-secondary)" }}
                              onClick={() => updateItem(i, { display: d })}>
                              {d === "balance" ? t("tray.displayBalance", "余额") : t("tray.displayCoding", "Coding")}
                            </button>
                          ))}
                        </div>
                      </div>
                    )}
                    {!isPlatform && (
                      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                        <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.metric", "指标")}</label>
                        <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                          {TODAY_METRICS.map((m) => (
                            <button key={m.value} className="btn btn-ghost"
                              style={{ padding: "3px 8px", fontSize: 11, borderRadius: 0, background: (item.metric || "tokens") === m.value ? "var(--accent)" : "transparent", color: (item.metric || "tokens") === m.value ? "#fff" : "var(--text-secondary)" }}
                              onClick={() => updateItem(i, { metric: m.value })}>{m.label}</button>
                          ))}
                        </div>
                      </div>
                    )}
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.lineMode", "行模式")}</label>
                      <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                        {(["single", "two"] as const).map((lm) => (
                          <button key={lm} className="btn btn-ghost"
                            style={{ padding: "3px 10px", fontSize: 11, borderRadius: 0, background: item.line_mode === lm ? "var(--accent)" : "transparent", color: item.line_mode === lm ? "#fff" : "var(--text-secondary)" }}
                            onClick={() => updateItem(i, { line_mode: lm })}>
                            {lm === "single" ? t("tray.lineModeSingle", "单行") : t("tray.lineModeTwo", "两行")}</button>
                        ))}
                      </div>
                    </div>
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.align", "对齐")}</label>
                      <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                        {ALIGN_OPTIONS.map((a) => (
                          <button key={a.value} className="btn btn-ghost"
                            style={{ padding: "3px 8px", fontSize: 12, borderRadius: 0, background: item.align === a.value ? "var(--accent)" : "transparent", color: item.align === a.value ? "#fff" : "var(--text-secondary)" }}
                            onClick={() => updateItem(i, { align: a.value })}>{a.label}</button>
                        ))}
                      </div>
                    </div>
                    {item.line_mode === "two" && (
                      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                        <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.alignRow2", "值行对齐")}</label>
                        <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                          {ALIGN_OPTIONS.map((a) => (
                            <button key={a.value} className="btn btn-ghost"
                              style={{ padding: "3px 8px", fontSize: 12, borderRadius: 0, background: (item.align_row2 || item.align) === a.value ? "var(--accent)" : "transparent", color: (item.align_row2 || item.align) === a.value ? "#fff" : "var(--text-secondary)" }}
                              onClick={() => updateItem(i, { align_row2: a.value })}>{a.label}</button>
                          ))}
                        </div>
                      </div>
                    )}
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.fontSize", "字号")}</label>
                      <input className="input" type="number" min={6} max={20} value={item.font_size}
                        onChange={(e) => updateItem(i, { font_size: Math.max(6, Math.min(20, Number(e.target.value))) })}
                        style={{ width: 52, fontSize: 12, padding: "3px 8px" }} />
                    </div>
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.color", "颜色")}</label>
                      <select className="input" value={item.color.mode}
                        onChange={(e) => { const mode = e.target.value as TrayColor["mode"]; updateItem(i, { color: { mode, value: mode === "preset" ? PRESET_COLORS[0].value : mode === "custom" ? (item.color.value || "#ffffff") : "" } }); }}
                        style={{ width: 100, padding: "3px 8px", fontSize: 11 }}>
                        <option value="follow">{t("tray.colorFollow", "跟随系统")}</option>
                        <option value="preset">{t("tray.colorPreset", "预设色")}</option>
                        <option value="custom">{t("tray.colorCustom", "自定义")}</option>
                      </select>
                      {item.color.mode === "preset" && (
                        <select className="input" value={item.color.value} onChange={(e) => updateItem(i, { color: { mode: "preset", value: e.target.value } })} style={{ width: 80, padding: "3px 8px", fontSize: 11 }}>
                          {PRESET_COLORS.map((c) => <option key={c.value} value={c.value}>{c.value}</option>)}
                        </select>
                      )}
                      {item.color.mode === "custom" && (
                        <input type="color" value={/^#[0-9a-fA-F]{6}$/.test(item.color.value) ? item.color.value : "#ffffff"}
                          onChange={(e) => updateItem(i, { color: { mode: "custom", value: e.target.value } })}
                          style={{ width: 28, height: 22, padding: 0, border: "1px solid var(--border)", borderRadius: 4, background: "transparent" }} />
                      )}
                    </div>
                  </div>
                )}
                {isExpanded && riskyHex && (
                  <div style={{ fontSize: 11, color: "var(--warning, #ff9f0a)", marginTop: 6 }}>
                    {t("tray.colorWarning", "该颜色在部分菜单栏主题下可能不清晰")}
                  </div>
                )}
              </div>
            </Fragment>
          );
        })}
      </div>

      {/* ── Add Item ── */}
      <div style={{ position: "relative" }}>
        <button className="btn btn-primary" onClick={() => setShowAddMenu(!showAddMenu)} style={{ fontSize: 12, gap: 6 }}>
          <span style={{ fontSize: 16, lineHeight: 1 }}>+</span>
          {t("tray.addItem", "添加展示项")}
        </button>

        {showAddMenu && (
          <>
            <div style={{ position: "fixed", inset: 0, zIndex: 998 }} onClick={() => setShowAddMenu(false)} />
            <div className="glass-elevated" style={{
              position: "absolute", top: "100%", left: 0, marginTop: 6,
              minWidth: 280, padding: 8, zIndex: 999, display: "flex", flexDirection: "column", gap: 2,
            }}>
              {availablePlatforms.length > 0 && (
                <>
                  <div style={{ fontSize: 10, color: "var(--text-tertiary)", padding: "4px 12px 2px", fontWeight: 600, letterSpacing: 0.5 }}>平台</div>
                  {availablePlatforms.map((p) => (
                    <button key={p.id} className="btn btn-ghost" style={{ justifyContent: "flex-start", fontSize: 12, padding: "8px 12px" }} onClick={() => addPlatform(p.id)}>{p.name}</button>
                  ))}
                </>
              )}
              <div style={{ fontSize: 10, color: "var(--text-tertiary)", padding: "4px 12px 2px", fontWeight: 600, letterSpacing: 0.5, borderTop: "1px solid var(--border)", marginTop: 4 }}>今日统计</div>
              {TODAY_METRICS.map((m) => (
                <button key={m.value} className="btn btn-ghost" style={{ justifyContent: "flex-start", fontSize: 12, padding: "8px 12px" }} onClick={() => addTodayUsage(m.value)}>
                  {t("tray.todayUsage", "今日消耗")} — {m.label}
                </button>
              ))}
              <div style={{ fontSize: 10, color: "var(--text-tertiary)", padding: "4px 12px 2px", fontWeight: 600, letterSpacing: 0.5, borderTop: "1px solid var(--border)", marginTop: 4 }}>分隔符</div>
              <div style={{ display: "flex", gap: 2, padding: "4px 8px" }}>
                {PRESET_SEPARATORS.map((s) => (
                  <button key={s.value} className="btn btn-ghost"
                    style={{ fontSize: 14, padding: "6px 10px", minWidth: 32, textAlign: "center" }}
                    onClick={() => {
                      const items = [...config.items, makeSeparatorItem(s.value, config.items.length)];
                      persist({ ...config, items: withOrders(items) });
                      setShowAddMenu(false);
                    }}
                  >{s.label}</button>
                ))}
              </div>
            </div>
          </>
        )}
      </div>

      {message && <div className="toast">{message}</div>}
    </div>
  );
}
