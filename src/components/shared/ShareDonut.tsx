// ShareDonut — 维度成本占比环形图（#26 图表引擎骨架原型：Recharts v3 + shadcn chart 公共层）。
// 系列色策略：首位琥珀（--primary，萤火虫主线），其余灰阶（--chart-2..5）。
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Cell, Pie, PieChart } from "recharts";
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";
import { formatCostUsd, formatPercent } from "@/utils/formatters";
import type { DimensionEntry } from "@/services/api/types";

const AUX_COLORS = ["var(--chart-2)", "var(--chart-3)", "var(--chart-4)", "var(--chart-5)"] as const;
const TOP_N = 4;

type Slice = { name: string; cost: number; fill: string; percent: number };

export function ShareDonut({ entries }: { entries: DimensionEntry[] }) {
  const { t } = useTranslation();

  const slices = useMemo<Slice[]>(() => {
    const sorted = entries
      .filter(e => e.total_cost > 0)
      .sort((a, b) => b.total_cost - a.total_cost);
    if (sorted.length === 0) return [];
    const total = sorted.reduce((s, e) => s + e.total_cost, 0);
    const top = sorted.slice(0, TOP_N);
    const rest = sorted.slice(TOP_N).reduce((s, e) => s + e.total_cost, 0);
    const items = top.map(e => e.total_cost);
    if (rest > 0) items.push(rest);
    return items.map((cost, i) => ({
      // 首位琥珀（成本最高项），其后灰阶；合并的「其他」固定最深灰
      name: i < top.length ? top[i].name : t("stats.donutRest", "其他"),
      cost,
      fill: i === 0 ? "var(--primary)" : AUX_COLORS[Math.min(i - 1, AUX_COLORS.length - 1)],
      percent: (cost / total) * 100,
    }));
  }, [entries, t]);

  if (slices.length < 2) return null;

  const totalCost = slices.reduce((s, d) => s + d.cost, 0);
  const chartConfig = {
    cost: { label: t("stats.totalCost", "预估成本") },
  } satisfies ChartConfig;

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 24, flexWrap: "wrap" }}>
      <div style={{ position: "relative", width: 200, height: 200, flexShrink: 0 }}>
        <ChartContainer config={chartConfig} className="h-full w-full">
          <PieChart>
            <ChartTooltip
              content={
                <ChartTooltipContent
                  nameKey="name"
                  hideLabel
                  formatter={(value, name) => (
                    <div key={String(name)} className="flex w-full items-center justify-between gap-4">
                      <span className="text-muted-foreground">{name}</span>
                      <span className="font-mono font-medium">{formatCostUsd(Number(value))}</span>
                    </div>
                  )}
                />
              }
            />
            <Pie data={slices} dataKey="cost" nameKey="name" innerRadius={62} outerRadius={92} paddingAngle={2} strokeWidth={0}>
              {slices.map(d => (
                <Cell key={d.name} fill={d.fill} />
              ))}
            </Pie>
          </PieChart>
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
          <span style={{ fontSize: 15, fontWeight: 600, fontVariantNumeric: "tabular-nums" }}>{formatCostUsd(totalCost)}</span>
          <span style={{ fontSize: 10, color: "var(--text-tertiary)" }}>{t("stats.totalCost", "预估成本")}</span>
        </div>
      </div>
      <div style={{ flex: 1, minWidth: 180, fontSize: 12, fontVariantNumeric: "tabular-nums" }}>
        {slices.map(d => (
          <div key={d.name} style={{ display: "flex", alignItems: "center", gap: 8, padding: "5px 0", borderBottom: "1px solid var(--border)" }}>
            <span style={{ width: 8, height: 8, borderRadius: 2, background: d.fill, flexShrink: 0 }} />
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{d.name}</span>
            <span style={{ marginLeft: "auto", color: "var(--text-secondary)" }}>{formatPercent(d.percent)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
