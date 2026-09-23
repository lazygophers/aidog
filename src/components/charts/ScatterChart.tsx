// ── 散点直方图（T10 / #40）：渲染服务端 bin 化的 (duration × cost) 计数矩阵（spec §D2）。
// 点数 = 非零 bin 格数（≤ duration_bins × cost_bins 网格），客户端无降采样问题（§F1）；
// 点大小 ∝ counts（ZAxis），色 = 主琥珀（§A3）。
import { useMemo, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  CartesianGrid,
  Scatter,
  ScatterChart as RechartsScatterChart,
  XAxis,
  YAxis,
  ZAxis,
} from "recharts";
import { ChartContainer, ChartTooltipContent } from "@/components/ui/chart";
import type { ScatterHistogram } from "@/services/api/types/generated/ScatterHistogram";
import { formatCostUsd, formatDurationMs } from "@/utils/formatters";
import { ChartCard } from "./ChartCard";
import { ChartsTooltip } from "./tooltip";

export interface ScatterPoint {
  /** duration bin 中心（ms）。 */
  x: number;
  /** cost bin 中心（$）。 */
  y: number;
  count: number;
}

/** 纯函数：bin 矩阵 → 散点点集（bin 中心，count=0 的格剔除）。全空矩阵 → 空数组。 */
export function scatterPoints(h: ScatterHistogram): ScatterPoint[] {
  const pts: ScatterPoint[] = [];
  for (let i = 0; i < h.counts.length; i++) {
    const row = h.counts[i];
    const x0 = h.duration_bins[i];
    const x1 = h.duration_bins[i + 1];
    if (x1 == null) continue;
    for (let j = 0; j < row.length; j++) {
      const count = row[j];
      if (!count) continue;
      const y1 = h.cost_bins[j + 1];
      if (y1 == null) continue;
      pts.push({ x: (x0 + x1) / 2, y: (h.cost_bins[j] + y1) / 2, count });
    }
  }
  return pts;
}

export interface ScatterChartProps {
  /** 服务端 bin 化结果（statsApi.scatterHistogram）。 */
  histogram: ScatterHistogram;
  height?: number;
  /** 轴标签（x=延迟 y=成本），缺省走 charts.scatterX / charts.scatterY（Recharts 轴标签限纯文本）。 */
  xLabel?: string;
  yLabel?: string;
  /** 轴 / tooltip 数值格式化，缺省 formatDurationMs / formatCostUsd（utils/formatters）。 */
  formatX?: (n: number) => string;
  formatY?: (n: number) => string;
  title?: ReactNode;
  subtitle?: ReactNode;
  emptyHint?: ReactNode;
  className?: string;
}

export function ScatterChart({
  histogram,
  height = 280,
  xLabel,
  yLabel,
  formatX,
  formatY,
  title,
  subtitle,
  emptyHint,
  className,
}: ScatterChartProps) {
  const { t } = useTranslation();
  const points = useMemo(() => scatterPoints(histogram), [histogram]);
  const fmtX = formatX ?? formatDurationMs;
  const fmtY = formatY ?? formatCostUsd;
  const xl = xLabel ?? t("charts.scatterX", "延迟");
  const yl = yLabel ?? t("charts.scatterY", "成本");

  // 轴域钉在 bin 边界（服务端 nice 边界），空矩阵退化 [0,1]
  const xDomain: [number, number] = histogram.duration_bins.length > 1
    ? [histogram.duration_bins[0], histogram.duration_bins[histogram.duration_bins.length - 1]]
    : [0, 1];
  const yDomain: [number, number] = histogram.cost_bins.length > 1
    ? [histogram.cost_bins[0], histogram.cost_bins[histogram.cost_bins.length - 1]]
    : [0, 1];

  return (
    <ChartCard title={title} subtitle={subtitle} empty={points.length === 0} emptyHint={emptyHint} className={className}>
      <ChartContainer
        config={{ points: { label: "points", color: "var(--data-primary)" } }}
        className="w-full"
        style={{ height }}
      >
        <RechartsScatterChart margin={{ top: 8, right: 12, bottom: 4, left: 0 }}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis
            dataKey="x"
            type="number"
            domain={xDomain}
            tickFormatter={(v: number) => fmtX(v)}
            tickLine={false}
            axisLine={false}
            tickMargin={8}
            minTickGap={32}
            label={{ value: xl, position: "insideBottomRight", offset: -2, fontSize: 11, fill: "var(--text-tertiary)" }}
          />
          <YAxis
            dataKey="y"
            type="number"
            domain={yDomain}
            width={56}
            tickFormatter={(v: number) => fmtY(v)}
            tickLine={false}
            axisLine={false}
            label={{ value: yl, angle: -90, position: "insideLeft", fontSize: 11, fill: "var(--text-tertiary)" }}
          />
          {/* 点大小 ∝ counts：range 下限 24 保单请求可见，上限 400 防盖格 */}
          <ZAxis dataKey="count" range={[24, 400]} />
          <ChartsTooltip
            cursor={{ strokeDasharray: "4 3" }}
            content={
              <ChartTooltipContent
                hideLabel
                formatter={(_v: unknown, _n: unknown, item: { payload?: unknown }) => {
                  const p = item?.payload as ScatterPoint | undefined;
                  if (!p) return null;
                  return (
                    <div className="flex flex-col gap-0.5 font-mono text-xs tabular-nums">
                      <span>{xl} · {fmtX(p.x)}</span>
                      <span>{yl} · {fmtY(p.y)}</span>
                      <span>{t("charts.scatterCount", { count: p.count })}</span>
                    </div>
                  );
                }}
              />
            }
          />
          <Scatter name="points" data={points} fill="var(--data-primary)" fillOpacity={0.55} />
        </RechartsScatterChart>
      </ChartContainer>
    </ChartCard>
  );
}
