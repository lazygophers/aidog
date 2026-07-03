// ─── 浮窗配置页 hook：state + actions + 拖拽 + 预览摘要 ──
// 自 PopoverConfigTab 主组件外迁（arch 阶段6 S5），JSX 留 PopoverLayout。
import { useState, useEffect, useCallback, type Dispatch, type SetStateAction } from "react";
import { useTranslation } from "react-i18next";
import {
  useSensor,
  useSensors,
  PointerSensor,
  KeyboardSensor,
  type DragEndEvent,
  type DragOverEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { sortableKeyboardCoordinates } from "@dnd-kit/sortable";
import {
  popoverConfigApi,
  groupApi,
  groupDetailApi,
  platformApi,
  trayConfigApi,
  statsApi,
  type PopoverConfig,
  type PopoverItem,
  type PopoverItemType,
  type TodayPlatformStat,
  type TodayStats,
  type Group,
  type GroupDetail,
  type Platform,
} from "../../services/api";
import { usePolling } from "../../hooks/usePolling";
import { formatNumber, formatCostUsd, formatPercent } from "../../utils/formatters";
import {
  collectStatsQueries,
  type PopoverStatsMap,
  type PopoverStatsCtx,
} from "../../components/PopoverCards";
import { ALL_ITEM_TYPES, MULTI_INSTANCE_TYPES } from "./constants";
import { effRow, makeItem, normalizeConfig } from "./utils";

export interface PopoverConfigData {
  t: ReturnType<typeof useTranslation>["t"];
  loading: boolean;
  message: string;
  config: PopoverConfig;
  groups: Group[];
  groupDetails: GroupDetail[] | null;
  platforms: Platform[];
  todayStats: TodayStats | null;
  platformToday: TodayPlatformStat[];
  statsCtx: PopoverStatsCtx;
  activeId: string | null;
  showAddMenu: boolean;
  setShowAddMenu: Dispatch<SetStateAction<boolean>>;
  rowGroups: PopoverItem[][];
  colsForRow: (row: number) => number;
  availableTypes: PopoverItemType[];
  activeItem: PopoverItem | null;
  sensors: ReturnType<typeof useSensors>;
  // actions
  toggleVisible: (id: string) => void;
  removeItem: (id: string) => void;
  updateItem: (id: string, patch: Partial<PopoverItem>) => void;
  addItem: (type: PopoverItemType) => void;
  setRowCols: (row: number, cols: number) => void;
  showLayoutHint: () => void;
  handleDragStart: (e: DragStartEvent) => void;
  handleDragOver: (e: DragOverEvent) => void;
  handleDragEnd: (e: DragEndEvent) => void;
  previewValue: (type: PopoverItemType) => string;
  trendSummary: (item: PopoverItem) => string;
}

export function usePopoverConfig(): PopoverConfigData {
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

  const persist = async (items: PopoverItem[], rows: PopoverConfig["rows"]) => {
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

  const usedTypes = new Set(config.items.map((i) => i.item_type));
  const availableTypes = ALL_ITEM_TYPES.filter(
    (ty) => MULTI_INSTANCE_TYPES.has(ty) || !usedTypes.has(ty),
  );

  const activeItem = activeId ? config.items.find((i) => i.id === activeId) ?? null : null;

  return {
    t,
    loading,
    message,
    config,
    groups,
    groupDetails,
    platforms,
    todayStats,
    platformToday,
    statsCtx,
    activeId,
    activeItem,
    showAddMenu,
    setShowAddMenu,
    rowGroups,
    colsForRow,
    availableTypes,
    sensors,
    toggleVisible,
    removeItem,
    updateItem,
    addItem,
    setRowCols,
    showLayoutHint,
    handleDragStart,
    handleDragOver,
    handleDragEnd,
    previewValue,
    trendSummary,
  };
}
