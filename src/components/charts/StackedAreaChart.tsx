// ── 堆叠面积图（#39 批次二）：series_by 交叉聚合宽表（T7 buildTrendChartData 形状）→
// stackId 单栈；系列序即堆叠序（Stats 喂总量降序 → 最大维度沉底、主琥珀首层）。
// LTTB 按行总量取形：降采样选点看栈顶轮廓，各系列共用同一行集，堆叠形状不破。
// 可选 rightConfig：右轴独立标尺的总量线（不进栈、不参与 LTTB 取形，Home 维度趋势用）。
// 可选 bare + textColor：嵌入非主题面（Home 命令面板深色硬编码底）时拆玻璃卡外壳 +
// 传显式轴 / 图例 / 空态文字色，绕开主题 CSS 变量（浅色主题深字深底不可读）。
import { useMemo, type CSSProperties, type ReactNode } from "react";
import { Area, AreaChart as RechartsAreaChart, CartesianGrid, Legend, Line, XAxis, YAxis } from "recharts";
import { useTranslation } from "react-i18next";
import { ChartContainer, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { ChartCard } from "./ChartCard";
import { cn } from "@/lib/utils";
import { ChartsTooltip, tooltipValueRows, TOOLTIP_THROTTLE_MS } from "./tooltip";
import { withDefaultColors } from "./palette";
import { formatTimeTick, niceTicks, xNum } from "./ticks";
import { downsampleLTTB } from "./downsample";
import { useDrawIn } from "./drawIn";

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
  /** 双 Y 轴（LineChart 同范式）：这些系列键挂右轴独立标尺、渲染为虚线总量线；
   *  不进栈、不计入 LTTB 取形的行总量（栈顶轮廓由堆叠层决定）。 */
  rightConfig?: ChartConfig;
  /** 右轴数值格式化（右轴刻度 + tooltip 右轴系列），缺省同 valueFormat。 */
  rightValueFormat?: (n: number) => string;
  title?: ReactNode;
  subtitle?: ReactNode;
  emptyHint?: ReactNode;
  className?: string;
  /** 嵌入非主题面时置 true：拆掉 ChartCard 玻璃卡外壳，直接出图表 / 空态。
   *  Home 命令面板硬编码深色底用；缺省 false 走原玻璃卡（Stats 等，零影响）。 */
  bare?: boolean;
  /** 显式轴 / 图例 / 空态文字色（CSS 颜色）。不传走主题 CSS 变量（原行为）。
   *  面板底是硬编码深色而非主题面时必传，否则浅色主题下深字深底不可读。 */
  textColor?: string;
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
  rightConfig,
  rightValueFormat,
  title,
  subtitle,
  emptyHint,
  className,
  bare,
  textColor,
  children,
}: StackedAreaChartProps) {
  const { t } = useTranslation();
  const drawIn = useDrawIn();
  const seriesKeys = useMemo(() => Object.keys(config), [config]);
  const rightKeys = useMemo(() => (rightConfig ? Object.keys(rightConfig) : []), [rightConfig]);
  const allKeys = useMemo(() => [...seriesKeys, ...rightKeys], [seriesKeys, rightKeys]);
  const effConfig = useMemo(
    () => withDefaultColors(rightConfig ? { ...config, ...rightConfig } : config),
    [config, rightConfig],
  );

  /** 行总量 = 各堆叠系列值之和（LTTB 取形 + Y 域都看栈顶轮廓，右轴总量线不参与）。 */
  const rowTotal = (r: Record<string, unknown>) =>
    seriesKeys.reduce((s, k) => s + (Number(r[k]) || 0), 0);

  const { rows, downsampled } = useMemo(() => {
    const sampled = downsampleLTTB(data, (r) => xNum(r[xKey]), rowTotal);
    return { rows: sampled, downsampled: sampled.length < data.length };
  }, [data, xKey, seriesKeys]);

  const domain = useMemo(() => {
    const xs = rows.map((r) => xNum(r[xKey]));
    const totals = rows.map(rowTotal);
    const rightMax = rows.reduce(
      (m, r) => rightKeys.reduce((mm, k) => Math.max(mm, Number(r[k]) || 0), m),
      0,
    );
    return {
      xMin: Math.min(...xs),
      xMax: Math.max(...xs),
      yTicks: niceTicks(0, Math.max(...totals, 0), tickCount),
      rightYTicks: rightKeys.length > 0 ? niceTicks(0, rightMax, tickCount) : [],
    };
  }, [rows, xKey, seriesKeys, rightKeys, tickCount]);

  const xTicks = useMemo(
    () => (Number.isFinite(domain.xMin) ? niceTicks(domain.xMin, domain.xMax, 6) : []),
    [domain],
  );
  const fmt = valueFormat ?? ((n: number) => n.toLocaleString());
  const rightFmt = rightValueFormat ?? fmt;
  // tooltip 右轴系列按 dataKey 分派 rightFmt（LineChart 同范式：name 恒为 dataKey）。
  const rightKeySet = new Set(rightKeys);
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

  const chart = (
    <ChartContainer
      config={effConfig}
      className={cn(
        "w-full",
        // 轴刻度文字：ChartContainer 基类用 CSS fill-muted-foreground 上色（CSS 压过
        // Recharts tick 属性），显式色只能用 !important 同选择器覆盖，走 --chart-fg 传入。
        textColor && "[&_.recharts-cartesian-axis-tick_text]:fill-[var(--chart-fg)]!",
      )}
      style={{
        height,
        ...(textColor ? ({ "--chart-fg": textColor } as CSSProperties) : {}),
      }}
    >
      <RechartsAreaChart data={rows} throttleDelay={TOOLTIP_THROTTLE_MS} margin={{ top: 8, right: 12, bottom: 0, left: 0 }}>
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
          yAxisId="left"
          width={48}
          {...(domain.yTicks.length > 0 && {
            ticks: domain.yTicks,
            domain: [domain.yTicks[0], domain.yTicks[domain.yTicks.length - 1]] as [number, number],
          })}
          tickFormatter={(v: number) => fmt(v)}
          tickLine={false}
          axisLine={false}
        />
        {rightKeys.length > 0 && (
          <YAxis
            yAxisId="right"
            orientation="right"
            width={48}
            {...(domain.rightYTicks.length > 0 && {
              ticks: domain.rightYTicks,
              domain: [domain.rightYTicks[0], domain.rightYTicks[domain.rightYTicks.length - 1]] as [number, number],
            })}
            tickFormatter={(v: number) => rightFmt(v)}
            tickLine={false}
            axisLine={false}
          />
        )}
        {allKeys.length > 1 && (
          <Legend
            verticalAlign="top"
            align="left"
            iconType="square"
            iconSize={10}
            wrapperStyle={{ fontSize: 11, color: textColor ?? "var(--text-secondary)" }}
            formatter={(v: unknown) => effConfig[String(v)]?.label ?? String(v)}
          />
        )}
        <ChartsTooltip
          content={
            <ChartTooltipContent
              labelFormatter={(label: unknown) => formatTimeTick(Number(label), spanMs)}
              formatter={tooltipValueRows(
                (n, name) => (rightKeySet.has(String(name)) ? rightFmt(n) : fmt(n)),
                (name) => effConfig[String(name)]?.label ?? String(name),
              )}
            />
          }
        />
        {seriesKeys.map((k) => (
          <Area
            key={k}
            dataKey={k}
            stackId="s"
            yAxisId="left"
            type="monotone"
            stroke={`var(--color-${k})`}
            strokeWidth={1.5}
            fill={`url(#stack-${k})`}
            {...drawIn}
          />
        ))}
        {rightKeys.map((k) => (
          <Line
            key={k}
            dataKey={k}
            yAxisId="right"
            type="monotone"
            stroke={`var(--color-${k})`}
            strokeWidth={2}
            strokeDasharray="3 3"
            dot={false}
            activeDot={{ r: 3 }}
            {...drawIn}
          />
        ))}
        {children}
      </RechartsAreaChart>
    </ChartContainer>
  );

  // bare：无玻璃卡外壳，自渲染诚实空态（标题 / 副题 / 降采样注记随外壳一起不出现——
  // 该模式只用于嵌入自带标题排版的容器，如 Home 命令面板维度趋势）。
  if (bare) {
    if (data.length === 0) {
      return (
        <div
          style={{
            minHeight: 160,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: 4,
            fontSize: 12,
            color: textColor ?? "var(--text-secondary)",
          }}
        >
          <span>{t("charts.noData", "暂无数据")}</span>
          {emptyHint != null && (
            <span style={{ fontSize: 11, opacity: 0.75 }}>{emptyHint}</span>
          )}
        </div>
      );
    }
    return chart;
  }
  return (
    <ChartCard title={title} subtitle={sub} empty={data.length === 0} emptyHint={emptyHint} className={className}>
      {chart}
    </ChartCard>
  );
}
