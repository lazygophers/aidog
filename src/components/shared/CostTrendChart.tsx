// ── 消费趋势曲线（浮窗 cost_trend 卡片用）──
// 输入 buckets（time_bucket + total_cost），用 utils/chart.ts smoothPath 绘 SVG 曲线。
// 适配浮窗窄宽：viewBox 固定坐标系 + preserveAspectRatio=none 横向拉满，纵向固定高。
// 金额格式化统一走 formatters.ts，勿自定义。

import { useState } from "react";
import { smoothPath } from "../../utils/chart";
import { formatCostUsd } from "../../utils/formatters";
import type { StatsBucket } from "../../services/api";

export interface CostTrendChartProps {
  buckets: StatsBucket[];
}

/** 消费趋势曲线：按 total_cost(=SUM est_cost) 绘平滑曲线 + 末点/hover 金额。 */
export function CostTrendChart({ buckets }: CostTrendChartProps) {
  const [hoverIdx, setHoverIdx] = useState<number | null>(null);

  if (buckets.length === 0) {
    return null;
  }

  const W = 1000;
  const Hsvg = 100;
  const PAD_T = 10;
  const n = buckets.length;
  const plotH = Hsvg - PAD_T;
  const maxCost = Math.max(...buckets.map((b) => b.total_cost), 1e-12);
  const xAt = (i: number) => (n > 1 ? (i / (n - 1)) * W : W / 2);
  const yAt = (v: number) => PAD_T + (1 - v / maxCost) * plotH;
  const pts = buckets.map((b, i) => ({ x: xAt(i), y: yAt(b.total_cost) }));
  const linePath = smoothPath(pts, PAD_T, Hsvg);
  const areaPath =
    n > 0
      ? `${linePath} L ${pts[n - 1].x.toFixed(1)},${Hsvg} L ${pts[0].x.toFixed(1)},${Hsvg} Z`
      : "";

  const lastIdx = n - 1;
  const shownIdx = hoverIdx ?? lastIdx;
  const shownBucket = buckets[shownIdx];

  return (
    <div className="popover-trend-chart" onMouseLeave={() => setHoverIdx(null)}>
      <div className="popover-trend-value">
        {formatCostUsd(shownBucket.total_cost)}
        <span className="popover-trend-bucket">{shownBucket.time_bucket.slice(-5)}</span>
      </div>
      <svg
        viewBox={`0 0 ${W} ${Hsvg}`}
        preserveAspectRatio="none"
        className="popover-trend-svg"
      >
        <defs>
          <linearGradient id="popoverTrendArea" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.28" />
            <stop offset="100%" stopColor="var(--accent)" stopOpacity="0.02" />
          </linearGradient>
        </defs>
        <path d={areaPath} fill="url(#popoverTrendArea)" />
        <path
          d={linePath}
          fill="none"
          stroke="var(--accent)"
          strokeWidth={2}
          strokeLinejoin="round"
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
        {/* hover 命中区（每桶一竖条，透明） */}
        {pts.map((p, i) => (
          <rect
            key={i}
            x={(p.x - W / (n * 2)).toFixed(1)}
            y={0}
            width={(W / n).toFixed(1)}
            height={Hsvg}
            fill="transparent"
            onMouseEnter={() => setHoverIdx(i)}
          />
        ))}
        <circle
          cx={pts[shownIdx].x.toFixed(1)}
          cy={pts[shownIdx].y.toFixed(1)}
          r={3.5}
          fill="var(--accent)"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
    </div>
  );
}
