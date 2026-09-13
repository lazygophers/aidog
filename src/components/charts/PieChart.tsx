// ── 饼图（#35 批次一）：{name, value} 数据，色板首位琥珀 + 灰阶循环（公共层 seriesColors）。
// 环形（内孔 + 中央总值 + topN 合并）用 DonutChart。
import { useMemo, type ReactNode } from "react";
import { Cell, Pie, PieChart as RechartsPieChart } from "recharts";
import { ChartContainer, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { ChartCard } from "./ChartCard";
import { ChartsTooltip, tooltipValueRows } from "./tooltip";
import { seriesColors } from "./palette";
import { drawInProps } from "./drawIn";

export interface PieChartProps {
  /** 占比数据：name 进 tooltip，value 定扇区。内部过滤 value ≤ 0。 */
  data: { name: string; value: number }[];
  height?: number;
  /** 数值格式化（tooltip），缺省 toLocaleString。 */
  formatValue?: (n: number) => string;
  title?: ReactNode;
  subtitle?: ReactNode;
  emptyHint?: ReactNode;
  className?: string;
  /** Recharts 子组件穿透。 */
  children?: ReactNode;
  /** 入场动画（默认开；扇区动画依赖 rAF 步进，测试环境传 false 才挂载扇区）。 */
  animate?: boolean;
}

export function PieChart({
  data,
  height = 240,
  formatValue,
  title,
  subtitle,
  emptyHint,
  className,
  children,
  animate = true,
}: PieChartProps) {
  const slices = useMemo(() => data.filter((d) => d.value > 0), [data]);
  const fills = useMemo(() => seriesColors(slices.length), [slices.length]);
  const fmt = formatValue ?? ((n: number) => n.toLocaleString());
  // 单一值系列：config 只为 ChartStyle 落 --color-value（首色琥珀），名称展示走 nameKey。
  const config = useMemo<ChartConfig>(() => ({ value: {} }), []);

  return (
    <ChartCard
      title={title}
      subtitle={subtitle}
      empty={slices.length === 0}
      emptyHint={emptyHint}
      className={className}
    >
      <ChartContainer config={config} className="w-full" style={{ height }}>
        <RechartsPieChart>
          <ChartsTooltip
            content={
              <ChartTooltipContent
                nameKey="name"
                hideLabel
                formatter={tooltipValueRows(fmt)}
              />
            }
          />
          <Pie
            data={slices}
            dataKey="value"
            nameKey="name"
            outerRadius="85%"
            strokeWidth={0}
            {...(animate ? drawInProps(600) : { isAnimationActive: false })}
          >
            {slices.map((d, i) => (
              <Cell key={d.name} fill={fills[i]} />
            ))}
          </Pie>
          {children}
        </RechartsPieChart>
      </ChartContainer>
    </ChartCard>
  );
}
