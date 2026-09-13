// ── tooltip 统一封装（spec §B2：单一实现，基于 shadcn ChartTooltip/ChartTooltipContent）──
// - 最近点吸附：Recharts <Tooltip> 默认即沿轴吸附最近数据点画 cursor，这里不重复实现
// - RTL 安全：内容容器 dir="auto"（文案方向跟随内容，阿语自动右起），布局 flex 无绝对定位左右
// - 容器溢出防裁剪：allowEscapeViewBox 允许浮层越出图表 viewBox（边缘点不被裁），
//   内容限宽 max-w-64 防长名称撑破卡片
import type * as React from "react";
import { ChartTooltip as ShadcnChartTooltip, ChartTooltipContent } from "@/components/ui/chart";

export function ChartsTooltip({
  content,
  wrapperStyle,
  ...rest
}: React.ComponentProps<typeof ShadcnChartTooltip>) {
  return (
    <ShadcnChartTooltip
      cursor={{ stroke: "var(--border)", strokeDasharray: "4 3" }}
      allowEscapeViewBox={{ x: true, y: true }}
      isAnimationActive={false}
      wrapperStyle={{ zIndex: 40, outline: "none", ...wrapperStyle }}
      content={content ?? (
        <ChartTooltipContent dir="auto" className="pointer-events-none max-w-64" />
      )}
      {...rest}
    />
  );
}

/**
 * tooltip 数值行 formatter 工厂：给 ChartTooltipContent 的 formatter，
 * 渲染「色点 + 系列名 + fmt 格式化值」单行（ShareDonut 范式提炼，四组件共用）。
 * fmt 可选第二参拿系列名（LineChart 双 Y 轴按系列分派左右轴格式化；单轴调用方忽略）。
 */
export function tooltipValueRows(fmt: (n: number, name?: unknown) => string) {
  return (value: unknown, name: unknown, item: { color?: string } | undefined): React.ReactNode => (
    <div key={String(name)} className="flex w-full items-center justify-between gap-3">
      <span className="flex items-center gap-1.5 text-muted-foreground">
        <span className="h-2 w-2 shrink-0 rounded-[2px]" style={{ background: item?.color }} />
        {String(name)}
      </span>
      <span className="font-mono font-medium tabular-nums">{fmt(Number(value), name)}</span>
    </div>
  );
}
