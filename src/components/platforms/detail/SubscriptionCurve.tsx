// 订阅消耗曲线（分档改版）：每档一张小卡——卡头档名 + 周期徽标 + 「重置于 {time} ·
// forecast 文案」；卡体实线轨迹 + 虚线速率投影 + 重置点。无历史时只显示当前快照 +
// 「记录随查询积累」，不伪造历史。纯渲染，数据映射在 chartMath::buildTierCurve。
import { useTranslation } from "react-i18next";
import { monotonePath, type TierCurve } from "./chartMath";
import { formatDateTime } from "../../../utils/formatters";

const W = 560, H = 120, PAD = 8;

const xScale = (t: number, ws: number, re: number) =>
  PAD + (W - 2 * PAD) * Math.max(0, Math.min(1, (t - ws) / Math.max(1, re - ws)));
const yScale = (pct: number) =>
  H - PAD - (H - 2 * PAD) * (Math.max(0, Math.min(100, pct)) / 100);

export interface TierCardData {
  name: string;
  unit: string;
  cycleMs: number | null;
  /** 重置时刻（quota resets_at 或 est 窗口推算）；null = 未知。 */
  resetAt: number | null;
  curve: TierCurve | null;
  /** 无曲线时显示的当前快照%（est 估算）。 */
  snapshotPct: number | null;
}

/** 周期毫秒 → 徽标字（5h / 7d / 30d）。 */
const cycleBadge = (cycleMs: number): string => {
  const h = cycleMs / 3_600_000;
  return h < 24 ? `${Math.round(h)}h` : `${Math.round(h / 24)}d`;
};

export function SubscriptionCurveView({ cards }: { cards: TierCardData[] }) {
  return (
    <div data-testid="detail-t3-curve" style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      {cards.map(card => (
        <TierCard key={`${card.name}|${card.unit}`} card={card} />
      ))}
    </div>
  );
}

function TierCard({ card }: { card: TierCardData }) {
  const { t } = useTranslation();
  const { name, curve, resetAt, cycleMs } = card;
  const snapshotPct = curve?.snapshotPct ?? card.snapshotPct;
  const tierLabel = t(`platform.detail.tierName.${name}`, name);
  const resetLabel = resetAt != null
    ? t("platform.detail.resetAt", "Resets {{time}}", { time: formatDateTime(resetAt) ?? "-" })
    : null;

  // 投影终点：lasts → (resetAt, pctAtReset)；提前耗尽 → (exhaustAt, 0)。
  let proj: { x: number; y: number } | null = null;
  const actual = curve?.actual ?? [];
  if (curve && actual.length >= 2 && curve.forecast && curve.resetAt > 0) {
    const last = actual[actual.length - 1];
    const first = actual[0];
    const rate = (last.pct - first.pct) / Math.max(1, last.t - first.t);
    const endT = curve.forecast.lasts ? curve.resetAt : curve.forecast.exhaustAt ?? curve.resetAt;
    const endPct = curve.forecast.lasts
      ? Math.max(0, last.pct + rate * (curve.resetAt - last.t))
      : 0;
    proj = { x: xScale(endT, curve.windowStart, curve.resetAt), y: yScale(endPct) };
  }
  const fitPts = curve
    ? actual.map(p => ({ x: xScale(p.t, curve.windowStart, curve.resetAt), y: yScale(p.pct) }))
    : [];
  const fitPath = fitPts.length >= 3 ? monotonePath(fitPts) : "";
  const fitLine = fitPts.length === 2
    ? `M ${fitPts[0].x} ${fitPts[0].y} L ${fitPts[1].x} ${fitPts[1].y}`
    : "";
  const pathD = fitPath || fitLine;
  const lastPt = fitPts[fitPts.length - 1] ?? null;

  const showPlan = curve != null && curve.resetAt > 0 && actual.length >= 2;
  const forecastText = !curve?.forecast
    ? null
    : curve.forecast.lasts
      ? t("platform.detail.curveLasts", "At this pace it will last until the reset")
      : t("platform.detail.curveExhausts", "At this pace it will run out before the reset (~{{time}})", {
          time: formatDateTime(curve.forecast.exhaustAt ?? 0) ?? "-",
        });

  return (
    <div data-testid={`detail-t3-curve-card-${name}`}
      style={{ border: "1px solid var(--border)", borderRadius: 8, padding: "8px 10px", minWidth: 0 }}>
      {/* 卡头：档名 + 周期徽标 + 重置/forecast 一行 */}
      <div style={{ display: "flex", alignItems: "baseline", gap: 8, flexWrap: "wrap", fontSize: 14 }}>
        <span style={{ fontWeight: 600 }}>{tierLabel}</span>
        {cycleMs != null && <span data-testid={`detail-t3-cycle-badge-${name}`}
          style={{ fontSize: 12, padding: "1px 6px", borderRadius: 4, border: "1px solid var(--border)", color: "var(--muted-foreground)", fontFamily: "var(--font-mono, monospace)" }}>
          {cycleBadge(cycleMs)}
        </span>}
        <span style={{ color: "var(--muted-foreground)" }}>
          {resetLabel ?? "·"}
          {forecastText && resetLabel ? " · " : ""}
          {forecastText ?? ""}
          {showPlan ? ` · ${t("platform.detail.planLine")}` : ""}
        </span>
      </div>
      {/* 卡体：有真实轨迹才画线；否则只显示当前快照 + 积累说明 */}
      {curve && curve.resetAt > 0 && actual.length >= 2 ? (
        <svg viewBox={`0 0 ${W} ${H}`} width="100%" height={H} direction="ltr" role="img"
          aria-label={tierLabel} data-testid={`detail-t3-chart-${name}`}>
          {[100, 0].map(p => (
            <line key={p} x1={PAD} y1={yScale(p)} x2={W - PAD} y2={yScale(p)}
              stroke="var(--border)" strokeWidth={1} />
          ))}
          <line data-testid={`detail-t3-plan-${name}`}
            x1={xScale(curve.windowStart, curve.windowStart, curve.resetAt)} y1={yScale(100)}
            x2={xScale(curve.resetAt, curve.windowStart, curve.resetAt)} y2={yScale(0)}
            stroke="var(--muted-foreground)" strokeWidth={1} strokeOpacity={0.5} strokeDasharray="2 5" />
          <text x={PAD + 2} y={yScale(100) + 10} fontSize={11} fill="var(--muted-foreground)" fontFamily="var(--font-mono, monospace)">100</text>
          <text x={PAD + 2} y={yScale(0) - 3} fontSize={11} fill="var(--muted-foreground)" fontFamily="var(--font-mono, monospace)">0</text>
          {/* 重置点（右端竖虚线） */}
          <line data-testid={`detail-t3-reset-line-${name}`}
            x1={xScale(curve.resetAt, curve.windowStart, curve.resetAt)} y1={PAD} x2={xScale(curve.resetAt, curve.windowStart, curve.resetAt)} y2={H - PAD}
            stroke="var(--muted-foreground)" strokeWidth={1.5} strokeDasharray="4 4" />
          {/* 实线轨迹（真实记录） */}
          {pathD && <path data-testid={`detail-t3-fit-${name}`} d={pathD}
            stroke="var(--data-primary)" strokeWidth={2.5} fill="none" strokeLinejoin="round" vectorEffect="non-scaling-stroke" />}
          {fitPts.map((p, i) => (
            <circle key={i} cx={p.x} cy={p.y} r={3} fill="var(--data-primary)" />
          ))}
          {/* 虚线投影（推算，非实测） */}
          {proj && lastPt && (
            <line data-testid={`detail-t3-forecast-${name}`}
              x1={lastPt.x} y1={lastPt.y} x2={proj.x} y2={proj.y}
              stroke="var(--muted-foreground)" strokeWidth={1.5} strokeDasharray="6 5" />
          )}
        </svg>
      ) : (
        <div data-testid={`detail-t3-snapshot-${name}`}
          style={{ fontSize: 14, color: "var(--muted-foreground)", padding: "6px 0" }}>
          {snapshotPct != null && (
            <span style={{ color: "var(--text-secondary)", marginRight: 8 }}>
              {t("platform.detail.snapshot", "Snapshot {{pct}}%", { pct: Math.round(snapshotPct) })}
            </span>
          )}
          {t("platform.detail.curveAccumulating", "History builds up as quota queries accumulate")}
        </div>
      )}
    </div>
  );
}
