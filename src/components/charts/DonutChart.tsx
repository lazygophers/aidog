// ── 环形图（#35 批次一）：收编 ShareDonut（#26 原型）能力——
// 首位琥珀 + 灰阶（公共层 seriesColor）、topN + 「其他」合并、中央总值、侧列占比图例。
// 旧 src/components/shared/ShareDonut.tsx 随本组件删除（spec §B2 迁移清单）。
import { useMemo, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Cell, Pie, PieChart as RechartsPieChart } from "recharts";
import { ChartContainer, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { formatPercent } from "@/utils/formatters";
import { ChartCard } from "./ChartCard";
import { ChartsTooltip, tooltipValueRows } from "./tooltip";
import { seriesColor } from "./palette";
import { drawInProps } from "./drawIn";

type Slice = { name: string; value: number; fill: string; percent: number };

export interface DonutChartProps {
  /** 占比数据（顺序随意，内部按 value 降序）。 */
  data: { name: string; value: number }[];
  /** 前几名单列扇区，其余合并「其他」，默认 4。 */
  topN?: number;
  /** 数值格式化（中央总值 / tooltip / 图例），缺省 toLocaleString。 */
  formatValue?: (n: number) => string;
  /** 中央总值下的说明字（如「总成本」）。 */
  centerLabel?: ReactNode;
  /** 环形直径 px，默认 200。 */
  size?: number;
  title?: ReactNode;
  subtitle?: ReactNode;
  emptyHint?: ReactNode;
  className?: string;
  /** 入场动画（默认开；扇区动画依赖 rAF 步进，测试环境传 false 才挂载扇区）。 */
  animate?: boolean;
}

export function DonutChart({
  data,
  topN = 4,
  formatValue,
  centerLabel,
  size = 200,
  title,
  subtitle,
  emptyHint,
  className,
  animate = true,
}: DonutChartProps) {
  const { t } = useTranslation();

  const slices = useMemo<Slice[]>(() => {
    const sorted = data
      .filter((d) => d.value > 0)
      .sort((a, b) => b.value - a.value);
    if (sorted.length === 0) return [];
    const total = sorted.reduce((s, d) => s + d.value, 0);
    const items = sorted
      .slice(0, topN)
      .map((d) => ({ name: d.name, value: d.value }));
    const rest = sorted.slice(topN).reduce((s, d) => s + d.value, 0);
    if (rest > 0) items.push({ name: t("stats.donutRest", "其他"), value: rest });
    return items.map((d, i) => ({
      ...d,
      // 首位琥珀（值最大项），其后灰阶循环
      fill: seriesColor(i),
      percent: (d.value / total) * 100,
    }));
  }, [data, topN, t]);

  const fmt = formatValue ?? ((n: number) => n.toLocaleString());
  const total = useMemo(() => slices.reduce((s, d) => s + d.value, 0), [slices]);
  const config = useMemo<ChartConfig>(() => ({ value: {} }), []);

  // 单一有效扇区构不成占比图（100% 一块），走诚实空态
  if (slices.length < 2) {
    return (
      <ChartCard title={title} subtitle={subtitle} empty emptyHint={emptyHint} className={className}>{null}</ChartCard>
    );
  }

  return (
    <ChartCard title={title} subtitle={subtitle} emptyHint={emptyHint} className={className}>
      <div style={{ display: "flex", alignItems: "center", gap: 24, flexWrap: "wrap" }}>
        <div style={{ position: "relative", width: size, height: size, flexShrink: 0 }}>
          <ChartContainer config={config} className="h-full w-full">
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
                innerRadius="62%"
                outerRadius="88%"
                paddingAngle={2}
                strokeWidth={0}
                {...(animate ? drawInProps(600) : { isAnimationActive: false })}
              >
                {slices.map((d) => (
                  <Cell key={d.name} fill={d.fill} />
                ))}
              </Pie>
            </RechartsPieChart>
          </ChartContainer>
          <div
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              pointerEvents: "none",
            }}
          >
            <span style={{ fontSize: 15, fontWeight: 600, fontVariantNumeric: "tabular-nums" }}>
              {fmt(total)}
            </span>
            {centerLabel != null && (
              <span style={{ fontSize: 10, color: "var(--text-tertiary)" }}>{centerLabel}</span>
            )}
          </div>
        </div>
        <div style={{ flex: 1, minWidth: 180, fontSize: 12, fontVariantNumeric: "tabular-nums" }}>
          {slices.map((d) => (
            <div
              key={d.name}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                padding: "5px 0",
                borderBottom: "1px solid var(--border)",
              }}
            >
              <span style={{ width: 8, height: 8, borderRadius: 2, background: d.fill, flexShrink: 0 }} />
              <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {d.name}
              </span>
              <span style={{ marginLeft: "auto", color: "var(--text-secondary)" }}>
                {formatPercent(d.percent)}
              </span>
            </div>
          ))}
        </div>
      </div>
    </ChartCard>
  );
}
