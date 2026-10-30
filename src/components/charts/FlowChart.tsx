// FlowChart — 任意列数的流向图（请求流向票 04；首页票 05 复用）。
// 每列 Top 6 + 「其他」；相邻列按行聚合连线；节点显示数值 + 同列占比。
// 行是扁平记录：columns[i].key 取节点 id，row[metric] 取数值。
import { useTranslation } from "react-i18next";
import { formatMoney, formatNumber } from "../../utils/formatters";
import { seriesColor } from "./palette";

export type FlowMetric = "tokens" | "requests" | "cost";
export type FlowChartRow = Record<string, string | number>;
export interface FlowChartColumn {
  key: string;
  title: string;
  /** 节点 id → 显示名；缺省原样显示 id。 */
  labelOf?: (id: string) => string;
}
export interface FlowChartProps {
  columns: FlowChartColumn[];
  rows: FlowChartRow[];
  metric: FlowMetric;
}

const TOP_N = 6;
const OTHER = "__other__";
const COL_W = 170;
const PITCH = 36;
const PAD = 8;

/** 占比文案：1 位小数，(0, 0.1) 兜底 `<0.1%`。 */
export function formatPct(p: number): string {
  return p > 0 && p < 0.1 ? "<0.1%" : `${p.toFixed(1)}%`;
}

const fmtValue = (metric: FlowMetric, v: number) => (metric === "cost" ? formatMoney(v) : formatNumber(Math.round(v)));

export function FlowChart({ columns, rows, metric }: FlowChartProps) {
  const { t } = useTranslation();
  const val = (r: FlowChartRow) => Number(r[metric]) || 0;

  // 每列：id 总量 → Top N，其余归并 OTHER
  const cols = columns.map(c => {
    const totals = new Map<string, number>();
    for (const r of rows) totals.set(String(r[c.key]), (totals.get(String(r[c.key])) ?? 0) + val(r));
    const sorted = [...totals.entries()].sort((a, b) => b[1] - a[1]);
    const top = new Set(sorted.slice(0, TOP_N).map(e => e[0]));
    const nodes = new Map<string, number>();
    for (const [id, v] of sorted) {
      const k = top.has(id) ? id : OTHER;
      nodes.set(k, (nodes.get(k) ?? 0) + v);
    }
    const ids = [...nodes.keys()];
    const sum = [...nodes.values()].reduce((s, v) => s + v, 0);
    return { c, top, nodes, ids, sum };
  });
  const rowsMax = Math.max(1, ...cols.map(c => c.ids.length));
  const H = rowsMax * PITCH + PAD * 2;
  const W = columns.length * COL_W;
  const y = (ci: number, id: string) => PAD + cols[ci].ids.indexOf(id) * PITCH + 10;
  const x = (ci: number) => ci * COL_W + 4;
  const nodeOf = (ci: number, r: FlowChartRow) => {
    const id = String(r[columns[ci].key]);
    return cols[ci].top.has(id) ? id : OTHER;
  };
  const label = (ci: number, id: string) => (id === OTHER ? t("group.flow.other", "其他") : (columns[ci].labelOf?.(id) ?? id));
  const grand = Math.max(...cols.map(c => c.sum), 0);

  return (
    <div data-testid="flow-chart" style={{ overflowX: "auto" }}>
      <div style={{ display: "flex", minWidth: W, fontSize: 10.5, color: "var(--text-secondary)" }}>
        {columns.map(c => <div key={c.key} style={{ width: COL_W, paddingInlineStart: 12 }}>{c.title}</div>)}
      </div>
      <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`} style={{ direction: "ltr", fontFamily: "var(--font-mono, monospace)" }} role="img">
        {columns.slice(0, -1).map((_, ci) => {
          const links = new Map<string, number>();
          for (const r of rows) {
            const k = `${nodeOf(ci, r)}\u0000${nodeOf(ci + 1, r)}`;
            links.set(k, (links.get(k) ?? 0) + val(r));
          }
          return [...links.entries()].map(([k, v]) => {
            const [a, b] = k.split("\u0000");
            const y1 = y(ci, a), y2 = y(ci + 1, b);
            const x1 = x(ci) + 6, x2 = x(ci + 1), mx = (x1 + x2) / 2;
            return (
              <path key={`${ci}-${k}`} d={`M ${x1} ${y1} C ${mx} ${y1}, ${mx} ${y2}, ${x2} ${y2}`} fill="none"
                stroke={seriesColor(cols[ci].ids.indexOf(a))} strokeWidth={Math.max(1, grand > 0 ? (v / grand) * 14 : 1)}
                opacity={0.4} strokeLinecap="round" />
            );
          });
        })}
        {cols.map((col, ci) => <g key={columns[ci].key} data-testid={`flow-col-${columns[ci].key}`}>{col.ids.map(id => {
          const v = col.nodes.get(id) ?? 0;
          const pct = col.sum > 0 ? (v / col.sum) * 100 : null;
          const ny = y(ci, id);
          return (
            <g key={`${ci}-${id}`} data-testid={`flow-node-${columns[ci].key}-${id}`}>
              <rect x={x(ci)} y={ny - 10} width={6} height={20} rx={2} fill={seriesColor(col.ids.indexOf(id))} />
              <text x={x(ci) + 12} y={ny - 1} fontSize={10.5} fill="var(--foreground)">{label(ci, id).slice(0, 20)}<title>{label(ci, id)}</title></text>
              <text x={x(ci) + 12} y={ny + 11} fontSize={10} fill="var(--text-tertiary)">
                {fmtValue(metric, v)}
                {pct !== null && <tspan data-testid="flow-pct" data-value={pct} dx={6}>{formatPct(pct)}</tspan>}
              </text>
            </g>
          );
        })}</g>)}
      </svg>
    </div>
  );
}
