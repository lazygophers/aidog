import { useState, useEffect, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  platformApi,
  trayConfigApi,
  type Platform,
  type TrayConfig,
  type TrayItem,
  type TodayStats,
  onProxyLogUpdated,
} from "../services/api";
import { Button } from "@/components/ui/button";
import { useReveal } from "../components/shared";

/** 菜单栏最多画几段（与 Rust `TRAY_MAX_SEGMENTS` 一致，票 I15）。 */
export const MAX_SEGMENTS = 3;

// ponytail: 卡片包装 — 每实例独立 useReveal (React 规则禁 map 内条件 hook)。
function TraySectionCard({
  staggerMs,
  style,
  children,
}: {
  staggerMs: number;
  style?: React.CSSProperties;
  children: React.ReactNode;
}) {
  const { ref, shown } = useReveal<HTMLDivElement>(staggerMs);
  return (
    <div ref={ref} className={`glass-surface hover-lift reveal${shown ? " in" : ""}`} style={style}>
      {children}
    </div>
  );
}

/** 去除尾部多余的零：0.111000 → 0.111, 10.10100 → 10.101, 0.000 → 0 */
function trimZeros(s: string): string {
  if (!s.includes(".")) return s;
  return s.replace(/\.?0+$/, "");
}

/** 一个可挑选的展示段。key 是 UI 侧标识，不入库；入库的是 TrayItem。 */
export interface SegmentOption {
  key: string;
  item: TrayItem;
  label: string;
}

function makeItem(patch: Partial<TrayItem>): TrayItem {
  return {
    item_type: "today_usage", platform_id: null, display: "", metric: null, label: null,
    decimals: null, color: { mode: "follow", value: "" }, font_size: 9, line_mode: "single",
    align: "left", align_row2: null, enabled: true, order: 0,
    ...patch,
  };
}

/** 一个已存配置项对应的段 key（用于与候选清单对上号）。 */
export function segmentKey(item: TrayItem): string {
  switch (item.item_type) {
    case "today_usage": return `today_usage:${item.metric || "tokens"}`;
    case "platform": return `platform:${item.platform_id}`;
    default: return item.item_type;
  }
}

/** 候选段清单：今日统计 4 项 + 当前命中平台 + 高峰指示 + 每个已启用平台。 */
export function segmentOptions(platforms: Platform[], t: TFunction): SegmentOption[] {
  const metrics: [string, string, string][] = [
    ["cost", "tray.metric.cost", "花费"],
    ["tokens", "tray.metric.tokens", "Tokens"],
    ["requests", "tray.metric.requests", "请求"],
    ["cache_rate", "tray.metric.cache_rate", "Cache%"],
  ];
  return [
    ...metrics.map(([m, key, fallback]) => ({
      key: `today_usage:${m}`,
      item: makeItem({ item_type: "today_usage", metric: m }),
      label: `${t("tray.todayUsage", "今日消耗")} — ${t(key, fallback)}`,
    })),
    {
      key: "routed_platform",
      item: makeItem({ item_type: "routed_platform" }),
      label: t("tray.segment.routed", "当前命中平台"),
    },
    {
      key: "peak",
      item: makeItem({ item_type: "peak" }),
      label: t("tray.segment.peak", "高峰指示"),
    },
    ...platforms.map((p) => ({
      key: `platform:${p.id}`,
      item: makeItem({ item_type: "platform", platform_id: p.id, display: "balance" }),
      label: p.name,
    })),
  ];
}

/** 预览值。routed_platform / peak 由后端按实时路由算，前端只占位。 */
export function previewValue(
  item: TrayItem,
  platforms: Platform[],
  stats: TodayStats | null,
): string {
  const s = stats ?? { tokens: 0, cache_rate: 0, cost: 0, total_requests: 0 };
  switch (item.item_type) {
    case "today_usage":
      switch (item.metric || "tokens") {
        case "cost": return `$${trimZeros(s.cost.toFixed(item.decimals ?? 5))}`;
        case "requests": return `${s.total_requests}`;
        case "cache_rate": return `${s.cache_rate.toFixed(0)}%`;
        default: return `${s.tokens} tok`;
      }
    case "platform": {
      const p = platforms.find((pp) => pp.id === item.platform_id);
      if (!p) return "--.--";
      return `$${trimZeros(p.est_balance_remaining.toFixed(2))}`;
    }
    default:
      return "—";
  }
}

/**
 * 勾选 / 取消一个段，返回新的 items。
 *
 * 票 I15 的迁移规则在这里同样成立：**只翻 enabled，不删项**。
 * 取消勾选 → 该项留在 items 里（enabled=false），原有的 label / 颜色 / 行模式全保留，
 * 再勾回来还是老样子。已满 3 段时继续勾 → 原样返回（调用方把按钮置灰）。
 */
export function toggleSegment(items: TrayItem[], opt: SegmentOption): TrayItem[] {
  const idx = items.findIndex((it) => segmentKey(it) === opt.key);
  const enabledCount = items.filter((it) => it.enabled).length;
  let next: TrayItem[];
  if (idx >= 0 && items[idx].enabled) {
    next = items.map((it, i) => (i === idx ? { ...it, enabled: false } : it));
  } else if (enabledCount >= MAX_SEGMENTS) {
    return items;
  } else if (idx >= 0) {
    next = items.map((it, i) => (i === idx ? { ...it, enabled: true } : it));
  } else {
    next = [...items, opt.item];
  }
  // enabled 的排在前面（按原相对顺序），order 按下标重写。
  const sorted = [...next.filter((it) => it.enabled), ...next.filter((it) => !it.enabled)];
  return sorted.map((it, i) => ({ ...it, order: i }));
}

export function TrayConfigTab() {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [config, setConfig] = useState<TrayConfig>({ separator: "  ", items: [] });
  const [todayStats, setTodayStats] = useState<TodayStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState("");

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

  const options = useMemo(() => segmentOptions(platforms, t), [platforms, t]);
  const selected = useMemo(
    () => config.items.filter((i) => i.enabled).sort((a, b) => a.order - b.order),
    [config],
  );
  const selectedKeys = new Set(selected.map(segmentKey));
  const full = selected.length >= MAX_SEGMENTS;
  // 迁移留下的项：关着、且不在候选清单里（旧 separator / 已删平台）。
  const keptDisabled = config.items.filter(
    (i) => !i.enabled && !options.some((o) => o.key === segmentKey(i)),
  ).length;

  if (loading) {
    return <div className="text-secondary" style={{ fontSize: 13, padding: 20 }}>{t("common.loading", "加载中...")}</div>;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, width: "100%" }}>
      {/* ── 预览：模拟菜单栏那一行 ── */}
      <TraySectionCard staggerMs={0} style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{t("tray.preview", "实时预览")}</div>
          <div style={{ fontSize: 11, color: "var(--text-secondary)" }}>
            {selected.length}/{MAX_SEGMENTS}
          </div>
        </div>
        {/* 硬编码 rgba(30,30,30) / rgba(255,255,255,*) 是 macOS 菜单栏模拟色，不跟随 app 主题，勿 token 化。 */}
        <div style={{
          background: "rgba(30, 30, 30, 0.95)", borderRadius: 8, padding: "2px 14px",
          minHeight: 26, display: "flex", alignItems: "center", gap: 12,
          fontFamily: '"Maple Mono NF", "Maple Mono", "SF Pro Text", system-ui, sans-serif',
          fontSize: 16, fontWeight: 600, color: "rgba(255,255,255,0.85)", userSelect: "none",
        }}>
          {selected.length === 0 ? (
            <span style={{ color: "rgba(255,255,255,0.35)", fontStyle: "italic" }}>
              {t("tray.previewEmpty", "暂无展示项")}
            </span>
          ) : selected.map((item) => {
            const opt = options.find((o) => o.key === segmentKey(item));
            return (
              <span key={segmentKey(item)} style={{ whiteSpace: "pre" }}>
                {item.label || opt?.label || segmentKey(item)} {previewValue(item, platforms, todayStats)}
              </span>
            );
          })}
        </div>
      </TraySectionCard>

      {/* ── 候选清单：最多挑 3 项 ── */}
      <TraySectionCard staggerMs={60} style={{ display: "flex", flexDirection: "column", gap: 0 }}>
        <div style={{ padding: "12px 16px", fontSize: 12, color: "var(--text-secondary)" }}>
          {t("tray.pickAtMost", "最多挑 3 项，菜单栏按勾选顺序显示")}
        </div>
        {options.map((opt) => {
          const on = selectedKeys.has(opt.key);
          const disabled = !on && full;
          return (
            <Button
              key={opt.key}
              variant="ghost"
              disabled={disabled}
              aria-pressed={on}
              onClick={() => persist({ ...config, items: toggleSegment(config.items, opt) })}
              style={{
                justifyContent: "flex-start", gap: 10, fontSize: 13, padding: "10px 16px",
                borderRadius: 0, opacity: disabled ? 0.4 : 1,
              }}
            >
              <span style={{
                width: 16, height: 16, borderRadius: 4, flexShrink: 0,
                border: on ? "none" : "1px solid var(--glass-border)",
                background: on ? "var(--primary)" : "transparent",
                color: "var(--primary-foreground)", fontSize: 11, lineHeight: "16px", textAlign: "center",
              }}>{on ? "✓" : ""}</span>
              <span style={{ flex: 1, textAlign: "left" }}>{opt.label}</span>
            </Button>
          );
        })}
      </TraySectionCard>

      {keptDisabled > 0 && (
        <div style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
          {t("tray.keptDisabled", "旧版托盘配置里多出的 {{count}} 项已保留但未启用，不会丢失", { count: keptDisabled })}
        </div>
      )}

      {message && <div className="toast">{message}</div>}
    </div>
  );
}
