// ── 消费趋势曲线（浮窗 cost_trend 卡片用）──
// 输入 buckets（time_bucket + total_cost），用 utils/chart.ts smoothPath 绘 SVG 曲线。
// 适配浮窗窄宽：viewBox 固定坐标系 + preserveAspectRatio=none 横向拉满，纵向固定高。
// 金额格式化统一走 formatters.ts，勿自定义。

import { useMemo, useState } from "react";
import { smoothPath } from "../../utils/chart";
import { formatCostUsd } from "../../utils/formatters";
import type { StatsBucket } from "../../services/api";

export interface CostTrendChartProps {
  buckets: StatsBucket[];
}

const W = 1000;
const Hsvg = 100;
const PAD_T = 10;

/** 消费趋势曲线：按 total_cost(=SUM est_cost) 绘平滑曲线 + 末点/hover 金额。 */
export function CostTrendChart({ buckets }: CostTrendChartProps) {
  const [hoverIdx, setHoverIdx] = useState<number | null>(null);

  // pts/path 只依赖 buckets；hover 引起的 re-render（setHoverIdx）不应重算曲线几何。
  const { n, pts, linePath, areaPath } = useMemo(() => {
    const plotH = Hsvg - PAD_T;
    const bn = buckets.length;
    const maxCost = Math.max(...buckets.map((b) => b.total_cost), 1e-12);
    const xAt = (i: number) => (bn > 1 ? (i / (bn - 1)) * W : W / 2);
    const yAt = (v: number) => PAD_T + (1 - v / maxCost) * plotH;
    const bpts = buckets.map((b, i) => ({ x: xAt(i), y: yAt(b.total_cost) }));
    const bLinePath = smoothPath(bpts, PAD_T, Hsvg);
    const bAreaPath =
      bn > 0
        ? `${bLinePath} L ${bpts[bn - 1].x.toFixed(1)},${Hsvg} L ${bpts[0].x.toFixed(1)},${Hsvg} Z`
        : "";
    return { n: bn, pts: bpts, linePath: bLinePath, areaPath: bAreaPath };
  }, [buckets]);

  if (buckets.length === 0) {
    return null;
  }

  const lastIdx = n - 1;
  const shownIdx = hoverIdx ?? lastIdx;
  const shownBucket = buckets[shownIdx];

  return (
    <div className="popover-trend-chart" onMouseLeave={() => setHoverIdx(null)}>
      <div className="popover-trend-value">
        {formatCostUsd(shownBucket.total_cost)}
        {/* time_bucket 含时间段取 HH:MM（hourly "...HH:00:00"→HH:00），仅日期取 MM-DD；slice(-5) 对 hourly 误取秒位 */}
        <span className="popover-trend-bucket">
          {shownBucket.time_bucket.includes(" ")
            ? shownBucket.time_bucket.slice(11, 16)
            : shownBucket.time_bucket.slice(5)}
        </span>
      </div>
      <svg
        viewBox={`0 0 ${W} ${Hsvg}`}
        preserveAspectRatio="none"
        className="popover-trend-svg"
      >
        <defs>
          <linearGradient id="popoverTrendArea" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--primary)" stopOpacity="0.28" />
            <stop offset="100%" stopColor="var(--primary)" stopOpacity="0.02" />
          </linearGradient>
        </defs>
        <path d={areaPath} fill="url(#popoverTrendArea)" />
        <path
          d={linePath}
          fill="none"
          stroke="color-mix(in srgb, var(--primary) 82%, #000)"
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
          fill="var(--primary)"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
    </div>
  );
}
