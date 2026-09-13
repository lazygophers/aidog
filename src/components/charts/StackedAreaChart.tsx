// ── 堆叠面积图（#39 批次二）：series_by 交叉聚合宽表（T7 buildTrendChartData 形状）→
// stackId 单栈；系列序即堆叠序（Stats 喂总量降序 → 最大维度沉底、主琥珀首层）。
// LTTB 按行总量取形：降采样选点看栈顶轮廓，各系列共用同一行集，堆叠形状不破。
import { useMemo, type ReactNode } from "react";
import { Area, AreaChart as RechartsAreaChart, CartesianGrid, Legend, XAxis, YAxis } from "recharts";
import { useTranslation } from "react-i18next";
import { ChartContainer, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { ChartCard } from "./ChartCard";
import { ChartsTooltip, tooltipValueRows } from "./tooltip";
import { withDefaultColors } from "./palette";
import { formatTimeTick, niceTicks, xNum } from "./ticks";
import { downsampleLTTB } from "./downsample";
import { drawInProps } from "./drawIn";

export interface StackedAreaChartProps {
  /** 系列声明：每个 key 一层，label 进 tooltip/legend；缺 color 按键序走公共层色板。 */
  config: ChartConfig;
  /** 按 x 升序的宽表（与 LineChart 同形状，xKey 时间戳）。 */
  data: Record<string, unknown>[];
  /** 横轴（时间）字段名，默认 "x"。 */
  xKey?: string;
  /** 图表区高度 px（ChartContainer 必须带高度，spec §A2）。 */
  height?: number;
  /** 数值格式化（tooltip + Y 轴刻度），缺省 toLocaleString。 */
  valueFormat?: (n: number) => string;
  /** Y 轴刻度数（nice-ticks 目标），默认 5。 */
  tickCount?: number;
  title?: ReactNode;
  subtitle?: ReactNode;
  emptyHint?: ReactNode;
  className?: string;
  /** Recharts 子组件穿透（ReferenceLine 等直接写这里）。 */
  children?: ReactNode;
}

export function StackedAreaChart({
  config,
  data,
  xKey = "x",
  height = 240,
  valueFormat,
  tickCount = 5,
  title,
  subtitle,
  emptyHint,
  className,
  children,
}: StackedAreaChartProps) {
  const { t } = useTranslation();
  const seriesKeys = useMemo(() => Object.keys(config), [config]);
  const effConfig = useMemo(() => withDefaultColors(config), [config]);

  /** 行总量 = 各系列值之和（LTTB 取形 + Y 域都看栈顶轮廓）。 */
  const rowTotal = (r: Record<string, unknown>) =>
    seriesKeys.reduce((s, k) => s + (Number(r[k]) || 0), 0);

  const { rows, downsampled } = useMemo(() => {
    const sampled = downsampleLTTB(data, (r) => xNum(r[xKey]), rowTotal);
    return { rows: sampled, downsampled: sampled.length < data.length };
  }, [data, xKey, seriesKeys]);

  const domain = useMemo(() => {
    const xs = rows.map((r) => xNum(r[xKey]));
    const totals = rows.map(rowTotal);
    return {
      xMin: Math.min(...xs),
      xMax: Math.max(...xs),
      yTicks: niceTicks(0, Math.max(...totals, 0), tickCount),
    };
  }, [rows, xKey, seriesKeys, tickCount]);

  const xTicks = useMemo(
    () => (Number.isFinite(domain.xMin) ? niceTicks(domain.xMin, domain.xMax, 6) : []),
    [domain],
  );
  const fmt = valueFormat ?? ((n: number) => n.toLocaleString());
  const spanMs = domain.xMax - domain.xMin;
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
        <RechartsAreaChart data={rows} margin={{ top: 8, right: 12, bottom: 0, left: 0 }}>
          <defs>
            {seriesKeys.map((k) => (
              <linearGradient key={k} id={`stack-${k}`} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor={`var(--color-${k})`} stopOpacity={0.45} />
                <stop offset="100%" stopColor={`var(--color-${k})`} stopOpacity={0.08} />
              </linearGradient>
            ))}
          </defs>
          <CartesianGrid vertical={false} strokeDasharray="3 3" />
          <XAxis
            dataKey={xKey}
            type="number"
            scale="time"
            {...(xTicks.length > 0 && {
              ticks: xTicks,
              domain: [xTicks[0], xTicks[xTicks.length - 1]] as [number, number],
            })}
            tickFormatter={(v: number) => formatTimeTick(v, spanMs)}
            tickLine={false}
            axisLine={false}
            tickMargin={8}
            minTickGap={32}
          />
          <YAxis
            width={48}
            {...(domain.yTicks.length > 0 && {
              ticks: domain.yTicks,
              domain: [domain.yTicks[0], domain.yTicks[domain.yTicks.length - 1]] as [number, number],
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
            content={
              <ChartTooltipContent
                labelFormatter={(label: unknown) => formatTimeTick(Number(label), spanMs)}
                formatter={tooltipValueRows(fmt, (name) => config[String(name)]?.label ?? String(name))}
              />
            }
          />
          {seriesKeys.map((k) => (
            <Area
              key={k}
              dataKey={k}
              stackId="s"
              type="monotone"
              stroke={`var(--color-${k})`}
              strokeWidth={1.5}
              fill={`url(#stack-${k})`}
              {...drawInProps()}
            />
          ))}
          {children}
        </RechartsAreaChart>
      </ChartContainer>
    </ChartCard>
  );
}
