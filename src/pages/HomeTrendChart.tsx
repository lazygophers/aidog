// ── 首页维度趋势区块（home-dim-trend spec）：按模型 / 按分组 24h 堆叠面积 + 右轴请求线 ──
// 数据源 = load() 里 queryBatch 的 series_by 逐维度小时序列（与 DimPanel 行共用同一份查询）。
// 纯函数（buildDimTrend / buildSparkMap）导出供 Home.test 范式单测；组件只做 tab / 指标切换。
import { useMemo, useState, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { StatsBucket, StatsSeries } from "../services/api";
import type { ChartConfig } from "@/components/ui/chart";
import { StackedAreaChart, bucketMs } from "@/components/charts";
import { formatNumber, formatCostUsd } from "../utils/formatters";
import { F } from "../domains/shared/tokens";
import { DIM_TOP_N } from "./Home";

export type TrendMetric = "cost" | "tokens";
export type TrendDim = "model" | "group";

const bucketTokens = (b: StatsBucket) => b.input_tokens + b.output_tokens + b.cache_tokens;
const seriesTokens = (s: StatsSeries) => s.buckets.reduce((sum, b) => sum + bucketTokens(b), 0);

/** 多条维度序列合并成一条（「其它」层 / 行）：同桶逐字段求和，比率与均值不可加置 0。 */
function mergeSeries(rest: StatsSeries[]): StatsSeries {
  const m = new Map<string, StatsBucket>();
  for (const s of rest) {
    for (const b of s.buckets) {
      const cur = m.get(b.time_bucket);
      m.set(b.time_bucket, cur
        ? {
            time_bucket: b.time_bucket,
            total_requests: cur.total_requests + b.total_requests,
            success_count: cur.success_count + b.success_count,
            error_count: cur.error_count + b.error_count,
            input_tokens: cur.input_tokens + b.input_tokens,
            output_tokens: cur.output_tokens + b.output_tokens,
            cache_tokens: cur.cache_tokens + b.cache_tokens,
            avg_duration_ms: 0,
            total_cost: cur.total_cost + b.total_cost,
          }
        : { ...b });
    }
  }
  return {
    name: "",
    buckets: [...m.keys()].sort().map(k => m.get(k)!),
  };
}

export interface DimTrendData {
  /** 堆叠层声明（键 s0.. 安全 CSS 变量名，label = 维度名 / 其它 / 未分组）。 */
  config: ChartConfig;
  /** 按小时桶合并的宽表：x = 本地 ms + 各层指标值 + req = 全维度请求合计（右轴线）。 */
  rows: Record<string, unknown>[];
}

/**
 * 维度小时序列 → Top8 + 「其它」堆叠宽表（tokens 降序，与 buildDimRows 口径一致）。
 * 指标二选一：cost = 逐桶 total_cost / tokens = 逐桶三 token 合计；req 行恒为请求总数。
 */
export function buildDimTrend(
  series: StatsSeries[],
  metric: TrendMetric,
  labels: { other: string; ungrouped?: string },
): DimTrendData {
  const sorted = [...series].sort((a, b) => seriesTokens(b) - seriesTokens(a));
  const kept = sorted.slice(0, DIM_TOP_N);
  const rest = sorted.slice(DIM_TOP_N);
  const layers = [...kept, ...(rest.length > 0 ? [mergeSeries(rest)] : [])];
  const layerLabel = (s: StatsSeries, isOther: boolean) =>
    isOther ? labels.other : (s.name || labels.ungrouped || s.name);

  const val = (b: StatsBucket) => (metric === "cost" ? b.total_cost : bucketTokens(b));
  const rowMap = new Map<string, Record<string, unknown>>();
  layers.forEach((s, i) => {
    for (const b of s.buckets) {
      let row = rowMap.get(b.time_bucket);
      if (!row) {
        row = { x: bucketMs(b.time_bucket), req: 0 };
        rowMap.set(b.time_bucket, row);
      }
      row[`s${i}`] = val(b);
      row.req = (row.req as number) + b.total_requests;
    }
  });
  const rows = [...rowMap.values()].sort((a, b) => (a.x as number) - (b.x as number));
  const config: ChartConfig = Object.fromEntries(
    layers.map((s, i) => [`s${i}`, { label: layerLabel(s, i >= kept.length) }]),
  );
  return { config, rows };
}

/**
 * 维度小时序列 → 行迷你曲线数据（DimPanel 行尾 sparkline 用）：
 * key = 维度名（分组维度空名归 ungroupedLabel，与 buildDimRows 改名一致），值 = 逐桶 token。
 */
export function buildSparkMap(series: StatsSeries[], ungroupedLabel?: string): Map<string, number[]> {
  const m = new Map<string, number[]>();
  for (const s of series) {
    const key = s.name || ungroupedLabel || s.name;
    m.set(key, s.buckets.map(bucketTokens));
  }
  return m;
}

const tabButton = (active: boolean): CSSProperties => ({
  fontSize: 11,
  padding: "3px 10px",
  borderRadius: 6,
  border: `1px solid ${active ? "color-mix(in srgb, var(--accent-edge) 40%, var(--border))" : "var(--border)"}`,
  background: active ? "var(--accent-subtle)" : "transparent",
  color: active ? "var(--accent)" : "var(--text-secondary)",
  cursor: "pointer",
  whiteSpace: "nowrap",
});

export function HomeTrendChart({
  modelSeries,
  groupSeries,
  ungroupedLabel,
  loading,
}: {
  modelSeries: StatsSeries[];
  groupSeries: StatsSeries[];
  ungroupedLabel: string;
  loading: boolean;
}) {
  const { t } = useTranslation();
  const [dim, setDim] = useState<TrendDim>("model");
  const [metric, setMetric] = useState<TrendMetric>("cost");

  const series = dim === "model" ? modelSeries : groupSeries;
  const { config, rows } = useMemo(
    () => buildDimTrend(series, metric, { other: t("home.dimOther", "其它"), ungrouped: dim === "group" ? ungroupedLabel : undefined }),
    [series, metric, t, dim, ungroupedLabel],
  );

  const dims: { id: TrendDim; key: string; def: string }[] = [
    { id: "model", key: "home.tabModel", def: "按模型" },
    { id: "group", key: "home.tabGroup", def: "按分组" },
  ];
  const metrics: { id: TrendMetric; key: string; def: string }[] = [
    { id: "cost", key: "home.trendCost", def: "花费" },
    { id: "tokens", key: "home.tokens", def: "Token" },
  ];

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
        <b style={{ fontSize: F.small + 1 }}>{t("home.dimTrendTitle", "维度趋势 · 24 小时")}</b>
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap", alignItems: "center" }}>
          <div role="group" aria-label={t("home.dimTrendTitle", "维度趋势 · 24 小时")} style={{ display: "flex", gap: 4 }}>
            {dims.map(d => (
              <button key={d.id} type="button" style={tabButton(dim === d.id)} aria-pressed={dim === d.id} onClick={() => setDim(d.id)}>
                {t(d.key, d.def)}
              </button>
            ))}
          </div>
          <div role="group" aria-label={t("home.dimMetric", "tokens / 花费 / 请求")} style={{ display: "flex", gap: 4 }}>
            {metrics.map(m => (
              <button key={m.id} type="button" style={tabButton(metric === m.id)} aria-pressed={metric === m.id} onClick={() => setMetric(m.id)}>
                {t(m.key, m.def)}
              </button>
            ))}
          </div>
        </div>
      </div>
      <StackedAreaChart
        config={config}
        data={rows}
        height={240}
        valueFormat={metric === "cost" ? formatCostUsd : formatNumber}
        rightConfig={{ req: { label: t("home.trendRequests", "请求数") } }}
        rightValueFormat={formatNumber}
        emptyHint={loading ? "" : t("home.noToday", "今日暂无请求")}
      />
    </div>
  );
}
