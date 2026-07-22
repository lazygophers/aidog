import { useState, useEffect, useMemo, useCallback, Fragment } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  platformApi,
  trayConfigApi,
  type Platform,
  type TrayConfig,
  type TrayItem,
  type TrayColor,
  type TodayStats,
  onProxyLogUpdated,
} from "../services/api";
import { SortableList } from "../components/SortableList";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

const PRESET_COLORS: { value: string; cssVar: string }[] = [
  { value: "follow", cssVar: "var(--text-primary)" },
  { value: "red", cssVar: "var(--color-danger)" },
  { value: "green", cssVar: "#32d74b" },
  { value: "orange", cssVar: "var(--color-warning)" },
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
    item_type: "platform", platform_id: platformId, display, metric: null, label: null, decimals: null,
    color: defaultColor(), font_size: DEFAULT_FONT_SIZE, line_mode: "two",
    align: "left", align_row2: null, enabled: true, order,
  };
}

function makeTodayUsageItem(metric: string, order: number): TrayItem {
  return {
    item_type: "today_usage", platform_id: null, display: "", metric, label: null, decimals: null,
    color: defaultColor(), font_size: DEFAULT_FONT_SIZE, line_mode: "two",
    align: "left", align_row2: null, enabled: true, order,
  };
}

function makeSeparatorItem(separator: string, order: number): TrayItem {
  return {
    item_type: "separator", platform_id: null, display: separator, metric: null, label: null, decimals: null,
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
function computeItemText(item: TrayItem, platform: Platform | undefined, todayStats: TodayStats | null, t: TFunction): { label: string; value: string } {
  if (item.item_type === "today_usage") {
    const s = todayStats ?? { tokens: 0, cache_rate: 0, cost: 0, total_requests: 0 };
    const auto = (() => {
      switch (item.metric || "tokens") {
        case "cache_rate": return { label: t("tray.metric.cache_rate", "Cache"), value: `${s.cache_rate.toFixed(0)}%` };
        case "cost": return { label: t("tray.metric.cost", "花费"), value: `$${trimZeros(s.cost.toFixed(item.decimals ?? 5))}` };
        case "requests": return { label: t("tray.metric.requests", "请求"), value: `${s.total_requests}` };
        default: return { label: t("tray.metric.today", "今日"), value: `${s.tokens} tok` };
      }
    })();
    return { label: item.label || auto.label, value: auto.value };
  }
  if (!platform) return { label: item.label || `#${item.platform_id}`, value: "--.--" };
  let isCoding = item.display === "coding";
  let util = 0;
  if (platform.est_coding_plan) {
    try {
      const p = JSON.parse(platform.est_coding_plan);
      if (p?.tiers?.length) { isCoding = true; util = p.tiers[0].est_utilization ?? 0; }
    } catch { /* */ }
  }
  const autoLabel = platform.name;
  const autoValue = isCoding ? `${Math.max(0, 100 - util).toFixed(0)}%` : `$${trimZeros(platform.est_balance_remaining.toFixed(2))}`;
  return { label: item.label || autoLabel, value: autoValue };
}

function makeMetricLabel(t: TFunction) {
  return (metric: string) => {
    switch (metric) {
      case "cache_rate": return t("tray.metric.cache_rate", "Cache%");
      case "cost": return t("tray.metric.cost", "花费$");
      case "requests": return t("tray.metric.requests", "请求");
      default: return t("tray.metric.tokens", "Tokens");
    }
  };
}

interface Column { item: TrayItem; label: string; value: string; isTwo: boolean; align: string; alignRow2: string }
interface Gap { separator: string | null; sepIndex: number | null }

/** Stable id derived from an item's index within the current snapshot.
 * dnd-kit only needs ids stable across a single drag (snapshot fixed); the
 * persisted TrayItem has no id field (backend serde contract), so we derive one. */
function itemId(index: number): string {
  return `tray-item-${index}`;
}
function colId(index: number): string {
  return `tray-col-${index}`;
}
/** TrayItem + index-derived id, used as SortableList row. */
interface ListRow { id: string; item: TrayItem; index: number }
/** Preview column + index-derived id, used as SortableList row (horizontal). */
interface ColRow extends Column { id: string; colIndex: number }

export function TrayConfigTab() {
  const { t } = useTranslation();
  const metricLabel = makeMetricLabel(t);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [config, setConfig] = useState<TrayConfig>({ separator: "  ", items: [] });
  const [todayStats, setTodayStats] = useState<TodayStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  const [expandedIdx, setExpandedIdx] = useState<number | null>(null);
  const [showAddMenu, setShowAddMenu] = useState(false);

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

  const refreshStats = useCallback(async () => {
    try { setTodayStats(await trayConfigApi.todayStats()); } catch { /* */ }
  }, []);
  // 今日统计随 proxy_log 终态写入刷新（替代 30s 轮询）。
  useEffect(() => onProxyLogUpdated(() => { refreshStats(); }, 1000), [refreshStats]);

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
        const { label, value } = computeItemText(item, p, todayStats, t);
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

  // ── Reorder via @dnd-kit (SortableList) ──
  /** Reorder the full config.items list to the new row order. */
  const reorderItems = (rows: ListRow[]) => {
    persist({ ...config, items: withOrders(rows.map((r) => r.item)) });
  };

  /** Reorder by preview columns: rebuild config.items so the column-items
   * follow the new order, while separators keep their slot among the items
   * (they occupy the same positions in config.items they did before). */
  const reorderColumns = (cols: ColRow[]) => {
    const newColItems = cols.map((c) => c.item); // reordered column items
    let ci = 0;
    const next = config.items.map((it) =>
      it.item_type === "separator" || !it.enabled ? it : newColItems[ci++],
    );
    persist({ ...config, items: withOrders(next) });
  };

  const handlePreviewClick = (colIdx: number, el: HTMLElement) => {
    if (popover?.colIdx === colIdx) { setPopover(null); return; }
    setPopover({ colIdx, rect: el.getBoundingClientRect() });
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

  // Preview columns as SortableList rows (id derived from underlying item index).
  const colRows: ColRow[] = layout.columns.map((col, ci) => ({
    ...col,
    id: colId(config.items.indexOf(col.item)),
    colIndex: ci,
  }));

  // Config items as SortableList rows (id derived from item index).
  const listRows: ListRow[] = config.items.map((item, index) => ({
    id: itemId(index),
    item,
    index,
  }));

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }} onClick={() => setPopover(null)}>
      {/* ── Preview Bar ── */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("tray.preview", "实时预览")}</div>
          <div style={{
            display: "flex", alignItems: "center", gap: 6, fontSize: 11,
            color: layout.overBudget ? "var(--color-warning)" : layout.totalLines === 2 ? "var(--accent)" : "var(--text-secondary)",
          }}>
            <span style={{ fontWeight: 600 }}>{t("tray.lineBudget", "行数")} {layout.totalLines}/2</span>
            {layout.overBudget && <span style={{ color: "var(--color-warning)" }}>{t("tray.overBudgetHint", "超限")}</span>}
          </div>
        </div>

        {/* Simulated macOS menu bar — match tray rendering precisely */}
        <div style={{
          background: "rgba(30, 30, 30, 0.95)", borderRadius: 8,
          padding: hasTwoLine ? "4px 14px" : "2px 14px",
          minHeight: hasTwoLine ? 40 : 26, display: "flex", alignItems: "center",
          fontFamily: '"Maple Mono NF", "Maple Mono", "SF Pro Text", system-ui, sans-serif',
          fontSize: hasTwoLine ? 12 : 16,
          fontWeight: 600,
          color: "rgba(255,255,255,0.85)", userSelect: "none",
          marginTop: 8,
        }}>
          {colRows.length === 0 ? (
            <span style={{ color: "rgba(255,255,255,0.35)", fontStyle: "italic" }}>
              {t("tray.previewEmpty", "暂无展示项")}
            </span>
          ) : !hasTwoLine ? (
            /* ── Single-line: columns with gaps, dnd-kit sortable + clickable ── */
            <div style={{ display: "flex", alignItems: "center", gap: 0 }}>
              <SortableList<ColRow> items={colRows} onReorder={reorderColumns}
                renderItem={(col, handle) => (
                  <span style={{ display: "inline-flex", alignItems: "center" }}>
                    {col.colIndex > 0 && (
                      <span style={{
                        display: "inline-flex", alignItems: "center", justifyContent: "center",
                        width: layout.gaps[col.colIndex - 1]?.separator ? "auto" : 5,
                        padding: layout.gaps[col.colIndex - 1]?.separator ? "0 5px" : 0,
                        fontSize: 12, color: "rgba(255,255,255,0.35)",
                      }}>
                        {layout.gaps[col.colIndex - 1]?.separator || ""}
                      </span>
                    )}
                    <span
                      ref={handle.ref}
                      {...handle.attributes}
                      {...handle.listeners}
                      style={{
                        textAlign: cssAlign(col.align), whiteSpace: "pre",
                        cursor: "grab", padding: "2px 4px", borderRadius: 4,
                        outline: popover?.colIdx === col.colIndex ? "2px solid var(--accent)" : "none",
                        touchAction: "none",
                      }}
                      onClick={(e) => { if (!handle.isDragging) handlePreviewClick(col.colIndex, e.currentTarget); }}
                    >
                      {col.label} {col.value}
                    </span>
                  </span>
                )}
              />
            </div>
          ) : (
            /* ── Two-line: grid with gaps, dnd-kit sortable + clickable ── */
            <div style={{ display: "grid", gridAutoFlow: "column", gap: 0, width: "100%" }}>
              <SortableList<ColRow> items={colRows} onReorder={reorderColumns} strategy="grid"
                renderItem={(col, handle) => (
                  <div style={{ display: "flex", alignItems: "stretch", height: "100%" }}>
                    {col.colIndex > 0 && (
                      <div style={{
                        display: "flex", alignItems: "center", justifyContent: "center",
                        width: layout.gaps[col.colIndex - 1]?.separator ? "auto" : 5,
                        padding: layout.gaps[col.colIndex - 1]?.separator ? "0 5px" : 0,
                        fontSize: 12, color: "rgba(255,255,255,0.35)", height: "100%",
                      }}>
                        {layout.gaps[col.colIndex - 1]?.separator || ""}
                      </div>
                    )}
                    <div
                      ref={handle.ref}
                      {...handle.attributes}
                      {...handle.listeners}
                      style={{
                        display: "flex", flexDirection: "column", alignItems: "stretch", gap: 5,
                        cursor: "grab", padding: "2px 4px", borderRadius: 4,
                        outline: popover?.colIdx === col.colIndex ? "2px solid var(--accent)" : "none",
                        touchAction: "none",
                      }}
                      onClick={(e) => { if (!handle.isDragging) handlePreviewClick(col.colIndex, e.currentTarget); }}
                    >
                      <div style={{ textAlign: cssAlign(col.align), fontSize: 12, lineHeight: "13px", whiteSpace: "nowrap" }}>
                        {col.isTwo ? col.label : `${col.label} ${col.value}`}
                      </div>
                      {col.isTwo && (
                        <div style={{ textAlign: cssAlign(col.alignRow2), fontSize: 16, lineHeight: "17px", whiteSpace: "nowrap" }}>
                          {col.value}
                        </div>
                      )}
                    </div>
                  </div>
                )}
              />
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
                    <Button variant="outline" key={c.value} title={c.value}
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
                    <Button variant="outline" key={m}
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
                    </Button>
                  ))}
                </div>

                {/* Alignment */}
                <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 8 }}>
                  <span style={{ fontSize: 11, color: "var(--text-secondary)", width: 40 }}>{t("tray.align", "对齐")}</span>
                  {(["left", "center", "right"] as const).map((a) => (
                    <Button variant="outline" key={a}
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
                    </Button>
                  ))}
                </div>

                {/* Close */}
                <Button variant="outline" style={{
                  fontSize: 11, color: "var(--text-tertiary)", cursor: "pointer",
                  background: "none", border: "none", padding: "2px 0", marginTop: 4,
                }} onClick={() => setPopover(null)}>
                  {t("common.close", "关闭")}
                </Button>
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

        <SortableList<ListRow> items={listRows} onReorder={reorderItems}
          renderItem={(row, handle) => {
          const item = row.item;
          const i = row.index;
          const isSep = item.item_type === "separator";
          const isExpanded = expandedIdx === i;
          const isPlatform = item.item_type === "platform";
          const riskyHex = item.color.mode === "custom" && isRiskyHex(item.color.value);

          const summary = isSep
            ? t("tray.separatorItem", "分隔符")
            : isPlatform
              ? item.display === "coding" ? t("tray.displayCoding", "Coding") : t("tray.displayBalance", "余额")
              : metricLabel(item.metric || "tokens");

          return (
            <Fragment>
              <div
                className={`card-item${handle.isDragging ? " is-dragging" : ""}`}
                style={{
                  position: "relative", display: "flex", flexDirection: "column", gap: 0,
                  opacity: item.enabled ? 1 : 0.5,
                  paddingLeft: 40, transition: "opacity 200ms ease",
                }}
              >
                <div className={`drag-handle${handle.isDragging ? " is-active" : ""}`}
                  ref={handle.ref}
                  {...handle.attributes}
                  {...handle.listeners}
                  style={{ touchAction: "none", cursor: "grab" }}
                >{gripSvg}</div>

                <div
                  style={{ display: "flex", alignItems: "center", gap: 5, cursor: "pointer", userSelect: "none" }}
                  onClick={() => { if (!handle.isDragging) setExpandedIdx(isExpanded ? null : i); }}
                >
                  <span style={{ fontSize: 13, fontWeight: 600, flex: 1 }}>
                    {isSep
                      ? `${t("tray.separatorItem", "分隔符")} "${item.display || "·"}"`
                      : isPlatform
                        ? platformName(item.platform_id)
                        : `${t("tray.todayUsage", "今日消耗")} (${metricLabel(item.metric || "tokens")})`}
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
                  <Button variant="ghost" 
                    style={{ fontSize: 12, color: "var(--danger, var(--color-danger))", width: 24, height: 24, padding: 0, flexShrink: 0 }}
                    onClick={(e) => { e.stopPropagation(); removeItem(i); }}>×</Button>
                </div>

                {isExpanded && isSep && (
                  <div style={{ marginTop: 10, paddingTop: 10, borderTop: "1px solid var(--border)", display: "flex", gap: 5, alignItems: "center", flexWrap: "wrap" }}
                    onClick={(e) => e.stopPropagation()}>
                    <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.separatorChar", "分隔符")}</label>
                    <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                      {PRESET_SEPARATORS.map((s) => (
                        <Button variant="ghost" key={s.value} 
                          style={{ padding: "3px 10px", fontSize: 13, borderRadius: 0, minWidth: 28, background: item.display === s.value ? "var(--accent)" : "transparent", color: item.display === s.value ? "#fff" : "var(--text-secondary)" }}
                          onClick={() => updateItem(i, { display: s.value })}>{s.value === " " ? t("tray.sep.space", "空格") : s.label}</Button>
                      ))}
                    </div>
                    <Input  type="text" value={item.display} placeholder={t("tray.custom", "自定义")}
                      onChange={(e) => updateItem(i, { display: e.target.value })} style={{ width: 60, fontSize: 12, padding: "3px 8px" }} />
                  </div>
                )}

                {isExpanded && !isSep && (
                  <div style={{ marginTop: 10, paddingTop: 10, borderTop: "1px solid var(--border)", display: "flex", gap: 16, alignItems: "flex-start", flexWrap: "wrap" }}
                    onClick={(e) => e.stopPropagation()}>
                    {isPlatform && (
                      <div style={{ display: "flex", gap: 5, alignItems: "center" }}>
                        <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.display", "展示")}</label>
                        <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                          {(["balance", "coding"] as const).map((d) => (
                            <Button variant="ghost" key={d} 
                              style={{ padding: "3px 10px", fontSize: 11, borderRadius: 0, background: item.display === d ? "var(--accent)" : "transparent", color: item.display === d ? "#fff" : "var(--text-secondary)" }}
                              onClick={() => updateItem(i, { display: d })}>
                              {d === "balance" ? t("tray.displayBalance", "余额") : t("tray.displayCoding", "Coding")}
                            </Button>
                          ))}
                        </div>
                      </div>
                    )}
                    {!isPlatform && (
                      <div style={{ display: "flex", gap: 5, alignItems: "center" }}>
                        <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.metric", "指标")}</label>
                        <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                          {TODAY_METRICS.map((m) => (
                            <Button variant="ghost" key={m.value} 
                              style={{ padding: "3px 8px", fontSize: 11, borderRadius: 0, background: (item.metric || "tokens") === m.value ? "var(--accent)" : "transparent", color: (item.metric || "tokens") === m.value ? "#fff" : "var(--text-secondary)" }}
                              onClick={() => updateItem(i, { metric: m.value })}>{metricLabel(m.value)}</Button>
                          ))}
                        </div>
                      </div>
                    )}
                    <div style={{ display: "flex", gap: 5, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.customLabel", "标签")}</label>
                      <Input  type="text" value={item.label || ""} placeholder={t("tray.customLabelPlaceholder", "默认")}
                        onChange={(e) => updateItem(i, { label: e.target.value || null })}
                        style={{ width: 80, fontSize: 12, padding: "3px 8px" }} />
                    </div>
                    <div style={{ display: "flex", gap: 5, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.decimals", "小数位")}</label>
                      <Input  type="number" min={0} max={10} value={item.decimals ?? 5}
                        onChange={(e) => updateItem(i, { decimals: Number(e.target.value) || null })}
                        style={{ width: 52, fontSize: 12, padding: "3px 8px" }} />
                    </div>
                    <div style={{ display: "flex", gap: 5, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.lineMode", "行模式")}</label>
                      <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                        {(["single", "two"] as const).map((lm) => (
                          <Button variant="ghost" key={lm} 
                            style={{ padding: "3px 10px", fontSize: 11, borderRadius: 0, background: item.line_mode === lm ? "var(--accent)" : "transparent", color: item.line_mode === lm ? "#fff" : "var(--text-secondary)" }}
                            onClick={() => updateItem(i, { line_mode: lm })}>
                            {lm === "single" ? t("tray.lineModeSingle", "单行") : t("tray.lineModeTwo", "两行")}</Button>
                        ))}
                      </div>
                    </div>
                    <div style={{ display: "flex", gap: 5, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.align", "对齐")}</label>
                      <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                        {ALIGN_OPTIONS.map((a) => (
                          <Button variant="ghost" key={a.value} 
                            style={{ padding: "3px 8px", fontSize: 12, borderRadius: 0, background: item.align === a.value ? "var(--accent)" : "transparent", color: item.align === a.value ? "#fff" : "var(--text-secondary)" }}
                            onClick={() => updateItem(i, { align: a.value })}>{a.label}</Button>
                        ))}
                      </div>
                    </div>
                    {item.line_mode === "two" && (
                      <div style={{ display: "flex", gap: 5, alignItems: "center" }}>
                        <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.alignRow2", "值行对齐")}</label>
                        <div style={{ display: "flex", border: "1px solid var(--border)", borderRadius: 6, overflow: "hidden" }}>
                          {ALIGN_OPTIONS.map((a) => (
                            <Button variant="ghost" key={a.value} 
                              style={{ padding: "3px 8px", fontSize: 12, borderRadius: 0, background: (item.align_row2 || item.align) === a.value ? "var(--accent)" : "transparent", color: (item.align_row2 || item.align) === a.value ? "#fff" : "var(--text-secondary)" }}
                              onClick={() => updateItem(i, { align_row2: a.value })}>{a.label}</Button>
                          ))}
                        </div>
                      </div>
                    )}
                    <div style={{ display: "flex", gap: 5, alignItems: "center" }}>
                      <label style={{ fontSize: 11, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{t("tray.color", "颜色")}</label>
                      <Select  value={item.color.mode}
                        onValueChange={(v) => { const mode = v as TrayColor["mode"]; updateItem(i, { color: { mode, value: mode === "preset" ? PRESET_COLORS[0].value : mode === "custom" ? (item.color.value || "#ffffff") : "" } }); }}
                        >
<SelectTrigger style={{ width: 100, padding: "3px 8px", fontSize: 11 }}><SelectValue/></SelectTrigger>
<SelectContent>
                        <SelectItem value="follow">{t("tray.colorFollow", "跟随系统")}</SelectItem>
                        <SelectItem value="preset">{t("tray.colorPreset", "预设色")}</SelectItem>
                        <SelectItem value="custom">{t("tray.colorCustom", "自定义")}</SelectItem>
                      </SelectContent>
</Select>
                      {item.color.mode === "preset" && (
                        <Select  value={item.color.value} onValueChange={(v) => updateItem(i, { color: { mode: "preset", value: v } })} >
<SelectTrigger style={{ width: 80, padding: "3px 8px", fontSize: 11 }}><SelectValue/></SelectTrigger>
<SelectContent>
                          {PRESET_COLORS.map((c) => <SelectItem key={c.value} value={c.value}>{c.value}</SelectItem>)}
                        </SelectContent>
</Select>
                      )}
                      {item.color.mode === "custom" && (
                        <Input type="color" value={/^#[0-9a-fA-F]{6}$/.test(item.color.value) ? item.color.value : "#ffffff"}
                          onChange={(e) => updateItem(i, { color: { mode: "custom", value: e.target.value } })}
                          style={{ width: 28, height: 22, padding: 0, border: "1px solid var(--border)", borderRadius: 4, background: "transparent" }} />
                      )}
                    </div>
                  </div>
                )}
                {isExpanded && riskyHex && (
                  <div style={{ fontSize: 11, color: "var(--warning, var(--color-warning))", marginTop: 6 }}>
                    {t("tray.colorWarning", "该颜色在部分菜单栏主题下可能不清晰")}
                  </div>
                )}
              </div>
            </Fragment>
          );
        }}
        />
      </div>

      {/* ── Add Item ── */}
      <div style={{ position: "relative" }}>
        <Button variant="default"  onClick={() => setShowAddMenu(!showAddMenu)} style={{ fontSize: 12, gap: 6 }}>
          <span style={{ fontSize: 16, lineHeight: 1 }}>+</span>
          {t("tray.addItem", "添加展示项")}
        </Button>

        {showAddMenu && (
          <>
            <div style={{ position: "fixed", inset: 0, zIndex: 998 }} onClick={() => setShowAddMenu(false)} />
            <div className="glass-elevated" style={{
              position: "absolute", top: "100%", left: 0, marginTop: 6,
              minWidth: 280, padding: 8, zIndex: 999, display: "flex", flexDirection: "column", gap: 2,
            }}>
              {availablePlatforms.length > 0 && (
                <>
                  <div style={{ fontSize: 10, color: "var(--text-tertiary)", padding: "4px 12px 2px", fontWeight: 600, letterSpacing: 0.5 }}>{t("tray.catPlatform", "平台")}</div>
                  {availablePlatforms.map((p) => (
                    <Button variant="ghost" key={p.id}  style={{ justifyContent: "flex-start", fontSize: 12, padding: "8px 12px" }} onClick={() => addPlatform(p.id)}>{p.name}</Button>
                  ))}
                </>
              )}
              <div style={{ fontSize: 10, color: "var(--text-tertiary)", padding: "4px 12px 2px", fontWeight: 600, letterSpacing: 0.5, borderTop: "1px solid var(--border)", marginTop: 4 }}>{t("tray.todayStats", "今日统计")}</div>
              {TODAY_METRICS.map((m) => (
                <Button variant="ghost" key={m.value}  style={{ justifyContent: "flex-start", fontSize: 12, padding: "8px 12px" }} onClick={() => addTodayUsage(m.value)}>
                  {t("tray.todayUsage", "今日消耗")} — {metricLabel(m.value)}
                </Button>
              ))}
              <div style={{ fontSize: 10, color: "var(--text-tertiary)", padding: "4px 12px 2px", fontWeight: 600, letterSpacing: 0.5, borderTop: "1px solid var(--border)", marginTop: 4 }}>{t("tray.separatorChar", "分隔符")}</div>
              <div style={{ display: "flex", gap: 2, padding: "4px 8px" }}>
                {PRESET_SEPARATORS.map((s) => (
                  <Button variant="ghost" key={s.value} 
                    style={{ fontSize: 14, padding: "6px 10px", minWidth: 32, textAlign: "center" }}
                    onClick={() => {
                      const items = [...config.items, makeSeparatorItem(s.value, config.items.length)];
                      persist({ ...config, items: withOrders(items) });
                      setShowAddMenu(false);
                    }}
                  >{s.value === " " ? t("tray.sep.space", "空格") : s.label}</Button>
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
