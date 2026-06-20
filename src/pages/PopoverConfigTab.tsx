import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  popoverConfigApi,
  groupApi,
  platformApi,
  type PopoverConfig,
  type PopoverItem,
  type PopoverItemType,
  type PopoverTrendScope,
  type PopoverTrendWindow,
  type TodayPlatformStat,
  type TodayStats,
  type Group,
  type Platform,
  trayConfigApi,
} from "../services/api";
import { SortableList } from "../components/SortableList";
import { usePolling } from "../hooks/usePolling";
import { formatNumber, formatCostUsd, formatPercent } from "../utils/formatters";

/** 预定义指标集（顺序即添加菜单顺序）。 */
const ALL_ITEM_TYPES: PopoverItemType[] = [
  "proxy_status",
  "platform_balance",
  "today_cost",
  "today_cache_rate",
  "today_tokens",
  "platform_today",
  "cost_trend",
];

/** 可重复添加的多实例类型（各自独立配置）。 */
const MULTI_INSTANCE_TYPES: ReadonlySet<PopoverItemType> = new Set<PopoverItemType>(["cost_trend"]);

/** 指标类型 → i18n key + 默认中文标签。 */
const TYPE_LABELS: Record<PopoverItemType, { key: string; fallback: string }> = {
  proxy_status: { key: "popover.itemProxyStatus", fallback: "代理状态" },
  platform_balance: { key: "popover.itemPlatformBalance", fallback: "平台余额/配额" },
  today_cost: { key: "popover.todayCost", fallback: "今日金额" },
  today_cache_rate: { key: "popover.todayCacheRate", fallback: "今日缓存率" },
  today_tokens: { key: "popover.todayTokens", fallback: "今日 Token" },
  platform_today: { key: "popover.platformToday", fallback: "各平台今日" },
  cost_trend: { key: "popover.itemCostTrend", fallback: "消费趋势" },
};

const TREND_WINDOWS: PopoverTrendWindow[] = ["today", "7d", "30d"];

function makeItem(type: PopoverItemType, order: number): PopoverItem {
  const base: PopoverItem = { id: `popover-${type}-${Date.now()}`, item_type: type, visible: true, order };
  if (type === "cost_trend") {
    return { ...base, scope: "overall", scope_ref: null, time_window: "7d" };
  }
  return base;
}

type ListRow = PopoverItem & { id: string };

export function PopoverConfigTab() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<PopoverConfig>({ items: [] });
  const [todayStats, setTodayStats] = useState<TodayStats | null>(null);
  const [platformToday, setPlatformToday] = useState<TodayPlatformStat[]>([]);
  const [groups, setGroups] = useState<Group[]>([]);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  const [showAddMenu, setShowAddMenu] = useState(false);

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
        setConfig(cfg);
        setTodayStats(stats);
        setPlatformToday(pt);
        setGroups(gs);
        setPlatforms(ps);
      } catch (e) { console.error(e); }
      setLoading(false);
    })();
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

  const persist = async (next: PopoverConfig) => {
    setConfig(next);
    try { await popoverConfigApi.set(next); } catch (e) { console.error(e); setMessage(String(e)); }
  };

  const withOrders = (items: PopoverItem[]): PopoverItem[] => items.map((it, idx) => ({ ...it, order: idx }));

  const sortedItems = [...config.items].sort((a, b) => a.order - b.order);

  const toggleVisible = (id: string) => {
    const items = config.items.map((it) => (it.id === id ? { ...it, visible: !it.visible } : it));
    persist({ ...config, items });
  };

  const removeItem = (id: string) => {
    persist({ ...config, items: withOrders(config.items.filter((it) => it.id !== id)) });
  };

  const updateItem = (id: string, patch: Partial<PopoverItem>) => {
    const items = config.items.map((it) => (it.id === id ? { ...it, ...patch } : it));
    persist({ ...config, items });
  };

  const addItem = (type: PopoverItemType) => {
    persist({ ...config, items: withOrders([...sortedItems, makeItem(type, sortedItems.length)]) });
    setShowAddMenu(false);
  };

  const reorder = (rows: ListRow[]) => {
    persist({ ...config, items: withOrders(rows) });
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
    }
  };

  /** cost_trend 当前配置摘要（标题副行展示 scope + 时间窗）。 */
  const trendSummary = (item: PopoverItem): string => {
    const scope = item.scope ?? "overall";
    const win = item.time_window ?? "7d";
    const winLabel = t(
      `popover.trendWindow_${win}`,
      win === "today" ? "今日" : win === "30d" ? "近 30 天" : "近 7 天",
    );
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
    return `${scopeLabel} · ${winLabel}`;
  };

  if (loading) {
    return <div className="text-secondary" style={{ fontSize: 13, padding: 20 }}>{t("common.loading", "加载中...")}</div>;
  }

  const usedTypes = new Set(config.items.map((i) => i.item_type));
  // 单例类型已添加则隐藏；多实例类型（cost_trend）始终可添加。
  const availableTypes = ALL_ITEM_TYPES.filter(
    (ty) => MULTI_INSTANCE_TYPES.has(ty) || !usedTypes.has(ty),
  );

  const gripSvg = (
    <svg width="14" height="20" viewBox="0 0 14 20" fill="currentColor">
      <circle cx="4" cy="3" r="1.8" /><circle cx="4" cy="10" r="1.8" /><circle cx="4" cy="17" r="1.8" />
      <circle cx="10" cy="3" r="1.8" /><circle cx="10" cy="10" r="1.8" /><circle cx="10" cy="17" r="1.8" />
    </svg>
  );

  const listRows: ListRow[] = sortedItems;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* 说明 */}
      <div className="glass-surface" style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{t("popover.title", "浮窗展示")}</div>
        <div className="text-secondary" style={{ fontSize: 12 }}>
          {t("popover.desc", "托盘图标左击弹出的浮窗内容，可显隐、拖拽排序、增删展示项。")}
        </div>
      </div>

      {/* 展示项列表 */}
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

        {listRows.length === 0 ? (
          <div className="text-tertiary" style={{ fontSize: 12, fontStyle: "italic", padding: "8px 0" }}>
            {t("popover.empty", "暂无展示项，点击「添加项」")}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <SortableList<ListRow> items={listRows} onReorder={reorder}
              renderItem={(item, handle) => (
                <div style={{
                  display: "flex", flexDirection: "column", gap: 8,
                  padding: "8px 10px", borderRadius: 8,
                  background: "var(--bg-glass)", border: "1px solid var(--border)",
                  opacity: item.visible ? 1 : 0.5,
                }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                    <span
                      ref={handle.ref}
                      {...handle.attributes}
                      {...handle.listeners}
                      style={{ cursor: "grab", color: "var(--text-tertiary)", display: "inline-flex", touchAction: "none" }}
                    >
                      {gripSvg}
                    </span>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: 13, fontWeight: 500 }}>
                        {t(TYPE_LABELS[item.item_type].key, TYPE_LABELS[item.item_type].fallback)}
                      </div>
                      <div className="text-tertiary" style={{ fontSize: 11, marginTop: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {item.item_type === "cost_trend" ? trendSummary(item) : previewValue(item.item_type)}
                      </div>
                    </div>
                    <div
                      className={`toggle ${item.visible ? "active" : ""}`}
                      onClick={() => toggleVisible(item.id)}
                      role="switch"
                      aria-checked={item.visible}
                      tabIndex={0}
                      title={t("popover.toggleVisible", "显隐")}
                    />
                    <button
                      className="btn btn-ghost"
                      style={{ fontSize: 12, padding: "2px 8px", color: "var(--status-error, #ff3b30)" }}
                      onClick={() => removeItem(item.id)}
                      title={t("common.delete", "删除")}
                    >
                      ✕
                    </button>
                  </div>
                  {item.item_type === "cost_trend" && (
                    <div style={{ display: "flex", flexWrap: "wrap", gap: 8, paddingLeft: 24 }}>
                      <select
                        className="input"
                        style={{ fontSize: 12, width: "auto", minWidth: 110 }}
                        value={item.scope ?? "overall"}
                        onChange={(e) => {
                          const scope = e.target.value as PopoverTrendScope;
                          updateItem(item.id, {
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
                        <select
                          className="input"
                          style={{ fontSize: 12, width: "auto", minWidth: 120 }}
                          value={item.scope_ref ?? ""}
                          onChange={(e) => updateItem(item.id, { scope_ref: e.target.value || null })}
                        >
                          {groups.length === 0 && <option value="">{t("popover.trendNoGroup", "无分组")}</option>}
                          {groups.map((g) => (
                            <option key={g.group_key} value={g.group_key}>{g.name}</option>
                          ))}
                        </select>
                      )}
                      {item.scope === "platform" && (
                        <select
                          className="input"
                          style={{ fontSize: 12, width: "auto", minWidth: 120 }}
                          value={item.scope_ref ?? ""}
                          onChange={(e) => updateItem(item.id, { scope_ref: e.target.value || null })}
                        >
                          {platforms.length === 0 && <option value="">{t("popover.trendNoPlatform", "无平台")}</option>}
                          {platforms.map((p) => (
                            <option key={p.id} value={String(p.id)}>{p.name}</option>
                          ))}
                        </select>
                      )}
                      <select
                        className="input"
                        style={{ fontSize: 12, width: "auto", minWidth: 100 }}
                        value={item.time_window ?? "7d"}
                        onChange={(e) => updateItem(item.id, { time_window: e.target.value as PopoverTrendWindow })}
                      >
                        {TREND_WINDOWS.map((w) => (
                          <option key={w} value={w}>
                            {t(`popover.trendWindow_${w}`, w === "today" ? "今日" : w === "30d" ? "近 30 天" : "近 7 天")}
                          </option>
                        ))}
                      </select>
                    </div>
                  )}
                </div>
              )}
            />
          </div>
        )}
      </div>

      {message && <div className="text-secondary" style={{ fontSize: 12, color: "var(--status-error, #ff3b30)" }}>{message}</div>}
    </div>
  );
}
