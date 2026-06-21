import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  DndContext,
  PointerSensor,
  KeyboardSensor,
  useSensor,
  useSensors,
  useDroppable,
  pointerWithin,
  type DragEndEvent,
  type DragOverEvent,
  type DragStartEvent,
  DragOverlay,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  rectSortingStrategy,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  popoverConfigApi,
  groupApi,
  groupDetailApi,
  platformApi,
  type PopoverConfig,
  type PopoverItem,
  type PopoverItemType,
  type PopoverItemSize,
  type PopoverTrendScope,
  type PopoverTrendWindow,
  type RowMeta,
  type TrayColor,
  type TodayPlatformStat,
  type TodayStats,
  type Group,
  type GroupDetail,
  type Platform,
  trayConfigApi,
} from "../services/api";
import { usePolling } from "../hooks/usePolling";
import { formatNumber, formatCostUsd, formatPercent } from "../utils/formatters";
import {
  renderGrid,
  collectStatsQueries,
  type PopoverData,
  type PopoverStatsMap,
  type PopoverStatsCtx,
} from "../components/PopoverCards";
import { statsApi } from "../services/api";
import "../styles/popover.css";

/** 预定义指标集（顺序即添加菜单顺序）。 */
const ALL_ITEM_TYPES: PopoverItemType[] = [
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
const MULTI_INSTANCE_TYPES: ReadonlySet<PopoverItemType> = new Set<PopoverItemType>([
  "cost_trend", "platform_metric", "group_cost", "group_tokens", "group_requests", "group_balance",
]);

/** group_* 系列：scope 锁 "group"，配置 UI 显示分组下拉。 */
const GROUP_TYPES: ReadonlySet<PopoverItemType> = new Set<PopoverItemType>([
  "group_cost", "group_tokens", "group_requests", "group_balance",
]);

/** 指标类型 → i18n key + 默认中文标签。 */
const TYPE_LABELS: Record<PopoverItemType, { key: string; fallback: string }> = {
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

const TREND_WINDOWS: PopoverTrendWindow[] = ["today", "7d", "30d"];

const SIZE_OPTIONS: PopoverItemSize[] = ["s", "m", "l"];
const MAX_COLS = 3;

/** 颜色编辑预设（follow + 3 预设；custom 走 hex input）。 */
const COLOR_PRESETS: { mode: "follow" | "preset"; value: string; css: string }[] = [
  { mode: "follow", value: "", css: "var(--text-primary)" },
  { mode: "preset", value: "red", css: "var(--color-danger, #ff3b30)" },
  { mode: "preset", value: "green", css: "#32d74b" },
  { mode: "preset", value: "orange", css: "var(--color-warning, #ff9500)" },
];

function defaultColor(): TrayColor {
  return { mode: "follow", value: "" };
}

/** 6 位 hex 校验（容许带 #）。 */
function isValidHex(s: string): boolean {
  return /^#?[0-9a-fA-F]{6}$/.test(s.trim());
}

function makeItem(
  type: PopoverItemType,
  order: number,
  row: number,
  platforms: Platform[],
  groups: Group[],
): PopoverItem {
  const base: PopoverItem = {
    id: `popover-${type}-${Date.now()}`,
    item_type: type,
    visible: true,
    order,
    row,
    size: "m",
    color: defaultColor(),
  };
  if (type === "cost_trend") {
    return { ...base, scope: "overall", scope_ref: null, time_window: "7d" };
  }
  if (type === "platform_metric") {
    return { ...base, scope: "platform", scope_ref: platforms[0] ? String(platforms[0].id) : null, time_window: "today" };
  }
  if (GROUP_TYPES.has(type)) {
    const ref = groups[0]?.group_key ?? null;
    if (type === "group_cost") return { ...base, scope: "group", scope_ref: ref, time_window: "7d" };
    if (type === "group_balance") return { ...base, scope: "group", scope_ref: ref };
    return { ...base, scope: "group", scope_ref: ref, time_window: "today" };
  }
  return base;
}

/** effectiveRow：row 缺省回退 order（与渲染层一致）。 */
function effRow(item: PopoverItem): number {
  return item.row ?? item.order;
}

/** 把 items 规整为「按 row 升序分组、行号连续从 0、行内按 order 排」的结构，
 * 并重写 row/order 为规范值，rows[] 与之对齐。返回新 config（幂等）。 */
function normalizeConfig(items: PopoverItem[], rows: RowMeta[] | undefined): PopoverConfig {
  const map = new Map<number, PopoverItem[]>();
  for (const it of items) {
    const r = effRow(it);
    const list = map.get(r);
    if (list) list.push(it);
    else map.set(r, [it]);
  }
  const rowNums = [...map.keys()].sort((a, b) => a - b);
  const nextItems: PopoverItem[] = [];
  const nextRows: RowMeta[] = [];
  rowNums.forEach((oldRow, newRow) => {
    const list = map.get(oldRow)!.sort((a, b) => a.order - b.order);
    list.forEach((it, idx) => {
      nextItems.push({ ...it, row: newRow, order: idx });
    });
    nextRows.push({ cols: rows?.[oldRow]?.cols ?? 1 });
  });
  return { items: nextItems, rows: nextRows };
}

export function PopoverConfigTab() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<PopoverConfig>({ items: [], rows: [] });
  const [todayStats, setTodayStats] = useState<TodayStats | null>(null);
  const [platformToday, setPlatformToday] = useState<TodayPlatformStat[]>([]);
  const [groups, setGroups] = useState<Group[]>([]);
  const [groupDetails, setGroupDetails] = useState<GroupDetail[] | null>(null);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  const [showAddMenu, setShowAddMenu] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const [cfg, stats, pt, gs, ps] = await Promise.all([
          popoverConfigApi.get(),
          trayConfigApi.todayStats(),
          popoverConfigApi.platformToday(),
          groupApi.list(),
          platformApi.list(),
        ]);
        setConfig(normalizeConfig(cfg.items, cfg.rows));
        setTodayStats(stats);
        setPlatformToday(pt);
        setGroups(gs);
        setPlatforms(ps);
      } catch (e) { console.error(e); }
      setLoading(false);
    })();
    // group_balance 预览数据（独立失败兜底）。
    groupDetailApi.list().then(setGroupDetails).catch(() => setGroupDetails([]));
  }, []);

  const refreshStats = useCallback(async () => {
    try {
      const [stats, pt] = await Promise.all([
        trayConfigApi.todayStats(),
        popoverConfigApi.platformToday(),
      ]);
      setTodayStats(stats);
      setPlatformToday(pt);
    } catch { /* */ }
  }, []);
  usePolling(refreshStats, 30_000);

  // 预览统计：批量拉取当前 config 所需的全部卡数据（一次 IPC），与真实浮窗同口径。
  const [statsMap, setStatsMap] = useState<PopoverStatsMap>(new Map());
  const [statsLoaded, setStatsLoaded] = useState(false);
  const statsKey = JSON.stringify(collectStatsQueries(config));
  useEffect(() => {
    let cancelled = false;
    const { itemIds, queries } = collectStatsQueries(config);
    if (queries.length === 0) {
      setStatsMap(new Map());
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
    return () => { cancelled = true; };
    // eslint 依赖：statsKey 覆盖 config 中影响查询的字段。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [statsKey]);
  const statsCtx: PopoverStatsCtx = { map: statsMap, loaded: statsLoaded };

  const persist = async (items: PopoverItem[], rows: RowMeta[] | undefined) => {
    const next = normalizeConfig(items, rows);
    setConfig(next);
    try { await popoverConfigApi.set(next); } catch (e) { console.error(e); setMessage(String(e)); }
  };

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  // ── 行分组视图（渲染用）：row → items（已 normalize，行号连续） ──
  const rowGroups: PopoverItem[][] = (() => {
    const map = new Map<number, PopoverItem[]>();
    for (const it of config.items) {
      const r = effRow(it);
      (map.get(r) ?? map.set(r, []).get(r)!).push(it);
    }
    const nums = [...map.keys()].sort((a, b) => a - b);
    return nums.map((n) => map.get(n)!.sort((a, b) => a.order - b.order));
  })();

  const colsForRow = (row: number): number => config.rows?.[row]?.cols ?? 1;

  const toggleVisible = (id: string) => {
    const items = config.items.map((it) => (it.id === id ? { ...it, visible: !it.visible } : it));
    persist(items, config.rows);
  };

  const removeItem = (id: string) => {
    persist(config.items.filter((it) => it.id !== id), config.rows);
  };

  const updateItem = (id: string, patch: Partial<PopoverItem>) => {
    const items = config.items.map((it) => (it.id === id ? { ...it, ...patch } : it));
    persist(items, config.rows);
  };

  const addItem = (type: PopoverItemType) => {
    // 新项追加到新的一行（行号 = 当前最大行 + 1）。
    const nextRow = rowGroups.length;
    const newItem = makeItem(type, 0, nextRow, platforms, groups);
    const rows = [...(config.rows ?? []), { cols: 1 as const }];
    persist([...config.items, newItem], rows);
    setShowAddMenu(false);
  };

  const setRowCols = (row: number, cols: number) => {
    const rows = rowGroups.map((_, r) => ({ cols: (r === row ? cols : colsForRow(r)) as 1 | 2 | 3 }));
    persist(config.items, rows);
  };

  const showLayoutHint = () => {
    setMessage(t("popover.layoutHint", "提示：添加项会新建一行；拖动卡片左上角手柄可跨行/行内调整；每行可设 1-3 列。"));
  };

  // ── 二维拖拽：跨行 / 行内吸附 ──
  // 思路：单 DndContext + 每行一个 SortableContext（rectSortingStrategy）。
  // onDragOver 检测光标所在目标行，实时把 active item 迁到目标行（改 row + 重排 order）。
  // 落点判定靠 over.id（另一 item）或 row 容器 droppable id（空目标行兜底）。
  const moveItemToRow = (items: PopoverItem[], activeId: string, targetRow: number, beforeId: string | null): PopoverItem[] => {
    const active = items.find((i) => i.id === activeId);
    if (!active) return items;
    if (effRow(active) === targetRow && (beforeId === null || beforeId === activeId)) return items;
    // 重建目标行序列。
    const rest = items.filter((i) => i.id !== activeId);
    const targetItems = rest.filter((i) => effRow(i) === targetRow).sort((a, b) => a.order - b.order);
    let insertIdx = targetItems.length;
    if (beforeId) {
      const bi = targetItems.findIndex((i) => i.id === beforeId);
      if (bi >= 0) insertIdx = bi;
    }
    targetItems.splice(insertIdx, 0, { ...active, row: targetRow });
    // 写回 order。
    const updated = new Map<string, PopoverItem>();
    targetItems.forEach((it, idx) => updated.set(it.id, { ...it, row: targetRow, order: idx }));
    return items.map((i) => updated.get(i.id) ?? (i.id === activeId ? { ...i, row: targetRow } : i));
  };

  /** 从 droppable id 反推目标行号：item id → 其行；"row-N" 容器 id → N。 */
  const rowOfDroppable = (id: string): { row: number; beforeId: string | null } | null => {
    if (id.startsWith("row-")) {
      const n = Number(id.slice(4));
      return Number.isNaN(n) ? null : { row: n, beforeId: null };
    }
    const it = config.items.find((i) => i.id === id);
    if (!it) return null;
    return { row: effRow(it), beforeId: id };
  };

  const handleDragStart = (e: DragStartEvent) => setActiveId(String(e.active.id));

  const handleDragOver = (e: DragOverEvent) => {
    const { active, over } = e;
    if (!over) return;
    const target = rowOfDroppable(String(over.id));
    if (!target) return;
    const activeIdStr = String(active.id);
    const activeItem = config.items.find((i) => i.id === activeIdStr);
    if (!activeItem) return;
    // 仅跨行时实时迁移（行内排序交给 dragEnd 处理，避免抖动）。
    if (effRow(activeItem) !== target.row) {
      const items = moveItemToRow(config.items, activeIdStr, target.row, target.beforeId);
      setConfig(normalizeConfig(items, config.rows));
    }
  };

  const handleDragEnd = (e: DragEndEvent) => {
    setActiveId(null);
    const { active, over } = e;
    if (!over) { void persist(config.items, config.rows); return; }
    const activeIdStr = String(active.id);
    const overIdStr = String(over.id);
    if (activeIdStr === overIdStr) { void persist(config.items, config.rows); return; }
    const target = rowOfDroppable(overIdStr);
    if (!target) { void persist(config.items, config.rows); return; }
    const items = moveItemToRow(config.items, activeIdStr, target.row, target.beforeId);
    void persist(items, config.rows);
  };

  /** 该 item 的预览值文本（与 popover 渲染对齐）。 */
  const previewValue = (type: PopoverItemType): string => {
    const s = todayStats ?? { tokens: 0, cache_rate: 0, cost: 0, total_requests: 0 };
    switch (type) {
      case "proxy_status": return t("popover.previewStatusLine", "Running :port");
      case "platform_balance": return t("popover.previewTrayCols", "（来自托盘配置的列）");
      case "today_cost": return formatCostUsd(s.cost);
      case "today_cache_rate": return formatPercent(s.cache_rate, 0);
      case "today_tokens": return `${formatNumber(s.tokens)} tok`;
      case "platform_today":
        return platformToday.length === 0
          ? t("popover.noUsageToday", "今日暂无用量")
          : t("popover.previewPlatformCount", "{{count}} 个平台", { count: platformToday.length });
      case "cost_trend":
        return t("popover.previewTrend", "消费曲线（{{window}}）", {
          window: t(`popover.trendWindow_${"7d"}`, "近 7 天"),
        });
      case "platform_metric":
        return t("popover.itemPlatformMetric", "指定平台指标");
      case "group_cost":
        return t("popover.itemGroupCost", "分组金额");
      case "group_tokens":
        return t("popover.itemGroupTokens", "分组今日Token");
      case "group_requests":
        return t("popover.itemGroupRequests", "分组今日请求");
      case "group_balance":
        return t("popover.itemGroupBalance", "分组余额");
    }
  };

  /** cost_trend / platform_metric / group_* 配置摘要（标题副行展示 scope + 时间窗）。 */
  const trendSummary = (item: PopoverItem): string => {
    const scope = item.scope ?? "overall";
    let scopeLabel: string;
    if (scope === "platform") {
      const p = platforms.find((x) => String(x.id) === item.scope_ref);
      scopeLabel = p?.name || t("popover.trendScopePlatform", "平台");
    } else if (scope === "group") {
      const g = groups.find((x) => x.group_key === item.scope_ref);
      scopeLabel = g?.name || t("popover.trendScopeGroup", "分组");
    } else {
      scopeLabel = t("popover.trendScopeOverall", "整体");
    }
    if (item.item_type === "group_balance") return scopeLabel;
    const win = item.time_window ?? "7d";
    const winLabel = t(
      `popover.trendWindow_${win}`,
      win === "today" ? "今日" : win === "30d" ? "近 30 天" : "近 7 天",
    );
    return `${scopeLabel} · ${winLabel}`;
  };

  if (loading) {
    return <div className="text-secondary" style={{ fontSize: 13, padding: 20 }}>{t("common.loading", "加载中...")}</div>;
  }

  const usedTypes = new Set(config.items.map((i) => i.item_type));
  const availableTypes = ALL_ITEM_TYPES.filter(
    (ty) => MULTI_INSTANCE_TYPES.has(ty) || !usedTypes.has(ty),
  );

  const activeItem = activeId ? config.items.find((i) => i.id === activeId) ?? null : null;

  // 实时预览数据：用 draft config + 已轮询 stats 合成 PopoverData（与真实浮窗共用 renderGrid）。
  const previewData: PopoverData = {
    config,
    entries: [], // platform_balance 余额行来自托盘配置，预览不可得，此处留空（卡片自隐）。
    today_stats: todayStats ?? { tokens: 0, input_tokens: 0, output_tokens: 0, cache_tokens: 0, cache_rate: 0, cost: 0, total_requests: 0 },
    platform_today: platformToday,
    proxy_running: true,
    proxy_port: 0,
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* 说明 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("popover.title", "浮窗展示")}</div>
        <div className="text-secondary" style={{ fontSize: 12 }}>
          {t("popover.descGrid", "托盘浮窗内容，可显隐、二维拖拽布局、设每行列数、每卡尺寸与颜色。")}
        </div>
      </div>

      {/* 展示项布局编辑器 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("popover.items", "展示项")}</div>
          <div style={{ position: "relative" }}>
            <button
              className="btn btn-ghost"
              style={{ fontSize: 12, padding: "4px 10px" }}
              disabled={availableTypes.length === 0}
              onClick={() => setShowAddMenu((v) => !v)}
            >
              + {t("popover.addItem", "添加项")}
            </button>
            {showAddMenu && availableTypes.length > 0 && (
              <div className="glass-surface" style={{
                position: "absolute", top: "100%", right: 0, marginTop: 4, zIndex: 50,
                minWidth: 160, padding: 6, borderRadius: 10, display: "flex", flexDirection: "column", gap: 2,
              }}>
                {availableTypes.map((ty) => (
                  <button
                    key={ty}
                    className="btn btn-ghost"
                    style={{ fontSize: 12, padding: "6px 10px", justifyContent: "flex-start", textAlign: "left" }}
                    onClick={() => addItem(ty)}
                  >
                    {t(TYPE_LABELS[ty].key, TYPE_LABELS[ty].fallback)}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        {rowGroups.length === 0 ? (
          <div className="text-tertiary" style={{ fontSize: 12, fontStyle: "italic", padding: "8px 0" }}>
            {t("popover.empty", "暂无展示项，点击「添加项」")}
          </div>
        ) : (
          <DndContext
            sensors={sensors}
            collisionDetection={pointerWithin}
            onDragStart={handleDragStart}
            onDragOver={handleDragOver}
            onDragEnd={handleDragEnd}
          >
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {rowGroups.map((items, row) => (
                <RowContainer
                  key={`row-${row}`}
                  row={row}
                  cols={colsForRow(row)}
                  items={items}
                  onSetCols={(c) => setRowCols(row, c)}
                >
                  {items.map((item) => (
                    <SortableCard key={item.id} item={item}>
                      <CardEditor
                        item={item}
                        t={t}
                        platforms={platforms}
                        groups={groups}
                        summary={
                          item.item_type === "cost_trend" || item.item_type === "platform_metric" || GROUP_TYPES.has(item.item_type)
                            ? trendSummary(item)
                            : previewValue(item.item_type)
                        }
                        onToggleVisible={() => toggleVisible(item.id)}
                        onRemove={() => removeItem(item.id)}
                        onUpdate={(patch) => updateItem(item.id, patch)}
                      />
                    </SortableCard>
                  ))}
                </RowContainer>
              ))}
            </div>
            <DragOverlay>
              {activeItem ? (
                <div style={{
                  padding: "8px 10px", borderRadius: 8, fontSize: 13, fontWeight: 500,
                  background: "var(--bg-floating, var(--bg-glass))", border: "1px solid var(--accent)",
                  boxShadow: "var(--shadow-lg)",
                }}>
                  {t(TYPE_LABELS[activeItem.item_type].key, TYPE_LABELS[activeItem.item_type].fallback)}
                </div>
              ) : null}
            </DragOverlay>
          </DndContext>
        )}

        <button
          className="btn btn-ghost"
          style={{ fontSize: 11, padding: "2px 8px", alignSelf: "flex-start", color: "var(--text-tertiary)" }}
          onClick={showLayoutHint}
        >
          {t("popover.rowHintBtn", "布局说明")}
        </button>
      </div>

      {/* 实时预览（draft state，即改即见；与真实浮窗共用 renderGrid） */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("popover.preview", "实时预览")}</div>
        <div className="text-secondary" style={{ fontSize: 11 }}>
          {t("popover.previewHint", "下方按当前布局即时渲染浮窗外观，无需保存。")}
        </div>
        <div style={{ display: "flex", justifyContent: "center", padding: "8px 0" }}>
          {rowGroups.length === 0 ? (
            <div className="text-tertiary" style={{ fontSize: 12, fontStyle: "italic" }}>
              {t("popover.empty", "暂无展示项，点击「添加项」")}
            </div>
          ) : (
            <div className="popover-root" style={{ margin: 0 }}>
              {renderGrid(config, previewData, groups, groupDetails, t, statsCtx)}
            </div>
          )}
        </div>
      </div>

      {message && <div className="text-secondary" style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{message}</div>}
    </div>
  );
}

// ── 行容器（droppable）：列数选择 + 该行 grid 子项 ──
function RowContainer({
  row, cols, items, onSetCols, children,
}: {
  row: number;
  cols: number;
  items: PopoverItem[];
  onSetCols: (c: number) => void;
  children: React.ReactNode;
}) {
  const { t } = useTranslation();
  const { setNodeRef, isOver } = useDroppable({ id: `row-${row}` });
  return (
    <div
      ref={setNodeRef}
      style={{
        border: `1px solid ${isOver ? "var(--accent)" : "var(--border)"}`,
        borderRadius: 10, padding: 8,
        background: isOver ? "var(--bg-glass)" : "transparent",
        display: "flex", flexDirection: "column", gap: 8,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
          {t("popover.rowLabel", "第 {{n}} 行", { n: row + 1 })}
        </span>
        <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>{t("popover.cols", "列数")}</span>
        {[1, 2, 3].map((c) => (
          <button
            key={c}
            style={{
              fontSize: 11, padding: "2px 8px", borderRadius: 4, cursor: "pointer",
              border: cols === c ? "none" : "1px solid var(--glass-border)",
              background: cols === c ? "var(--accent)" : "transparent",
              color: cols === c ? "#fff" : "var(--text-secondary)",
            }}
            onClick={() => onSetCols(c)}
          >
            {c}
          </button>
        ))}
      </div>
      <SortableContext items={items.map((i) => i.id)} strategy={rectSortingStrategy}>
        <div style={{
          display: "grid",
          gridTemplateColumns: `repeat(${Math.min(cols, MAX_COLS)}, minmax(0, 1fr))`,
          gap: 8,
        }}>
          {children}
        </div>
      </SortableContext>
    </div>
  );
}

const gripSvg = (
  <svg width="12" height="18" viewBox="0 0 14 20" fill="currentColor">
    <circle cx="4" cy="3" r="1.8" /><circle cx="4" cy="10" r="1.8" /><circle cx="4" cy="17" r="1.8" />
    <circle cx="10" cy="3" r="1.8" /><circle cx="10" cy="10" r="1.8" /><circle cx="10" cy="17" r="1.8" />
  </svg>
);

// ── 单卡（sortable wrapper）：左上角手柄拖拽（PointerSensor，不依赖 WKWebView HTML5 DnD）──
function SortableCard({ item, children }: { item: PopoverItem; children: React.ReactNode }) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({ id: item.id });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.4 : item.visible ? 1 : 0.5,
    zIndex: isDragging ? 10 : undefined,
    position: "relative",
  };
  return (
    <div ref={setNodeRef} style={style}>
      {children}
      <span
        ref={setActivatorNodeRef}
        {...attributes}
        {...listeners}
        style={{
          position: "absolute", top: 6, left: 6, cursor: "grab",
          color: "var(--text-tertiary)", display: "inline-flex", touchAction: "none",
        }}
      >
        {gripSvg}
      </span>
    </div>
  );
}

// ── 卡片编辑体（标题 / 摘要 / 显隐 / 删除 / scope 配置 / 尺寸 / 颜色）──
function CardEditor({
  item, t, platforms, groups, summary, onToggleVisible, onRemove, onUpdate,
}: {
  item: PopoverItem;
  t: (k: string, d: string, o?: Record<string, unknown>) => string;
  platforms: Platform[];
  groups: Group[];
  summary: string;
  onToggleVisible: () => void;
  onRemove: () => void;
  onUpdate: (patch: Partial<PopoverItem>) => void;
}) {
  const color = item.color ?? defaultColor();
  const size = item.size ?? "m";
  return (
    <div style={{
      display: "flex", flexDirection: "column", gap: 8,
      padding: "8px 10px 8px 22px", borderRadius: 8,
      background: "var(--bg-glass)", border: "1px solid var(--border)",
      height: "100%", boxSizing: "border-box",
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 13, fontWeight: 500 }}>
            {t(TYPE_LABELS[item.item_type].key, TYPE_LABELS[item.item_type].fallback)}
          </div>
          <div className="text-tertiary" style={{ fontSize: 11, marginTop: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {summary}
          </div>
        </div>
        <div
          className={`toggle ${item.visible ? "active" : ""}`}
          onClick={onToggleVisible}
          role="switch"
          aria-checked={item.visible}
          tabIndex={0}
          title={t("popover.toggleVisible", "显隐")}
        />
        <button
          className="btn btn-ghost"
          style={{ fontSize: 12, padding: "2px 8px", color: "var(--status-error, #ff3b30)" }}
          onClick={onRemove}
          title={t("common.delete", "删除")}
        >
          ✕
        </button>
      </div>

      {/* scope 配置（cost_trend / platform_metric / group_*） */}
      <ScopeConfig item={item} t={t} platforms={platforms} groups={groups} onUpdate={onUpdate} />

      {/* 尺寸选择 */}
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
        <span style={{ fontSize: 11, color: "var(--text-secondary)", width: 36 }}>{t("popover.size", "尺寸")}</span>
        {SIZE_OPTIONS.map((s) => (
          <button
            key={s}
            style={{
              fontSize: 11, padding: "2px 8px", borderRadius: 4, cursor: "pointer",
              border: size === s ? "none" : "1px solid var(--glass-border)",
              background: size === s ? "var(--accent)" : "transparent",
              color: size === s ? "#fff" : "var(--text-secondary)",
            }}
            onClick={() => onUpdate({ size: s })}
            title={t(`popover.size_${s}`, s === "s" ? "小" : s === "l" ? "大" : "中")}
          >
            {s.toUpperCase()}
          </button>
        ))}
      </div>

      {/* 颜色编辑 */}
      <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
        <span style={{ fontSize: 11, color: "var(--text-secondary)", width: 36 }}>{t("popover.color", "颜色")}</span>
        {COLOR_PRESETS.map((c) => {
          const selected = c.mode === "follow"
            ? color.mode === "follow"
            : color.mode === "preset" && color.value === c.value;
          return (
            <button
              key={`${c.mode}-${c.value}`}
              title={c.mode === "follow" ? t("popover.colorFollow", "跟随") : c.value}
              style={{
                width: 18, height: 18, borderRadius: "50%", padding: 0, cursor: "pointer",
                border: selected ? "2px solid var(--accent)" : "1px solid var(--glass-border)",
                background: c.css,
              }}
              onClick={() => onUpdate({ color: { mode: c.mode, value: c.value } })}
            />
          );
        })}
        {/* custom hex */}
        <CustomHexInput
          value={color.mode === "custom" ? color.value : ""}
          active={color.mode === "custom"}
          onChange={(hex) => onUpdate({ color: { mode: "custom", value: hex } })}
          t={t}
        />
      </div>
    </div>
  );
}

function CustomHexInput({
  value, active, onChange, t,
}: {
  value: string;
  active: boolean;
  onChange: (hex: string) => void;
  t: (k: string, d: string) => string;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => { setDraft(value); }, [value]);
  const valid = draft === "" || isValidHex(draft);
  return (
    <input
      className="input"
      placeholder={t("popover.colorHex", "#RRGGBB")}
      value={draft}
      onChange={(e) => {
        const v = e.target.value;
        setDraft(v);
        if (isValidHex(v)) onChange(v.replace(/^#/, ""));
      }}
      style={{
        fontSize: 11, width: 80, padding: "2px 6px",
        border: active ? "1px solid var(--accent)" : valid ? "1px solid var(--glass-border)" : "1px solid var(--status-error, #ff3b30)",
      }}
    />
  );
}

// ── scope / 时间窗配置（按 type 分支）──
function ScopeConfig({
  item, t, platforms, groups, onUpdate,
}: {
  item: PopoverItem;
  t: (k: string, d: string, o?: Record<string, unknown>) => string;
  platforms: Platform[];
  groups: Group[];
  onUpdate: (patch: Partial<PopoverItem>) => void;
}) {
  if (item.item_type === "cost_trend") {
    return (
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
        <select
          className="input"
          style={{ fontSize: 12, width: "auto", minWidth: 90 }}
          value={item.scope ?? "overall"}
          onChange={(e) => {
            const scope = e.target.value as PopoverTrendScope;
            onUpdate({
              scope,
              scope_ref: scope === "overall" ? null
                : scope === "platform" ? (platforms[0] ? String(platforms[0].id) : null)
                : (groups[0]?.group_key ?? null),
            });
          }}
        >
          <option value="overall">{t("popover.trendScopeOverall", "整体")}</option>
          <option value="group">{t("popover.trendScopeGroup", "分组")}</option>
          <option value="platform">{t("popover.trendScopePlatform", "平台")}</option>
        </select>
        {item.scope === "group" && (
          <GroupSelect item={item} groups={groups} t={t} onUpdate={onUpdate} />
        )}
        {item.scope === "platform" && (
          <PlatformSelect item={item} platforms={platforms} t={t} onUpdate={onUpdate} />
        )}
        <WindowSelect item={item} t={t} onUpdate={onUpdate} def="7d" />
      </div>
    );
  }
  if (item.item_type === "platform_metric") {
    return (
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
        <PlatformSelect item={item} platforms={platforms} t={t} onUpdate={onUpdate} />
        <WindowSelect item={item} t={t} onUpdate={onUpdate} def="today" />
      </div>
    );
  }
  if (GROUP_TYPES.has(item.item_type)) {
    return (
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
        <GroupSelect item={item} groups={groups} t={t} onUpdate={onUpdate} />
        {item.item_type === "group_cost" && <WindowSelect item={item} t={t} onUpdate={onUpdate} def="7d" />}
      </div>
    );
  }
  return null;
}

function GroupSelect({ item, groups, t, onUpdate }: { item: PopoverItem; groups: Group[]; t: (k: string, d: string) => string; onUpdate: (p: Partial<PopoverItem>) => void }) {
  return (
    <select
      className="input"
      style={{ fontSize: 12, width: "auto", minWidth: 110 }}
      value={item.scope_ref ?? ""}
      onChange={(e) => onUpdate({ scope_ref: e.target.value || null })}
    >
      {groups.length === 0 && <option value="">{t("popover.trendNoGroup", "无分组")}</option>}
      {groups.map((g) => (
        <option key={g.group_key} value={g.group_key}>{g.name}</option>
      ))}
    </select>
  );
}

function PlatformSelect({ item, platforms, t, onUpdate }: { item: PopoverItem; platforms: Platform[]; t: (k: string, d: string) => string; onUpdate: (p: Partial<PopoverItem>) => void }) {
  return (
    <select
      className="input"
      style={{ fontSize: 12, width: "auto", minWidth: 110 }}
      value={item.scope_ref ?? ""}
      onChange={(e) => onUpdate({ scope_ref: e.target.value || null })}
    >
      {platforms.length === 0 && <option value="">{t("popover.trendNoPlatform", "无平台")}</option>}
      {platforms.map((p) => (
        <option key={p.id} value={String(p.id)}>{p.name}</option>
      ))}
    </select>
  );
}

function WindowSelect({ item, t, onUpdate, def }: { item: PopoverItem; t: (k: string, d: string) => string; onUpdate: (p: Partial<PopoverItem>) => void; def: PopoverTrendWindow }) {
  return (
    <select
      className="input"
      style={{ fontSize: 12, width: "auto", minWidth: 90 }}
      value={item.time_window ?? def}
      onChange={(e) => onUpdate({ time_window: e.target.value as PopoverTrendWindow })}
    >
      {TREND_WINDOWS.map((w) => (
        <option key={w} value={w}>
          {t(`popover.trendWindow_${w}`, w === "today" ? "今日" : w === "30d" ? "近 30 天" : "近 7 天")}
        </option>
      ))}
    </select>
  );
}
