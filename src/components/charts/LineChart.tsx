// ── 折线图（#35 批次一）：config 声明式系列 + 公共层轴刻度 / LTTB 降采样 / drawIn 动画 / tooltip。
// xKey 取时间戳（number ms 或 Date；串会被 " "→"T" 归一再 Date.parse，Safari 兼容）。
// data 需按 x 升序（LTTB 前提，代理日志 bucket 天然有序）。
import { useId, useMemo, type ReactNode } from "react";
import { Area, CartesianGrid, ComposedChart, Legend, Line, XAxis, YAxis } from "recharts";
import { useTranslation } from "react-i18next";
import { ChartContainer, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { ChartCard } from "./ChartCard";
import { ChartsTooltip, tooltipValueRows } from "./tooltip";
import { withDefaultColors } from "./palette";
import { formatTimeTick, niceTicks, xNum } from "./ticks";
import { downsampleLTTB } from "./downsample";
import { drawInProps } from "./drawIn";

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
  /** 迷你模式（spec §C2 浮窗曲线）：去 ChartCard 容器 / 网格 / 轴 / 图例，仅曲线 + tooltip；
   *  轴保留为 hide（非删除），tooltip 时间标签仍走 labelFormatter。高度缺省 64。 */
  mini?: boolean;
  /** 双 Y 轴（spec §B2 公共层，组件不各自实现归一化）：这些系列键挂右轴独立标尺，
   *  与 config（左轴）合并生成色变量；双轴刻度 / tooltip 各走各的 valueFormat。 */
  rightConfig?: ChartConfig;
  /** 右轴数值格式化（右轴刻度 + tooltip 右轴系列），缺省 toLocaleString。 */
  rightValueFormat?: (n: number) => string;
  /** 首个左轴系列下方渐变面积填充（紧凑双线视觉：主线琥珀 + 面积）。 */
  area?: boolean;
  /** 虚线系列键（辅线灰阶虚线等），渲染为 strokeDasharray "3 3"。 */
  dashedKeys?: string[];
}

export function LineChart({
  config,
  data,
  xKey = "x",
  height,
  valueFormat,
  tickCount = 5,
  title,
  subtitle,
  emptyHint,
  className,
  children,
  mini = false,
  rightConfig,
  rightValueFormat,
  area = false,
  dashedKeys = [],
}: LineChartProps) {
  const { t } = useTranslation();
  const areaId = useId();
  const leftKeys = useMemo(() => Object.keys(config), [config]);
  const rightKeys = useMemo(() => (rightConfig ? Object.keys(rightConfig) : []), [rightConfig]);
  const seriesKeys = useMemo(() => [...leftKeys, ...rightKeys], [leftKeys, rightKeys]);
  const effConfig = useMemo(
    () => withDefaultColors(rightConfig ? { ...config, ...rightConfig } : config),
    [config, rightConfig],
  );

  const { rows, downsampled } = useMemo(() => {
    const sampled = downsampleLTTB(
      data,
      (r) => xNum(r[xKey]),
      (r) => Number(r[seriesKeys[0]]),
    );
    // ponytail: LTTB 只按 seriesKeys[0]（主系列）选点，多序列时次系列尖峰可能被降采样丢掉
    //（已知局限）；需要多准则保形时升级为 per-series 或联合 LTTB 再改这里。
    return { rows: sampled, downsampled: sampled.length < data.length };
  }, [data, xKey, seriesKeys]);

  const domain = useMemo(() => {
    const xs = rows.map((r) => xNum(r[xKey]));
    const ticksFor = (keys: string[]) => {
      const ys = rows.flatMap((r) => keys.map((k) => Number(r[k])));
      return niceTicks(Math.min(0, ...ys), Math.max(...ys), tickCount);
    };
    return {
      xMin: Math.min(...xs),
      xMax: Math.max(...xs),
      yTicks: ticksFor(leftKeys),
      rightYTicks: rightKeys.length > 0 ? ticksFor(rightKeys) : [],
    };
  }, [rows, xKey, leftKeys, rightKeys, tickCount]);

  const xTicks = useMemo(
    () => (Number.isFinite(domain.xMin) ? niceTicks(domain.xMin, domain.xMax, 6) : []),
    [domain],
  );
  const fmt = valueFormat ?? ((n: number) => n.toLocaleString());
  const rightFmt = rightValueFormat ?? fmt;
  // tooltip 右轴系列按 label 分派 rightFmt（label 撞名时退左轴格式化，可接受的已知上限）。
  const rightLabels = new Set(rightKeys.map((k) => String(effConfig[k]?.label ?? k)));
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

  const effHeight = height ?? (mini ? 64 : 240);

  const chart = (
    <ChartContainer config={effConfig} className="w-full" style={{ height: effHeight }}>
      <ComposedChart data={rows} margin={mini ? { top: 4, right: 4, bottom: 0, left: 0 } : { top: 8, right: 12, bottom: 0, left: 0 }}>
        {!mini && <CartesianGrid vertical={false} strokeDasharray="3 3" />}
        {area && leftKeys.length > 0 && (
          <defs>
            <linearGradient id={areaId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stopColor={`var(--color-${leftKeys[0]})`} stopOpacity=".2" />
              <stop offset="1" stopColor={`var(--color-${leftKeys[0]})`} stopOpacity="0" />
            </linearGradient>
          </defs>
        )}
        <XAxis
          dataKey={xKey}
          type="number"
          scale="time"
          hide={mini}
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
            hide={mini}
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
              hide={mini}
              {...(domain.rightYTicks.length > 0 && {
                ticks: domain.rightYTicks,
                domain: [domain.rightYTicks[0], domain.rightYTicks[domain.rightYTicks.length - 1]] as [number, number],
              })}
              tickFormatter={(v: number) => rightFmt(v)}
              tickLine={false}
              axisLine={false}
            />
          )}
          {/* 多系列图例（spec C1 时间序列 tab 按维度多序列）：标签取 config label（s0 等安全键不外露） */}
          {!mini && seriesKeys.length > 1 && (
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
                formatter={tooltipValueRows((n, name) =>
                  rightLabels.has(String(name)) ? rightFmt(n) : fmt(n),
                )}
              />
            }
          />
          {area && leftKeys.length > 0 && (
            <Area
              type="monotone"
              dataKey={leftKeys[0]}
              yAxisId="left"
              stroke="none"
              fill={`url(#${areaId})`}
              activeDot={false}
              legendType="none"
            />
          )}
          {seriesKeys.map((k) => (
            <Line
              key={k}
              dataKey={k}
              yAxisId={rightKeys.includes(k) ? "right" : "left"}
              stroke={`var(--color-${k})`}
              strokeWidth={2}
              strokeDasharray={dashedKeys.includes(k) ? "3 3" : undefined}
              dot={mini ? false : rows.length <= 60}
              activeDot={{ r: mini ? 2.5 : 3 }}
              {...drawInProps()}
            />
          ))}
          {children}
        </ComposedChart>
      </ChartContainer>
  );
  // mini：裸渲染（浮窗自带卡片壳），空态由调用方前置判定（诚实空态文案归浮窗）
  if (mini) return chart;
  return (
    <ChartCard title={title} subtitle={sub} empty={data.length === 0} emptyHint={emptyHint} className={className}>
      {chart}
    </ChartCard>
  );
}
