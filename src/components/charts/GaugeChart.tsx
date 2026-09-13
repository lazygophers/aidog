// ── 快照仪表盘（#35 批次一）：DOM+CSS conic/radial 自研（spec §A1/C4），零 canvas 零 Recharts。
// 快照态：琥珀弧 + 灰轨 + 中央 百分比/数值。趋势态接口已留（trend props），
// 趋势数据后端 T4（quota_snapshot 表）未到，暂不渲染。
import { type ReactNode } from "react";
import { clamp, formatPercent } from "@/utils/formatters";
import { ChartCard } from "./ChartCard";

/** 趋势态预留接口：at = 事件 Unix 秒，fraction = 当时占比 [0,1]（T4 落库后喂）。 */
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
  /** 趋势态预留（快照态忽略；T4 后端到位后启用）。 */
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
  trend: _trend,
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
            background: `conic-gradient(var(--primary) ${fraction * 360}deg, var(--chart-3) 0deg)`,
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
    </ChartCard>
  );
}
