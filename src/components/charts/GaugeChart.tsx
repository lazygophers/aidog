// ── 快照仪表盘（#35 批次一）：DOM+CSS conic/radial 自研（spec §A1/C4），零 canvas 零 Recharts。
// 快照态：琥珀弧 + 灰轨 + 中央 百分比/数值。趋势态（T10 / #40 接通）：卡内下方
// 迷你 sparkline（琥珀主线，fraction 序列；≥2 点画线，单点画点）。
import { type ReactNode } from "react";
import { clamp, formatPercent } from "@/utils/formatters";
import { ChartCard } from "./ChartCard";

/** 趋势态数据：at = 事件 Unix 秒，fraction = 当时占比 [0,1]（quota_snapshots 喂）。 */
export interface GaugeTrendPoint {
  at: number;
  fraction: number;
}

export interface GaugeChartProps {
  /** 当前值（如已用量）。 */
  value: number;
  /** 满值（如配额总量），默认 1。max ≤ 0 视为无数据（诚实空态）。 */
  max?: number;
  /** 中央数值展示（默认展示 value 本身）。 */
  formatValue?: (n: number) => string;
  /** 说明字（如平台名）。 */
  label?: ReactNode;
  /** 直径 px，默认 160。 */
  size?: number;
  /** 趋势态（快照态之上叠加）：≥2 点在仪表下方画 fraction sparkline。 */
  trend?: GaugeTrendPoint[];
  title?: ReactNode;
  subtitle?: ReactNode;
  emptyHint?: ReactNode;
  className?: string;
}

export function GaugeChart({
  value,
  max = 1,
  formatValue,
  label,
  size = 160,
  trend,
  title,
  subtitle,
  emptyHint,
  className,
}: GaugeChartProps) {
  if (!(max > 0)) {
    return (
      <ChartCard title={title} subtitle={subtitle} empty emptyHint={emptyHint} className={className}>{null}</ChartCard>
    );
  }

  const fraction = clamp(value / max, 0, 1);
  const fmt = formatValue ?? ((n: number) => n.toLocaleString());
  const ringWidth = Math.max(10, Math.round(size / 14));

  return (
    <ChartCard title={title} subtitle={subtitle} emptyHint={emptyHint} className={className}>
      <div
        role="meter"
        aria-valuemin={0}
        aria-valuemax={max}
        aria-valuenow={clamp(value, 0, max)}
        style={{ position: "relative", width: size, height: size, margin: "0 auto" }}
      >
        {/* 琥珀弧 + 灰轨：conic-gradient 双 stop；radial mask 抠出环孔 */}
        <div
          style={{
            position: "absolute",
            inset: 0,
            borderRadius: "50%",
            // 已填弧是数据本身，用 data-primary；界面填充色在深色下是近黑，
            // 弧与灰轨几乎同色，读不出填了多少。
            background: `conic-gradient(var(--data-primary) ${fraction * 360}deg, var(--chart-3) 0deg)`,
            maskImage: `radial-gradient(farthest-side, transparent calc(100% - ${ringWidth}px), #000 calc(100% - ${ringWidth - 1}px))`,
            WebkitMaskImage: `radial-gradient(farthest-side, transparent calc(100% - ${ringWidth}px), #000 calc(100% - ${ringWidth - 1}px))`,
          }}
        />
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: 2,
            fontVariantNumeric: "tabular-nums",
          }}
        >
          <span style={{ fontSize: size / 7, fontWeight: 600 }}>
            {formatPercent(fraction * 100, 0)}
          </span>
          <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>{fmt(value)}</span>
          {label != null && (
            <span style={{ fontSize: 10, color: "var(--text-tertiary)", maxWidth: "70%", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {label}
            </span>
          )}
        </div>
      </div>
      {trend != null && trend.length > 0 && (
        <TrendSparkline points={trend} width={size} />
      )}
    </ChartCard>
  );
}

// ── 趋势 sparkline：fraction 序列 → 等宽折线（x 按 at 归一，y 上=1 下=0）。单点 → 圆点。 ──
function TrendSparkline({ points, width }: { points: GaugeTrendPoint[]; width: number }) {
  const H = 30;
  const PAD = 2;
  const t0 = points[0].at;
  const tSpan = Math.max(1e-9, points[points.length - 1].at - t0);
  const x = (p: GaugeTrendPoint) => PAD + ((p.at - t0) / tSpan) * (width - PAD * 2);
  const y = (p: GaugeTrendPoint) => PAD + (1 - clamp(p.fraction, 0, 1)) * (H - PAD * 2);
  return (
    <svg
      data-testid="gauge-trend"
      role="img"
      width={width}
      height={H}
      style={{ display: "block", margin: "10px auto 0" }}
      shapeRendering="crispEdges"
    >
      {points.length > 1 ? (
        <polyline
          points={points.map((p) => `${x(p)},${y(p)}`).join(" ")}
          fill="none"
          // 数据线用 data-primary，不用界面填充色（深色下近黑 = 线看不见）。
          stroke="var(--data-primary)"
          strokeWidth={1.5}
          strokeLinejoin="round"
          strokeLinecap="round"
        />
      ) : (
        <circle cx={x(points[0])} cy={y(points[0])} r={2} fill="var(--data-primary)" />
      )}
    </svg>
  );
}
