import { useState, useEffect, useMemo, useCallback, useRef } from "react";
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
import { useReveal } from "../components/shared";
import {
  DndContext, KeyboardSensor, PointerSensor, closestCenter, pointerWithin,
  useDraggable, useDroppable, useSensor, useSensors,
  type CollisionDetection, type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext, horizontalListSortingStrategy, sortableKeyboardCoordinates, useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

/** 菜单栏最多画几段（与 Rust `TRAY_MAX_SEGMENTS` 一致，票 I15）。 */
export const MAX_SEGMENTS = 3;

// G3 卡面（票 01 骨架）：card 底 + 1px 边 + 小投影，入场 reveal 沿用。
const cardStyle: React.CSSProperties = {
  background: "var(--card)", border: "1px solid var(--border)",
  borderRadius: 10, boxShadow: "var(--shadow-sm)",
};
// mono 数字（spec 拍板沿用 Maple Mono，预览条同族）
const MONO = '"Maple Mono NF", "Maple Mono", "SF Mono", ui-monospace, monospace';
// 「紫的亮变体」语义位（定稿 --accent-text）：本仓已有等位 token accent-text → --accent
// （src/themes/mono.ts 映射），不新造第二份。
const ACCENT_TEXT = "var(--accent)";

// ponytail: 卡片包装 — 每实例独立 useReveal (React 规则禁 map 内条件 hook)。
function TraySectionCard({
  staggerMs,
  children,
}: {
  staggerMs: number;
  children: React.ReactNode;
}) {
  const { ref, shown } = useReveal<HTMLDivElement>(staggerMs);
  return (
    <div ref={ref} className={`reveal${shown ? " in" : ""}`} style={cardStyle}>
      {children}
    </div>
  );
}

/** 卡片节头：标题 + mono 小标签 + 右侧提示。 */
function SecHead({ title, lab, hint }: { title: string; lab: string; hint?: string }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "13px 18px", borderBottom: "1px solid var(--border)" }}>
      <div style={{ fontSize: 13.5, fontWeight: 700 }}>{title}</div>
      <div style={{ fontFamily: MONO, fontSize: 10, letterSpacing: 1.2, color: "var(--muted-foreground)", textTransform: "uppercase" }}>{lab}</div>
      {hint && <div style={{ marginLeft: "auto", fontSize: 11.5, color: "var(--muted-foreground)" }}>{hint}</div>}
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
    decimals: null, metrics: null, unit: null,
    color: { mode: "follow", value: "" }, font_size: 9, line_mode: "single",
    align: "left", align_row2: null, enabled: true, order: 0,
    ...patch,
  };
}

function codingMetrics(platform: Platform): string[] {
  try {
    const plan = JSON.parse(platform.est_coding_plan || "{}") as { tiers?: { name?: string }[] };
    return (plan.tiers ?? []).filter((tier) => tier.name).map((tier) => tier.name!);
  } catch {
    return [];
  }
}

function platformMetricLabel(metric: string, t: TFunction): string {
  if (metric.startsWith("coding:")) return `${t("tray.platform.codingRemaining", "剩余%")} (${metric.slice(7)})`;
  if (metric.startsWith("reset_time:")) return `${t("tray.platform.resetTime", "重置时间")} (${metric.slice(11)})`;
  const keys: Record<string, [string, string]> = {
    balance: ["tray.platform.balance", "余额"],
    coding: ["tray.platform.coding", "套餐剩余"],
    today_cost: ["tray.platform.todayCost", "今日花费"],
    today_tokens: ["tray.platform.todayTokens", "今日 token 数"],
  };
  const [key, fallback] = keys[metric] ?? [metric, metric];
  return t(key, fallback);
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

// 票 03：段颜色三态 → 预览实际色。preset/custom 都映射到具体色值，follow 用菜单栏前景。
const HEX_RE = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/;
const PRESET_COLORS: Record<string, string> = {
  red: "var(--destructive)", green: "var(--success)", orange: "var(--warning)",
};
const ALIGN_FLEX: Record<string, string> = { left: "flex-start", center: "center", right: "flex-end" };

export function segColor(c: TrayItem["color"]): string {
  if (c.mode === "preset") return PRESET_COLORS[c.value] ?? "var(--mb-fg-strong)";
  if (c.mode === "custom" && HEX_RE.test(c.value)) return c.value;
  return "var(--mb-fg-strong)";
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
      const money = (v: number, d: number) => {
        const n = trimZeros(v.toFixed(d));
        return item.unit ? `${n}${item.unit}` : `$${n}`;
      };
      let tiers: { name: string; est_utilization: number; window_start?: number }[] = [];
      try {
        tiers = (JSON.parse(p.est_coding_plan || "{}") as { tiers?: typeof tiers }).tiers ?? [];
      } catch { /* 旧数据无 plan */ }
      const pct = (metric: string): string | null => {
        const tier = metric === "coding" ? tiers[0] : tiers.find((x) => `coding:${x.name}` === metric);
        return tier ? `${Math.max(0, 100 - tier.est_utilization).toFixed(0)}%` : null;
      };
      // reset 时刻 = window_start + cycle（与 Rust usage_color::cycle_ms_for_tier 对称的窗口周期表）。
      const CYCLE_MS: Record<string, number> = {
        five_hour: 5 * 3_600_000,
        weekly_limit: 7 * 86_400_000, seven_day: 7 * 86_400_000,
        mcp_monthly: 30 * 86_400_000,
      };
      const resetTime = (metric: string): string | null => {
        const name = metric.slice("reset_time:".length);
        const tier = tiers.find((x) => x.name === name);
        const cycle = CYCLE_MS[name];
        if (!tier || !cycle || !tier.window_start) return null;
        const d = new Date(tier.window_start + cycle);
        const now = new Date();
        const hm = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
        return d.toDateString() === now.toDateString()
          ? hm
          : `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${hm}`;
      };
      const one = (metric: string): string | null => {
        if (metric === "balance") return money(p.est_balance_remaining, item.decimals ?? 2);
        if (metric === "today_cost") return money(s.cost, item.decimals ?? 5);
        if (metric === "today_tokens") return `${s.tokens}${item.unit ?? " tok"}`;
        if (metric.startsWith("reset_time:")) return resetTime(metric);
        return pct(metric);
      };
      const metrics = item.metrics?.length ? item.metrics : null;
      if (metrics) {
        const parts = metrics.map(one).filter(Boolean) as string[];
        return parts.length ? parts.join(" · ") : "—";
      }
      // 旧口径（无 metrics）：有 coding plan → 首窗口剩余%；否则 $余额。manual 配额预览未对齐（预存差异，后端口径为准）。
      return pct("coding") ?? money(p.est_balance_remaining, 2);
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

/** 已启用段 key 移到启用序列的 toIndex；order 按下标重写。key 未启用 → 原样返回。 */
export function moveSegment(items: TrayItem[], key: string, toIndex: number): TrayItem[] {
  const enabled = items.filter((it) => it.enabled);
  const from = enabled.findIndex((it) => segmentKey(it) === key);
  if (from < 0) return items;
  const [moved] = enabled.splice(from, 1);
  enabled.splice(Math.max(0, Math.min(toIndex, enabled.length)), 0, moved);
  return [...enabled, ...items.filter((it) => !it.enabled)].map((it, i) => ({ ...it, order: i }));
}

/** 候选库拖入装载槽：装载 opt 并插到启用序列 index。已装载 / 满槽 → 原样返回（沿用 toggleSegment 上限）。 */
export function loadSegmentAt(items: TrayItem[], opt: SegmentOption, index: number): TrayItem[] {
  if (items.some((it) => it.enabled && segmentKey(it) === opt.key)) return items;
  const next = toggleSegment(items, opt);
  return next === items ? items : moveSegment(next, opt.key, index);
}

// ── 票 03：段编辑面板 —— TrayItem 全部可编辑字段就地露出，编辑即持久化 ──
const editorInput: React.CSSProperties = {
  background: "var(--background)", border: "1px solid var(--input)", borderRadius: 6,
  padding: "5px 9px", fontSize: 12, color: "var(--foreground)", width: 90, outline: "none",
};
const editorBtn: React.CSSProperties = {
  border: "1px solid var(--input)", background: "var(--card)", borderRadius: 6,
  padding: "4px 10px", fontSize: 11.5, cursor: "pointer", color: "var(--muted-foreground)",
};
const editorBtnActive: React.CSSProperties = {
  ...editorBtn, borderColor: "var(--primary)", color: "var(--accent)", background: "var(--accent-subtle)",
};

function FieldLabel({ children }: { children: React.ReactNode }) {
  return (
    <span style={{ fontFamily: MONO, fontSize: 10, letterSpacing: 0.8, color: "var(--muted-foreground)", textTransform: "uppercase" }}>
      {children}
    </span>
  );
}

export function SegmentEditor({
  seg, t, onPatch, platform,
}: {
  seg: TrayItem;
  t: TFunction;
  onPatch: (patch: Partial<TrayItem>) => void;
  platform: Platform | null;
}) {
  const [customOn, setCustomOn] = useState(seg.color.mode === "custom");
  const [hex, setHex] = useState(seg.color.mode === "custom" ? seg.color.value : "");
  const [hexInvalid, setHexInvalid] = useState(false);
  const two = seg.line_mode === "two";
  const curRow2 = seg.align_row2 ?? seg.align;

  const patchHex = (v: string) => {
    setHex(v);
    if (HEX_RE.test(v)) {
      setHexInvalid(false);
      onPatch({ color: { mode: "custom", value: v } });
    } else {
      // 非法 hex 不落库，只标红提示
      setHexInvalid(v.length > 0);
    }
  };

  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))", gap: "12px 18px", alignItems: "start" }}>
      {/* 平台槽指标多选：勾选写入 metrics（按固定词表序）；全不勾 → metrics=null 回落旧 display 口径 */}
      {seg.item_type === "platform" && platform && (() => {
        const tiers = codingMetrics(platform);
        const words = [
          "balance",
          ...(tiers.length ? ["coding"] : []),
          ...tiers.map((n) => `coding:${n}`),
          ...tiers.map((n) => `reset_time:${n}`),
          "today_cost", "today_tokens",
        ];
        const chosen = new Set(seg.metrics ?? []);
        const toggleMetric = (m: string) => {
          const next = words.filter((w) => (w === m ? !chosen.has(w) : chosen.has(w)));
          onPatch({ metrics: next.length ? next : null });
        };
        return (
          <div style={{ gridColumn: "1 / -1", display: "flex", flexDirection: "column", gap: 5 }}>
            <FieldLabel>{t("tray.platform.metrics", "展示指标（可多选）")}</FieldLabel>
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
              {words.map((m) => (
                <button
                  key={m}
                  type="button"
                  data-testid={`tray-edit-metric-${m}`}
                  data-state={chosen.has(m) ? "active" : ""}
                  style={chosen.has(m) ? editorBtnActive : editorBtn}
                  onClick={() => toggleMetric(m)}
                >
                  {platformMetricLabel(m, t)}
                </button>
              ))}
            </div>
          </div>
        );
      })()}
      {/* 单位后缀（仅平台槽；留空回落默认） */}
      {seg.item_type === "platform" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
          <FieldLabel>{t("tray.platform.unit", "单位后缀")}</FieldLabel>
          <input
            data-testid="tray-edit-unit"
            value={seg.unit ?? ""}
            placeholder={t("tray.platform.unitPlaceholder", "留空使用默认")}
            onChange={(e) => onPatch({ unit: e.target.value || null })}
            style={editorInput}
          />
        </div>
      )}
      {/* label */}
      <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
        <FieldLabel>{t("tray.customLabel", "自定义标签")}</FieldLabel>
        <input
          data-testid="tray-edit-label"
          value={seg.label ?? ""}
          placeholder={t("tray.customLabelPlaceholder", "留空使用默认名")}
          onChange={(e) => onPatch({ label: e.target.value || null })}
          style={{ ...editorInput, width: "100%" }}
        />
      </div>
      {/* decimals */}
      <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
        <FieldLabel>{t("tray.decimals", "小数位")}</FieldLabel>
        <input
          type="number" min={0} max={5}
          data-testid="tray-edit-decimals"
          value={seg.decimals ?? 5}
          onChange={(e) => {
            const n = Number(e.target.value);
            if (!Number.isNaN(n)) onPatch({ decimals: Math.max(0, Math.min(5, Math.round(n))) });
          }}
          style={editorInput}
        />
      </div>
      {/* color 三态 */}
      <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
        <FieldLabel>{t("tray.color", "颜色")}</FieldLabel>
        <div style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
          <button
            data-testid="tray-edit-color-follow"
            data-state={seg.color.mode === "follow" ? "active" : ""}
            style={seg.color.mode === "follow" ? editorBtnActive : editorBtn}
            onClick={() => { setCustomOn(false); setHexInvalid(false); onPatch({ color: { mode: "follow", value: "" } }); }}
          >
            {t("tray.colorFollow", "跟随主题")}
          </button>
          <button
            data-testid="tray-edit-color-preset"
            data-state={seg.color.mode === "preset" ? "active" : ""}
            style={seg.color.mode === "preset" ? editorBtnActive : editorBtn}
            onClick={() => { setCustomOn(false); setHexInvalid(false); onPatch({ color: { mode: "preset", value: seg.color.mode === "preset" ? seg.color.value : "red" } }); }}
          >
            {t("tray.colorPreset", "预设")}
          </button>
          {seg.color.mode === "preset" && ["red", "green", "orange"].map((v) => (
            <button
              key={v}
              data-testid={`tray-edit-preset-${v}`}
              data-state={seg.color.value === v ? "active" : ""}
              style={seg.color.value === v ? editorBtnActive : editorBtn}
              onClick={() => onPatch({ color: { mode: "preset", value: v } })}
            >
              <span style={{ display: "inline-block", width: 10, height: 10, borderRadius: "50%", background: PRESET_COLORS[v] }} />
            </button>
          ))}
          <button
            data-testid="tray-edit-color-custom"
            data-state={seg.color.mode === "custom" ? "active" : ""}
            style={seg.color.mode === "custom" ? editorBtnActive : editorBtn}
            onClick={() => {
              setCustomOn(true);
              if (HEX_RE.test(hex)) onPatch({ color: { mode: "custom", value: hex } });
            }}
          >
            {t("tray.colorCustom", "自定义")}
          </button>
        </div>
        {customOn && (
          <div style={{ display: "flex", flexDirection: "column", gap: 3 }}>
            <input
              data-testid="tray-edit-hex"
              value={hex}
              placeholder="#RRGGBB"
              onChange={(e) => patchHex(e.target.value)}
              style={{ ...editorInput, width: 110, borderColor: hexInvalid ? "var(--destructive)" : editorInput.borderColor }}
            />
            {hexInvalid && (
              <span data-testid="tray-edit-hex-invalid" style={{ fontSize: 10.5, color: "var(--destructive)" }}>
                {t("tray.colorInvalid", "颜色格式无效，请输入如 #RRGGBB")}
              </span>
            )}
          </div>
        )}
      </div>
      {/* font_size */}
      <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
        <FieldLabel>{t("tray.fontSize", "字号")}</FieldLabel>
        <input
          type="number" min={7} max={32}
          data-testid="tray-edit-fontsize"
          value={seg.font_size}
          onChange={(e) => {
            const n = Number(e.target.value);
            if (!Number.isNaN(n) && n >= 7 && n <= 32) onPatch({ font_size: Math.round(n) });
          }}
          style={editorInput}
        />
      </div>
      {/* line_mode */}
      <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
        <FieldLabel>{t("tray.lineMode", "行模式")}</FieldLabel>
        <div style={{ display: "flex", gap: 6 }}>
          <button
            data-testid="tray-edit-line-single"
            data-state={!two ? "active" : ""}
            style={!two ? editorBtnActive : editorBtn}
            onClick={() => onPatch({ line_mode: "single" })}
          >
            {t("tray.lineModeSingle", "单行")}
          </button>
          <button
            data-testid="tray-edit-line-two"
            data-state={two ? "active" : ""}
            style={two ? editorBtnActive : editorBtn}
            onClick={() => onPatch({ line_mode: "two" })}
          >
            {t("tray.lineModeTwo", "两行")}
          </button>
        </div>
      </div>
      {/* align */}
      <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
        <FieldLabel>{t("tray.align", "对齐")}</FieldLabel>
        <div style={{ display: "flex", gap: 6 }}>
          {["left", "center", "right"].map((v) => (
            <button
              key={v}
              data-testid={`tray-edit-align-${v}`}
              data-state={seg.align === v ? "active" : ""}
              style={seg.align === v ? editorBtnActive : editorBtn}
              onClick={() => onPatch({ align: v })}
            >
              {t(`tray.align${v.charAt(0).toUpperCase()}${v.slice(1)}`, v)}
            </button>
          ))}
        </div>
      </div>
      {/* align_row2（仅两行模式） */}
      {two && (
        <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
          <FieldLabel>{t("tray.alignRow2", "值行对齐")}</FieldLabel>
          <div style={{ display: "flex", gap: 6 }}>
            {["left", "center", "right"].map((v) => (
              <button
                key={v}
                data-testid={`tray-edit-align2-${v}`}
                data-state={curRow2 === v ? "active" : ""}
                style={curRow2 === v ? editorBtnActive : editorBtn}
                onClick={() => onPatch({ align_row2: v })}
              >
                {t(`tray.align${v.charAt(0).toUpperCase()}${v.slice(1)}`, v)}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

const SLOT_LABEL: React.CSSProperties = { position: "absolute", top: -9, left: 12, fontFamily: MONO, fontSize: 9.5, background: "var(--card)", border: "1px solid var(--border)", borderRadius: 4, padding: "1px 7px", letterSpacing: 1, lineHeight: 1.4 };
const OVER_BG = "var(--accent-subtle)";

// 指针拖拽按指针落点判定；键盘拖拽无指针坐标，回退最近中心。
const collision: CollisionDetection = (args) =>
  args.pointerCoordinates ? pointerWithin(args) : closestCenter(args);

function EmptySlot({ i, tag, label }: { i: number; tag: string; label: string }) {
  const { setNodeRef, isOver } = useDroppable({ id: `empty:${i}` });
  return (
    <div ref={setNodeRef} data-testid={`tray-slot-${i}`} data-state="empty" data-over={isOver || undefined} style={{ flex: 1, border: "1.5px dashed var(--input)", background: isOver ? OVER_BG : undefined, borderRadius: 9, minHeight: 66, display: "flex", alignItems: "center", padding: "10px 14px", fontSize: 11.5, color: "var(--muted-foreground)", position: "relative" }}>
      <span style={SLOT_LABEL}>{tag}</span>
      {label}
    </div>
  );
}

function SortableSlot({ i, tag, id, onEdit, handleLabel, children }: {
  i: number; tag: string; id: string; onEdit: () => void; handleLabel: string; children: React.ReactNode;
}) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging, isOver } = useSortable({ id: `slot:${id}` });
  return (
    <div
      ref={setNodeRef}
      data-testid={`tray-slot-${i}`}
      data-state="filled"
      data-dragging={isDragging || undefined}
      onClick={onEdit}
      style={{ flex: 1, border: "1px solid var(--border)", background: isOver && !isDragging ? OVER_BG : "var(--secondary)", borderRadius: 9, minHeight: 66, display: "flex", flexDirection: "column", justifyContent: "center", gap: 3, padding: "10px 14px", position: "relative", cursor: "pointer", transform: CSS.Transform.toString(transform), transition, opacity: isDragging ? 0.6 : 1, zIndex: isDragging ? 10 : undefined }}
    >
      <span style={SLOT_LABEL}>{tag}</span>
      {/* 拖拽手柄：点槽 = 编辑，故只有手柄起拖；键盘 Space 起拖、方向键移动 */}
      <button
        type="button"
        ref={setActivatorNodeRef}
        data-testid={`tray-slot-handle-${i}`}
        aria-label={handleLabel}
        onClick={(e) => e.stopPropagation()}
        style={{ position: "absolute", top: 6, right: 8, border: "none", background: "transparent", color: "var(--muted-foreground)", cursor: "grab", fontSize: 14, lineHeight: 1, padding: 2, touchAction: "none" }}
        {...attributes}
        {...listeners}
      >
        ⠿
      </button>
      {children}
    </div>
  );
}

function DraggableChip({ id, disabled, children, ...rest }: {
  id: string; disabled: boolean; children: React.ReactNode;
} & React.HTMLAttributes<HTMLSpanElement> & { "data-testid": string; "data-state": string }) {
  const { listeners, setNodeRef, transform, isDragging } = useDraggable({ id: `chip:${id}`, disabled });
  // listeners 在前、调用方的 onKeyDown 在后覆盖：Enter/Space 仍是点选装载（键盘排序走槽手柄）
  return (
    <span
      ref={setNodeRef}
      {...listeners}
      {...rest}
      style={{ transform: CSS.Translate.toString(transform), position: "relative", zIndex: isDragging ? 10 : undefined, touchAction: "none" }}
    >
      {children}
    </span>
  );
}

function LibraryDrop({ children }: { children: React.ReactNode }) {
  const { setNodeRef, isOver } = useDroppable({ id: "library" });
  return (
    <div ref={setNodeRef} data-testid="tray-library" data-over={isOver || undefined} style={{ padding: "14px 18px 16px", display: "flex", flexDirection: "column", gap: 12, background: isOver ? OVER_BG : undefined, borderRadius: 10 }}>
      {children}
    </div>
  );
}

export function TrayConfigTab() {
  const { t } = useTranslation();
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [config, setConfig] = useState<TrayConfig>({ separator: "  ", items: [] });
  const [todayStats, setTodayStats] = useState<TodayStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingKey, setEditingKey] = useState<string | null>(null);

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

  // 勾选即存（票 I15 → 票 02 恢复）：乐观更新 + tray_config_set，失败显示错误态。
  // seq 单调守卫：连续编辑时并发 set 乱序回包，旧回包不得覆盖新状态。
  const persistSeq = useRef(0);
  const persist = useCallback(async (next: TrayConfig) => {
    setConfig(next);
    const seq = ++persistSeq.current;
    try {
      await trayConfigApi.set(next);
      if (seq === persistSeq.current) setError(null);
    } catch (e) {
      if (seq === persistSeq.current) setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const options = useMemo(() => segmentOptions(platforms, t), [platforms, t]);
  // 距离 5px 才起拖，保留点选；拖拽结果走同一条 persist（含 persistSeq 守卫）。
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  // 票 03：编辑指定段的字段，走与勾选同一条 persist 通道
  const patchItem = useCallback((key: string, patch: Partial<TrayItem>) => {
    persist({
      ...config,
      items: config.items.map((it) => (segmentKey(it) === key ? { ...it, ...patch } : it)),
    });
  }, [config, persist]);
  const selected = useMemo(
    () => config.items.filter((i) => i.enabled).sort((a, b) => a.order - b.order),
    [config],
  );
  const selectedKeys = new Set(selected.map(segmentKey));
  const full = selected.length >= MAX_SEGMENTS;
  const editingSeg = selected.find((s) => segmentKey(s) === editingKey) ?? null;
  // 迁移留下的项：关着、且不在候选清单里（旧 separator / 已删平台）。
  const keptDisabled = config.items.filter(
    (i) => !i.enabled && !options.some((o) => o.key === segmentKey(i)),
  ).length;

  const slotIndex = (overId: string) =>
    overId.startsWith("empty:") ? selected.length - 1 : selected.findIndex((s) => `slot:${segmentKey(s)}` === overId);
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over) return;
    const a = String(active.id), o = String(over.id);
    if (a.startsWith("chip:")) {
      const opt = options.find((x) => `chip:${x.key}` === a);
      if (!opt || !(o.startsWith("slot:") || o.startsWith("empty:"))) return;
      const at = o.startsWith("empty:") ? selected.length : slotIndex(o);
      const items = loadSegmentAt(config.items, opt, at);
      if (items !== config.items) persist({ ...config, items });
    } else if (a.startsWith("slot:")) {
      const key = a.slice(5);
      if (o === "library") {
        const opt = options.find((x) => x.key === key);
        if (opt) persist({ ...config, items: toggleSegment(config.items, opt) });
      } else if (o !== a) {
        const to = slotIndex(o);
        if (to >= 0) persist({ ...config, items: moveSegment(config.items, key, to) });
      }
    }
  };

  if (loading) {
    return <div className="text-secondary" style={{ fontSize: 13, padding: 20 }}>{t("common.loading", "加载中...")}</div>;
  }

  // 候选库按域分行：统计（今日 4 指标）/ 状态（命中·高峰）/ 平台余额。
  const libRows: [string, SegmentOption[]][] = [
    [t("tray.catStats", "统计"), options.filter((o) => o.key.startsWith("today_usage:"))],
    [t("tray.catStatus", "状态"), options.filter((o) => o.key === "routed_platform" || o.key === "peak")],
    [t("tray.catPlatform", "平台"), options.filter((o) => o.key.startsWith("platform:"))],
  ];

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* ── 页头：标题 + n/3 计数 ── */}
      <div style={{ display: "flex", alignItems: "flex-end", gap: 14 }}>
        <div>
          <h1 style={{ fontSize: 21, fontWeight: 800, margin: 0 }}>{t("tray.title", "托盘")}</h1>
          <div style={{ fontSize: 12.5, color: "var(--muted-foreground)", marginTop: 3 }}>
            {t("tray.pageDesc", "菜单栏仪表——选中的段按顺序实时显示，右侧同步今日观测。")}
          </div>
        </div>
        <div data-testid="tray-count" style={{ marginLeft: "auto", display: "flex", alignItems: "baseline", gap: 8, fontFamily: MONO, fontSize: 12, color: "var(--muted-foreground)" }}>
          <span style={{ fontSize: 15, fontWeight: 700, color: "var(--foreground)" }}>{selected.length}</span>
          {`/ ${MAX_SEGMENTS} ${t("tray.slotCountUnit", "段")}`}
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 280px", gap: 16, alignItems: "start" }}>
        <DndContext sensors={sensors} collisionDetection={collision} onDragEnd={onDragEnd}>
        <div style={{ display: "flex", flexDirection: "column", gap: 16, minWidth: 0 }}>
          {/* ── 卡 1：菜单栏预览 + 装载槽 ── */}
          <TraySectionCard staggerMs={0}>
            <SecHead
              title={t("tray.previewTitle", "菜单栏预览")}
              lab={t("tray.realScale", "REAL SCALE")}
              hint={t("tray.liveHint", "数值随代理日志实时刷新 · 预览条跟随系统主题")}
            />
            {/* 菜单栏模拟条：颜色全走 --mb-* 主题 token（light=浅色菜单栏 / dark=深色），双主题自动换色 */}
            <div
              data-testid="tray-menubar"
              style={{
                margin: "16px 18px 8px", background: "var(--mb-bg)", border: "1px solid var(--mb-border)",
                borderRadius: 8, height: 28, display: "flex", alignItems: "center", padding: "0 14px",
                fontSize: 12.5, color: "var(--mb-fg)",
                fontFamily: '-apple-system, "SF Pro Text", system-ui, sans-serif',
                userSelect: "none",
              }}
            >
              <div style={{ display: "flex", gap: 14, color: "var(--mb-fg-dim)", fontSize: 12 }}>
                <b style={{ color: "var(--mb-fg-strong)", fontWeight: 600 }}>⌘ AiDog</b>
                <span>{t("nav.groups", "分组")}</span>
                <span>{t("nav.logs", "日志")}</span>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 18, fontFamily: MONO, fontWeight: 600, fontSize: 12.5, marginLeft: "auto", paddingRight: 18, borderRight: "1px solid var(--mb-border)", marginRight: 16 }}>
                {selected.length === 0 ? (
                  <span style={{ color: "var(--mb-fg-dim)", fontStyle: "italic" }}>{t("tray.previewEmpty", "暂无展示项")}</span>
                ) : selected.map((seg) => {
                  const opt = options.find((o) => o.key === segmentKey(seg));
                  const two = seg.line_mode === "two";
                  return (
                    <span
                      key={segmentKey(seg)}
                      data-testid={`tray-segment-${segmentKey(seg)}`}
                      style={{
                        display: "flex", flexDirection: two ? "column" : "row",
                        alignItems: two ? (ALIGN_FLEX[seg.align] ?? "flex-start") : "center",
                        gap: two ? 1 : 5, whiteSpace: "nowrap", textAlign: seg.align as React.CSSProperties["textAlign"],
                        fontSize: seg.font_size,
                      }}
                    >
                      {seg.label || opt?.label || segmentKey(seg)}
                      <span
                        style={{
                          color: segColor(seg.color),
                          alignSelf: two ? (ALIGN_FLEX[seg.align_row2 ?? seg.align] ?? "flex-start") : undefined,
                        }}
                      >
                        {previewValue(seg, platforms, todayStats)}
                      </span>
                    </span>
                  );
                })}
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 14, opacity: 0.85 }}>
                <svg width="15" height="12" viewBox="0 0 16 12"><path d="M8 10.5 5.6 8.1a3.4 3.4 0 0 1 4.8 0ZM3.2 5.7a6.8 6.8 0 0 1 9.6 0l-1.4 1.4a4.8 4.8 0 0 0-6.8 0ZM.8 3.3a10.2 10.2 0 0 1 14.4 0l-1.4 1.4a8.2 8.2 0 0 0-11.6 0Z" fill="currentColor" /></svg>
                <svg width="22" height="11" viewBox="0 0 24 12"><rect x=".5" y=".5" width="20" height="11" rx="3" fill="none" stroke="currentColor" opacity=".5" /><rect x="2" y="2" width="13" height="8" rx="1.5" fill="currentColor" /><rect x="21.5" y="3.5" width="2" height="5" rx="1" fill="currentColor" opacity=".5" /></svg>
                <span style={{ fontFamily: MONO, fontSize: 12 }}>
                  {new Date().toLocaleDateString(undefined, { weekday: "short" })} {new Date().toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}
                </span>
              </div>
            </div>
            <div style={{ margin: "16px 18px 8px", fontFamily: MONO, fontSize: 10, letterSpacing: 1.2, color: "var(--muted-foreground)", textTransform: "uppercase" }}>
              {t("tray.slotsLabel", "装载槽")}
            </div>
            <div style={{ margin: "0 18px 18px", display: "flex", gap: 10 }}>
              <SortableContext items={selected.map((x) => `slot:${segmentKey(x)}`)} strategy={horizontalListSortingStrategy}>
                {[0, 1, 2].map((i) => {
                  const seg = selected[i];
                  if (!seg) return <EmptySlot key={i} i={i} tag={t("tray.slotIndex", "SLOT {{n}}", { n: i + 1 })} label={t("tray.slotEmpty", "空槽 — 从候选库点选装载")} />;
                  const opt = options.find((o) => o.key === segmentKey(seg));
                  return (
                    <SortableSlot key={segmentKey(seg)} i={i} tag={t("tray.slotIndex", "SLOT {{n}}", { n: i + 1 })} id={segmentKey(seg)} onEdit={() => setEditingKey(segmentKey(seg))} handleLabel={t("tray.dragHandle", "拖动排序")}>
                      <div style={{ fontSize: 12.5, fontWeight: 650 }}>{seg.label || opt?.label || segmentKey(seg)}</div>
                      <div style={{ fontFamily: MONO, fontSize: 19, fontWeight: 700, color: ACCENT_TEXT, letterSpacing: 0.3 }}>{previewValue(seg, platforms, todayStats)}</div>
                    </SortableSlot>
                  );
                })}
              </SortableContext>
            </div>
            {/* 票 03：段编辑面板 —— 点已装载槽就地展开，编辑即持久化 */}
            {editingSeg && (
              <div
                data-testid="tray-editor"
                data-editing={editingKey}
                style={{ margin: "0 18px 18px", borderTop: "1px solid var(--border)", paddingTop: 14 }}
              >
                {/* key=editingKey：切段重挂，清掉上一段的 hex/customOn 局部 state */}
                <div style={{ fontFamily: MONO, fontSize: 10, letterSpacing: 1.2, color: "var(--muted-foreground)", textTransform: "uppercase", marginBottom: 10 }}>
                  {t("tray.editTitle", "编辑段")}
                </div>
                <SegmentEditor
                  key={editingKey}
                  seg={editingSeg}
                  t={t}
                  onPatch={(p) => patchItem(editingKey!, p)}
                  platform={platforms.find((pp) => pp.id === editingSeg.platform_id) ?? null}
                />
              </div>
            )}
          </TraySectionCard>

          {/* ── 卡 2：候选库（静态壳，装载交互在票 02 接 toggleSegment + persist）── */}
          <TraySectionCard staggerMs={60}>
            <SecHead
              title={t("tray.libraryTitle", "候选库")}
              lab="LIBRARY"
              hint={t("tray.libraryHint", "最多装载 3 段 · 按装载顺序显示")}
            />
            <LibraryDrop>
              {libRows.map(([cat, rows]) => rows.length > 0 && (
                <div key={cat} style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                  <span style={{ width: 58, flexShrink: 0, fontFamily: MONO, fontSize: 10, letterSpacing: 1.2, color: "var(--muted-foreground)", textTransform: "uppercase" }}>{cat}</span>
                  {rows.map((opt) => {
                    const state = selectedKeys.has(opt.key) ? "loaded" : full ? "locked" : "";
                    return (
                      <DraggableChip
                        key={opt.key}
                        id={opt.key}
                        disabled={state !== ""}
                        className="tray-chip"
                        data-testid={`tray-chip-${opt.key}`}
                        data-state={state}
                        role="button"
                        tabIndex={0}
                        aria-pressed={state === "loaded"}
                        aria-disabled={state === "locked"}
                        onClick={() => {
                          if (state === "locked") return;
                          persist({ ...config, items: toggleSegment(config.items, opt) });
                        }}
                        onKeyDown={(e) => {
                          if (e.key !== "Enter" && e.key !== " ") return;
                          e.preventDefault();
                          if (state === "locked") return;
                          persist({ ...config, items: toggleSegment(config.items, opt) });
                        }}
                      >
                        {opt.label}
                        <span className="tray-chip-v">
                          {previewValue(opt.item, platforms, todayStats)}
                        </span>
                      </DraggableChip>
                    );
                  })}
                </div>
              ))}
            </LibraryDrop>
          </TraySectionCard>
        </div>
        </DndContext>

        {/* ── 右栏 · 实时观测 ── */}
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <TraySectionCard staggerMs={60}>
            <div style={{ padding: "14px 16px" }}>
              <div style={{ fontFamily: MONO, fontSize: 10, letterSpacing: 1.2, color: "var(--muted-foreground)", textTransform: "uppercase" }}>
                {t("tray.todayUsage", "今日消耗")}
              </div>
              <div data-testid="tray-big-cost" style={{ fontFamily: MONO, fontSize: 32, fontWeight: 700, letterSpacing: 0.5, marginTop: 4, color: ACCENT_TEXT }}>
                ${(todayStats?.cost ?? 0).toFixed(3)}
              </div>
              <div style={{ fontSize: 11, color: "var(--muted-foreground)", marginTop: 2 }}>
                {t("tray.todayCostSub", "随代理日志终态实时累加")}
              </div>
              {/* 高峰窗口数据接入在票 02+（先出徽标壳，不编造倍率/时段） */}
              <div style={{ display: "inline-flex", alignItems: "center", gap: 7, marginTop: 10, border: "1px solid color-mix(in srgb, var(--warning) 40%, transparent)", background: "color-mix(in srgb, var(--warning) 10%, transparent)", color: "var(--warning)", borderRadius: 6, padding: "5px 10px", fontSize: 11.5, fontFamily: MONO }}>
                <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--warning)" }} />
                {t("tray.peakBadge", "高峰时段")}
              </div>
            </div>
          </TraySectionCard>
          <TraySectionCard staggerMs={120}>
            <div style={{ padding: "14px 16px 6px" }}>
              <div style={{ fontFamily: MONO, fontSize: 10, letterSpacing: 1.2, color: "var(--muted-foreground)", textTransform: "uppercase" }}>
                {t("tray.todayMetrics", "今日指标")}
              </div>
              <div style={{ marginTop: 6 }}>
                {([
                  [t("tray.metric.tokens", "Tokens"), `${todayStats?.tokens ?? 0} tok`],
                  [t("tray.metric.requests", "请求"), `${todayStats?.total_requests ?? 0}`],
                  [t("tray.metric.cache_rate", "Cache%"), `${todayStats?.cache_rate ?? 0}%`],
                  [t("tray.segment.routed", "当前命中平台"), "—"],
                ] as [string, string][]).map(([k, v]) => (
                  <div key={k} style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", padding: "7px 0", borderBottom: "1px solid var(--border)", fontSize: 12, color: "var(--muted-foreground)" }}>
                    {k}
                    <b style={{ fontFamily: MONO, fontSize: 13, color: "var(--foreground)" }}>{v}</b>
                  </div>
                ))}
              </div>
            </div>
          </TraySectionCard>
          {/* 近 7 日消费趋势：壳先落地，数据接 statsApi.query 在票 02+（不画假曲线） */}
          <div style={{ ...cardStyle, padding: "4px 16px 14px" }}>
            <div style={{ fontFamily: MONO, fontSize: 10, letterSpacing: 1.2, color: "var(--muted-foreground)", textTransform: "uppercase", padding: "12px 0 6px" }}>
              {t("tray.trendTitle", "近 7 日消费趋势")}
            </div>
            <svg data-testid="tray-spark" viewBox="0 0 240 64" preserveAspectRatio="none" style={{ width: "100%", height: 64, display: "block" }}>
              <path d="M4,60 L236,60" fill="none" stroke="var(--border)" strokeWidth="1.5" strokeDasharray="4 4" />
            </svg>
          </div>
        </div>
      </div>

      {error && (
        <div data-testid="tray-error" style={{ display: "flex", gap: 8, alignItems: "center", fontSize: 11.5, color: "var(--destructive)", padding: "10px 14px", border: "1px solid color-mix(in srgb, var(--destructive) 40%, transparent)", borderRadius: 8 }}>
          {t("tray.persistError", "保存托盘配置失败：{{msg}}", { msg: error })}
        </div>
      )}

      {keptDisabled > 0 && (
        <div data-testid="tray-kept-disabled" style={{ display: "flex", gap: 8, alignItems: "center", fontSize: 11.5, color: "var(--muted-foreground)", padding: "10px 14px", border: "1px dashed var(--border)", borderRadius: 8 }}>
          <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--warning)", flexShrink: 0 }} />
          {t("tray.keptDisabled", "旧版托盘配置里多出的 {{count}} 项已保留但未启用，不会丢失", { count: keptDisabled })}
        </div>
      )}
    </div>
  );
}
