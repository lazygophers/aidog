// ── 折线图（#35 批次一）：config 声明式系列 + 公共层轴刻度 / LTTB 降采样 / drawIn 动画 / tooltip。
// xKey 取时间戳（number ms 或 Date；串会被 " "→"T" 归一再 Date.parse，Safari 兼容）。
// data 需按 x 升序（LTTB 前提，代理日志 bucket 天然有序）。
import { useMemo, type ReactNode } from "react";
import { CartesianGrid, Legend, Line, LineChart as RechartsLineChart, XAxis, YAxis } from "recharts";
import { useTranslation } from "react-i18next";
import { ChartContainer, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { ChartCard } from "./ChartCard";
import { ChartsTooltip, tooltipValueRows } from "./tooltip";
import { withDefaultColors } from "./palette";
import { formatTimeTick, niceTicks } from "./ticks";
import { downsampleLTTB } from "./downsample";
import { drawInProps } from "./drawIn";

/** x 值归一为 ms 时间戳：number 原样，Date 取时间，串归一后 Date.parse（NaN 交轴自动回落）。 */
function xNum(v: unknown): number {
  if (typeof v === "number") return v;
  if (v instanceof Date) return v.getTime();
  return Date.parse(String(v).replace(" ", "T"));
}

export interface LineChartProps {
  /** 系列声明：每个 key 一条线，label 进 tooltip/legend；缺 color 按键序走公共层色板。 */
  config: ChartConfig;
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

export function LineChart({
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
}: LineChartProps) {
  const { t } = useTranslation();
  const seriesKeys = useMemo(() => Object.keys(config), [config]);
  const effConfig = useMemo(() => withDefaultColors(config), [config]);

  const { rows, downsampled } = useMemo(() => {
    const sampled = downsampleLTTB(
      data,
      (r) => xNum(r[xKey]),
      (r) => Number(r[seriesKeys[0]]),
    );
    return { rows: sampled, downsampled: sampled.length < data.length };
  }, [data, xKey, seriesKeys]);

  const domain = useMemo(() => {
    const xs = rows.map((r) => xNum(r[xKey]));
    const ys = rows.flatMap((r) => seriesKeys.map((k) => Number(r[k])));
    return {
      xMin: Math.min(...xs),
      xMax: Math.max(...xs),
      yTicks: niceTicks(Math.min(0, ...ys), Math.max(...ys), tickCount),
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
        <RechartsLineChart data={rows} margin={{ top: 8, right: 12, bottom: 0, left: 0 }}>
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
          {/* 多系列图例（spec C1 时间序列 tab 按维度多序列）：标签取 config label（s0 等安全键不外露） */}
          {seriesKeys.length > 1 && (
            <Legend
              verticalAlign="top"
              align="left"
              iconType="plainline"
              iconSize={12}
              wrapperStyle={{ fontSize: 11, color: "var(--text-secondary)" }}
              formatter={(v: unknown) => effConfig[String(v)]?.label ?? String(v)}
            />
          )}
          <ChartsTooltip
            content={
              <ChartTooltipContent
                labelFormatter={(label: unknown) => formatTimeTick(Number(label), spanMs)}
                formatter={tooltipValueRows(fmt)}
              />
            }
          />
          {seriesKeys.map((k) => (
            <Line
              key={k}
              dataKey={k}
              stroke={`var(--color-${k})`}
              strokeWidth={2}
              dot={rows.length <= 60}
              activeDot={{ r: 3 }}
              {...drawInProps()}
            />
          ))}
          {children}
        </RechartsLineChart>
      </ChartContainer>
    </ChartCard>
  );
}
