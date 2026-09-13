// ── 柱状图（#35 批次一）：config 声明式系列（每 key 一根柱），Y 轴走公共层 nice-ticks。
// 横轴类目型（xKey 默认 "name"）；时间轴折线请用 LineChart。
import { useMemo, type ReactNode } from "react";
import { Bar, BarChart as RechartsBarChart, CartesianGrid, XAxis, YAxis } from "recharts";
import { ChartContainer, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { ChartCard } from "./ChartCard";
import { ChartsTooltip, tooltipValueRows } from "./tooltip";
import { withDefaultColors } from "./palette";
import { niceTicks } from "./ticks";
import { drawInProps } from "./drawIn";

export interface BarChartProps {
  /** 系列声明：每个 key 一组柱；缺 color 按键序走公共层色板（首位琥珀）。 */
  config: ChartConfig;
  data: Record<string, unknown>[];
  /** 横轴（类目）字段名，默认 "name"。 */
  xKey?: string;
  height?: number;
  valueFormat?: (n: number) => string;
  tickCount?: number;
  title?: ReactNode;
  subtitle?: ReactNode;
  emptyHint?: ReactNode;
  className?: string;
  /** Recharts 子组件穿透（ReferenceLine / Cell 定制色等）。 */
  children?: ReactNode;
}

export function BarChart({
  config,
  data,
  xKey = "name",
  height = 240,
  valueFormat,
  tickCount = 5,
  title,
  subtitle,
  emptyHint,
  className,
  children,
}: BarChartProps) {
  const seriesKeys = useMemo(() => Object.keys(config), [config]);
  const effConfig = useMemo(() => withDefaultColors(config), [config]);

  const yTicks = useMemo(() => {
    const ys = data.flatMap((r) => seriesKeys.map((k) => Number(r[k])));
    return niceTicks(Math.min(0, ...ys), Math.max(...ys), tickCount);
  }, [data, seriesKeys, tickCount]);

  const fmt = valueFormat ?? ((n: number) => n.toLocaleString());

  return (
    <ChartCard title={title} subtitle={subtitle} empty={data.length === 0} emptyHint={emptyHint} className={className}>
      <ChartContainer config={effConfig} className="w-full" style={{ height }}>
        <RechartsBarChart data={data} margin={{ top: 8, right: 12, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} strokeDasharray="3 3" />
          <XAxis dataKey={xKey} tickLine={false} axisLine={false} tickMargin={8} interval={0} />
          <YAxis
            width={48}
            {...(yTicks.length > 0 && {
              ticks: yTicks,
              domain: [yTicks[0], yTicks[yTicks.length - 1]] as [number, number],
            })}
            tickFormatter={(v: number) => fmt(v)}
            tickLine={false}
            axisLine={false}
          />
          <ChartsTooltip
            content={<ChartTooltipContent formatter={tooltipValueRows(fmt)} />}
          />
          {seriesKeys.map((k) => (
            <Bar
              key={k}
              dataKey={k}
              fill={`var(--color-${k})`}
              radius={[4, 4, 0, 0]}
              {...drawInProps()}
            />
          ))}
          {children}
        </RechartsBarChart>
      </ChartContainer>
    </ChartCard>
  );
}
