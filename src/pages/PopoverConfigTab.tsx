import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  popoverConfigApi,
  type PopoverConfig,
  type PopoverItem,
  type PopoverItemType,
  type TodayPlatformStat,
  type TodayStats,
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
];

/** 指标类型 → i18n key + 默认中文标签。 */
const TYPE_LABELS: Record<PopoverItemType, { key: string; fallback: string }> = {
  proxy_status: { key: "popover.itemProxyStatus", fallback: "代理状态" },
  platform_balance: { key: "popover.itemPlatformBalance", fallback: "平台余额/配额" },
  today_cost: { key: "popover.todayCost", fallback: "今日金额" },
  today_cache_rate: { key: "popover.todayCacheRate", fallback: "今日缓存率" },
  today_tokens: { key: "popover.todayTokens", fallback: "今日 Token" },
  platform_today: { key: "popover.platformToday", fallback: "各平台今日" },
};

function makeItem(type: PopoverItemType, order: number): PopoverItem {
  return { id: `popover-${type}-${Date.now()}`, item_type: type, visible: true, order };
}

type ListRow = PopoverItem & { id: string };

export function PopoverConfigTab() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<PopoverConfig>({ items: [] });
  const [todayStats, setTodayStats] = useState<TodayStats | null>(null);
  const [platformToday, setPlatformToday] = useState<TodayPlatformStat[]>([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");
  const [showAddMenu, setShowAddMenu] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const [cfg, stats, pt] = await Promise.all([
          popoverConfigApi.get(),
          trayConfigApi.todayStats(),
          popoverConfigApi.platformToday(),
        ]);
        setConfig(cfg);
        setTodayStats(stats);
        setPlatformToday(pt);
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
    }
  };

  if (loading) {
    return <div className="text-secondary" style={{ fontSize: 13, padding: 20 }}>{t("common.loading", "加载中...")}</div>;
  }

  const usedTypes = new Set(config.items.map((i) => i.item_type));
  // 单例类型（除 platform_today / 余额列允许，但简单起见允许重复添加除已存在外的所有）：
  const availableTypes = ALL_ITEM_TYPES.filter((ty) => !usedTypes.has(ty));

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
                  display: "flex", alignItems: "center", gap: 10,
                  padding: "8px 10px", borderRadius: 8,
                  background: "var(--bg-glass)", border: "1px solid var(--border)",
                  opacity: item.visible ? 1 : 0.5,
                }}>
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
                      {previewValue(item.item_type)}
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
              )}
            />
          </div>
        )}
      </div>

      {message && <div className="text-secondary" style={{ fontSize: 12, color: "var(--status-error, #ff3b30)" }}>{message}</div>}
    </div>
  );
}
