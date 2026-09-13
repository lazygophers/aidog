// ── 堆叠柱状图（#39 批次二）：类目横轴 × stackId 单栈（如「平台 × 日期」构成类目、
// 系列按维度拆）。Y 域按行总量（栈顶），LTTB 超阈值按行索引取形（类目无连续 x，
// 索引升序即 LTTB 前提；降采样后各系列共用行集，堆叠形状不破）。
import { useMemo, type ReactNode } from "react";
import { Bar, BarChart as RechartsBarChart, CartesianGrid, Legend, XAxis, YAxis } from "recharts";
import { useTranslation } from "react-i18next";
import { ChartContainer, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { ChartCard } from "./ChartCard";
import { ChartsTooltip, tooltipValueRows } from "./tooltip";
import { withDefaultColors } from "./palette";
import { niceTicks } from "./ticks";
import { downsampleLTTB } from "./downsample";
import { drawInProps } from "./drawIn";

export interface StackedBarChartProps {
  /** 系列声明：每个 key 一段柱，label 进 tooltip/legend；缺 color 按键序走公共层色板。 */
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

export function StackedBarChart({
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
}: StackedBarChartProps) {
  const { t } = useTranslation();
  const seriesKeys = useMemo(() => Object.keys(config), [config]);
  const effConfig = useMemo(() => withDefaultColors(config), [config]);

  const { rows, downsampled } = useMemo(() => {
    const sampled = downsampleLTTB(
      data.map((r, i) => ({ i, r })),
      (p) => p.i,
      (p) => seriesKeys.reduce((s, k) => s + (Number(p.r[k]) || 0), 0),
    );
    return { rows: sampled.map((p) => p.r), downsampled: sampled.length < data.length };
  }, [data, seriesKeys]);

  const yTicks = useMemo(() => {
    const totals = rows.map((r) => seriesKeys.reduce((s, k) => s + (Number(r[k]) || 0), 0));
    return niceTicks(0, Math.max(...totals, 0), tickCount);
  }, [rows, seriesKeys, tickCount]);

  const fmt = valueFormat ?? ((n: number) => n.toLocaleString());
  const sub = downsampled ? (
    <>
      {subtitle}
      {subtitle != null && " · "}
      {t("charts.downsampled", { count: rows.length })}
    </>
  ) : (
    subtitle
  );

  return (
    <ChartCard title={title} subtitle={sub} empty={data.length === 0} emptyHint={emptyHint} className={className}>
      <ChartContainer config={effConfig} className="w-full" style={{ height }}>
        <RechartsBarChart data={rows} margin={{ top: 8, right: 12, bottom: 0, left: 0 }}>
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
          {seriesKeys.length > 1 && (
            <Legend
              verticalAlign="top"
              align="left"
              iconType="square"
              iconSize={10}
              wrapperStyle={{ fontSize: 11, color: "var(--text-secondary)" }}
              formatter={(v: unknown) => effConfig[String(v)]?.label ?? String(v)}
            />
          )}
          <ChartsTooltip
            content={<ChartTooltipContent formatter={tooltipValueRows(fmt)} />}
          />
          {seriesKeys.map((k) => (
            <Bar key={k} dataKey={k} stackId="s" fill={`var(--color-${k})`} {...drawInProps()} />
          ))}
          {children}
        </RechartsBarChart>
      </ChartContainer>
    </ChartCard>
  );
}
