// FlowChart — 任意列数的桑基流向图（请求流向票 04；首页票 05 复用）。
// 每列 Top 6 + 「其他」；节点高度 ∝ 度量值，连线宽度 ∝ 流量并在节点处对齐堆叠；值为 0 的节点/连线不画。
// 行是扁平记录：columns[i].key 取节点 id，row[metric] 取数值。
// 动效：入场左→右依次展开、度量切换平滑过渡（JS 补间，WebKit 不支持 CSS `d` 过渡）、hover 高亮整条链路 + 数值 tooltip；
// prefers-reduced-motion 下去掉补间/位移/回弹，改纯淡入 + 更慢更淡的流光。
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { formatMoney, formatNumber } from "../../utils/formatters";
import { useThemeMode } from "../../themes/useThemeMode";

export type FlowMetric = "tokens" | "requests" | "cost";
export type FlowChartRow = Record<string, string | number>;
export interface FlowChartColumn {
  key: string;
  title: string;
  /** 节点 id → 显示名；缺省原样显示 id（空 id 显示「未指定」）。 */
  labelOf?: (id: string) => string;
}
export interface FlowChartProps {
  columns: FlowChartColumn[];
  rows: FlowChartRow[];
  metric: FlowMetric;
  /** 标签描边（halo）色，应与所在卡片底色一致；缺省 var(--card)。 */
  surface?: string;
}

const TOP_N = 6;
const OTHER = "__other__";
const COL_GAP = 190; // 相邻列节点左缘间距
const NODE_W = 10;
const LABEL_W = 200; // 末列标签留白
const TOP = 22; // 列标题高度
const INNER_H = 260; // 最高一列的绘图高度
const NODE_GAP = 10;
const MIN_H = 3;
const DUR = 450;
const STAGGER = 140;
const TOKEN_COLORS = ["var(--chart-1)", "var(--chart-3)", "var(--chart-4)", "var(--chart-2)"];
const OTHER_COLOR = "var(--text-tertiary)";

/** 占比文案：1 位小数，(0, 0.1) 兜底 `<0.1%`。 */
export function formatPct(p: number): string {
  return p > 0 && p < 0.1 ? "<0.1%" : `${p.toFixed(1)}%`;
}

/** 带状路径：上下两条三次贝塞尔边（控制点取水平间距 50%）+ 首尾闭合；入口/出口与节点条边缘对齐。 */
const ribbon = (x1: number, x2: number, ya: number, yb: number, w: number) => {
  const m = (x1 + x2) / 2, h = w / 2;
  return `M ${x1} ${ya - h} C ${m} ${ya - h}, ${m} ${yb - h}, ${x2} ${yb - h} L ${x2} ${yb + h} C ${m} ${yb + h}, ${m} ${ya + h}, ${x1} ${ya + h} Z`;
};
// 流光：沿中线移动的淡色亮段，周期与流量无关；hover 链路加快加亮；reduced-motion 周期拉长、更淡、hover 不加速。
const CSS = `@keyframes fc-flow{from{stroke-dashoffset:0}to{stroke-dashoffset:-100}}
.fc-shine{stroke-opacity:.14;animation:fc-flow 4.5s linear infinite;transition:stroke-opacity .18s}
.fc-shine.fc-hot{stroke-opacity:.5;animation-duration:1.6s}
@keyframes fc-fade{from{opacity:0}to{opacity:1}}
@media (prefers-reduced-motion: reduce){.fc-shine,.fc-shine.fc-hot{stroke-opacity:.07;animation-duration:9s}.fc-in{animation:fc-fade .25s ease-out backwards}}`;

const fmtValue = (metric: FlowMetric, v: number) => (metric === "cost" ? formatMoney(v) : formatNumber(Math.round(v)));

interface LNode { key: string; ci: number; id: string; v: number; y: number; h: number; color: string }
interface LLink { key: string; ci: number; a: string; b: string; v: number; y1: number; y2: number; w: number; color: string; color2: string }

/** 相邻两列间的交叉数：边 (a1→b1)、(a2→b2) 满足 a1<a2 且 b1>b2 即交叉（节点内出入口已按对端次序堆叠，节点级即边粒度）。 */
export function countCrossings(order: string[][], edges: Map<string, number>[]): number {
  let n = 0;
  edges.forEach((m, ci) => {
    const es = [...m.keys()].map(k => { const [a, b] = k.split("\u0000"); return [order[ci].indexOf(a), order[ci + 1].indexOf(b)]; });
    for (let i = 0; i < es.length; i++) for (let j = i + 1; j < es.length; j++) {
      if ((es[i][0] - es[j][0]) * (es[i][1] - es[j][1]) < 0) n++;
    }
  });
  return n;
}

/** 重心法减少交叉：左右交替扫描 + 相邻互换，各列按相邻列连接节点位置的流量加权均值重排；「其他」恒在列底；返回交叉数最少的次序（含初始，不会变差）。 */
export function orderColumns(init: string[][], edges: Map<string, number>[], rounds = 8): string[][] {
  let best = init.map(c => [...c]);
  let bestX = countCrossings(best, edges);
  let cur = best.map(c => [...c]);
  const sweep = (ci: number, from: number) => {
    const pos = new Map(cur[from].map((id, i) => [id, i]));
    const m = edges[Math.min(ci, from)];
    const acc = new Map<string, [number, number]>();
    for (const [k, v] of m) {
      const [a, b] = k.split("\u0000");
      const [me, other] = from < ci ? [b, a] : [a, b];
      const e = acc.get(me) ?? [0, 0];
      acc.set(me, [e[0] + v * (pos.get(other) ?? 0), e[1] + v]);
    }
    const old = new Map(cur[ci].map((id, i) => [id, i]));
    const bary = (id: string) => { const e = acc.get(id); return e && e[1] > 0 ? e[0] / e[1] : old.get(id)!; };
    const body = cur[ci].filter(id => id !== OTHER).sort((x, y) => bary(x) - bary(y) || old.get(x)! - old.get(y)!);
    cur[ci] = cur[ci].includes(OTHER) ? [...body, OTHER] : body;
  };
  for (let r = 0; r < rounds; r++) {
    const before = JSON.stringify(cur);
    for (let ci = 1; ci < cur.length; ci++) sweep(ci, ci - 1);
    for (let ci = cur.length - 2; ci >= 0; ci--) sweep(ci, ci + 1);
    // transpose：相邻节点互换若降低交叉则采纳，直到无改进（重心法的局部极小补刀）
    for (let again = true; again;) {
      again = false;
      let x0 = countCrossings(cur, edges);
      for (const col of cur) for (let i = 0; i + 1 < col.length && col[i + 1] !== OTHER; i++) {
        [col[i], col[i + 1]] = [col[i + 1], col[i]];
        const x1 = countCrossings(cur, edges);
        if (x1 < x0) { x0 = x1; again = true; } else [col[i], col[i + 1]] = [col[i + 1], col[i]];
      }
    }
    const x = countCrossings(cur, edges);
    if (x < bestX) { bestX = x; best = cur.map(c => [...c]); }
    if (JSON.stringify(cur) === before) break;
  }
  return best;
}

/** 纯布局：行 → 节点（Top N + 其他，0 值不入）与连线（宽度 ∝ 值，节点处堆叠对齐）。 */
export function computeLayout(columns: FlowChartColumn[], rows: FlowChartRow[], metric: FlowMetric) {
  const val = (r: FlowChartRow) => Number(r[metric]) || 0;
  const pos = rows.filter(r => val(r) > 0);
  const cols = columns.map(c => {
    const totals = new Map<string, number>();
    for (const r of pos) totals.set(String(r[c.key]), (totals.get(String(r[c.key])) ?? 0) + val(r));
    const sorted = [...totals.entries()].sort((a, b) => b[1] - a[1]);
    const top = new Set(sorted.slice(0, TOP_N).map(e => e[0]));
    const nodes = new Map<string, number>();
    for (const [id, v] of sorted) {
      const k = top.has(id) ? id : OTHER;
      nodes.set(k, (nodes.get(k) ?? 0) + v);
    }
    // 其他恒排末位
    const ids = [...nodes.keys()].sort((a, b) => (a === OTHER ? 1 : b === OTHER ? -1 : (nodes.get(b) ?? 0) - (nodes.get(a) ?? 0)));
    return { top, nodes, ids, sum: [...nodes.values()].reduce((s, v) => s + v, 0) };
  });
  const total = Math.max(0, ...cols.map(c => c.sum));
  const nodeOf = (ci: number, r: FlowChartRow) => {
    const id = String(r[columns[ci].key]);
    return cols[ci].top.has(id) ? id : OTHER;
  };
  const rank = cols.map(c => new Map(c.ids.map((id, i) => [id, i]))); // 配色按流量名次，重排不换色
  const edges = columns.slice(0, -1).map((_c, ci) => {
    const m = new Map<string, number>();
    for (const r of pos) { const key = `${nodeOf(ci, r)}\u0000${nodeOf(ci + 1, r)}`; m.set(key, (m.get(key) ?? 0) + val(r)); }
    return m;
  });
  orderColumns(cols.map(c => c.ids), edges).forEach((ids, ci) => { cols[ci].ids = ids; });
  const maxN = Math.max(1, ...cols.map(c => c.ids.length));
  const k = total > 0 ? (INNER_H - (maxN - 1) * NODE_GAP) / total : 0;
  const colH = cols.map(c => c.ids.reduce((s, id) => s + Math.max(MIN_H, (c.nodes.get(id) ?? 0) * k), 0) + Math.max(0, c.ids.length - 1) * NODE_GAP);
  const maxH = Math.max(0, ...colH);
  const nodes: LNode[] = [];
  const nmap = new Map<string, LNode>();
  cols.forEach((c, ci) => {
    let y = TOP + (maxH - colH[ci]) / 2;
    c.ids.forEach(id => {
      const v = c.nodes.get(id) ?? 0;
      const h = Math.max(MIN_H, v * k);
      const n: LNode = { key: `${ci}|${id}`, ci, id, v, y, h, color: id === OTHER ? OTHER_COLOR : TOKEN_COLORS[rank[ci].get(id)! % TOKEN_COLORS.length] };
      nodes.push(n);
      nmap.set(n.key, n);
      y += h + NODE_GAP;
    });
  });
  const links: LLink[] = [];
  for (let ci = 0; ci < columns.length - 1; ci++) {
    const agg = new Map<string, number>();
    for (const r of pos) {
      const key = `${nodeOf(ci, r)}\u0000${nodeOf(ci + 1, r)}`;
      agg.set(key, (agg.get(key) ?? 0) + val(r));
    }
    const ls: LLink[] = [...agg.entries()].map(([key, v]) => {
      const [a, b] = key.split("\u0000");
      return { key: `${ci}|${key}`, ci, a, b, v, y1: 0, y2: 0, w: Math.max(1, v * k), color: nmap.get(`${ci}|${a}`)!.color, color2: nmap.get(`${ci + 1}|${b}`)!.color };
    });
    const ia = (id: string, c: number) => cols[c].ids.indexOf(id);
    const out = new Map<string, number>();
    for (const l of [...ls].sort((p, q) => ia(p.a, ci) - ia(q.a, ci) || ia(p.b, ci + 1) - ia(q.b, ci + 1))) {
      const n = nmap.get(`${ci}|${l.a}`)!;
      const off = out.get(n.key) ?? 0;
      l.y1 = n.y + off + l.w / 2;
      out.set(n.key, off + l.w);
    }
    const inn = new Map<string, number>();
    for (const l of [...ls].sort((p, q) => ia(p.b, ci + 1) - ia(q.b, ci + 1) || ia(p.a, ci) - ia(q.a, ci))) {
      const n = nmap.get(`${ci + 1}|${l.b}`)!;
      const off = inn.get(n.key) ?? 0;
      l.y2 = n.y + off + l.w / 2;
      inn.set(n.key, off + l.w);
    }
    links.push(...ls);
  }
  return { cols, nodes, links, total };
}

// easeOutBack，轻微回弹（c1 小，过冲约 2%）
const ease = (p: number) => { const c1 = 0.5, c3 = c1 + 1; return 1 + c3 * Math.pow(p - 1, 3) + c1 * Math.pow(p - 1, 2); };
const hasMedia = () => typeof window !== "undefined" && typeof window.matchMedia === "function";
const prefersReduced = () => hasMedia() && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const canAnimate = () => hasMedia() && typeof requestAnimationFrame === "function" && !prefersReduced();

/** 补间：n: 键 [y,h]，l: 键 [y1,y2,w]；新键从零高/零宽长出（带列延迟），旧键直接消失。 */
function useTween(target: Map<string, number[]>, delayOf: (key: string) => number) {
  const [cur, setCur] = useState<Map<string, number[]>>(() => (canAnimate() ? new Map() : target));
  const curRef = useRef(cur);
  const delayRef = useRef(delayOf);
  delayRef.current = delayOf;
  useEffect(() => {
    if (!canAnimate()) { curRef.current = target; setCur(target); return; }
    const from = curRef.current;
    const zero = (key: string, a: number[]) => (key.startsWith("n:") ? [a[0], 0] : [a[0], a[1], 0]);
    const delays = new Map<string, number>();
    let maxDelay = 0;
    for (const key of target.keys()) {
      const d = from.has(key) ? 0 : delayRef.current(key);
      delays.set(key, d);
      maxDelay = Math.max(maxDelay, d);
    }
    const t0 = performance.now();
    let raf = 0;
    const step = (now: number) => {
      const el = now - t0;
      const next = new Map<string, number[]>();
      for (const [key, to] of target) {
        const f = from.get(key) ?? zero(key, to);
        const p = ease(Math.min(1, Math.max(0, (el - (delays.get(key) ?? 0)) / DUR)));
        next.set(key, to.map((x, i) => f[i] + (x - f[i]) * p));
      }
      curRef.current = next;
      setCur(next);
      if (el < DUR + maxDelay) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [target]);
  return cur;
}

type Hover = { kind: "node"; key: string } | { kind: "link"; key: string };

export function FlowChart({ columns, rows, metric, surface = "var(--card)" }: FlowChartProps) {
  const { t } = useTranslation();
  const layout = useMemo(() => computeLayout(columns, rows, metric), [columns, rows, metric]);
  const { cols, nodes, links, total } = layout;
  const target = useMemo(() => {
    const m = new Map<string, number[]>();
    for (const n of nodes) m.set(`n:${n.key}`, [n.y, n.h]);
    for (const l of links) m.set(`l:${l.key}`, [l.y1, l.y2, l.w]);
    return m;
  }, [nodes, links]);
  const cur = useTween(target, key => (Number(key.slice(2).split("|")[0]) + (key.startsWith("n:") ? 0 : 1)) * STAGGER);
  const uid = useId().replace(/:/g, "");
  const dark = useThemeMode() === "dark";
  const [hov, setHov] = useState<Hover | null>(null);
  const [pt, setPt] = useState({ x: 0, y: 0 });

  const reduced = prefersReduced();
  const nw = columns.length;
  const W = (nw - 1) * COL_GAP + NODE_W + LABEL_W;
  const H = TOP + Math.max(0, ...nodes.map(n => n.y + n.h - TOP)) + 8;
  const nx = (ci: number) => 4 + ci * COL_GAP;
  const label = (ci: number, id: string) =>
    id === OTHER ? t("group.flow.other", "其他") : (columns[ci].labelOf?.(id) ?? (id === "" ? t("flow.model.none", "未指定") : id));

  // 高亮链路：节点 → 其上游全部连线 ∪ 下游全部连线；连线 → 自身 ∪ 源上游 ∪ 宿下游
  const lit = useMemo(() => {
    if (!hov) return null;
    const set = new Set<string>();
    const up = (nk: string) => { for (const l of links) if (`${l.ci + 1}|${l.b}` === nk && !set.has(l.key)) { set.add(l.key); up(`${l.ci}|${l.a}`); } };
    const down = (nk: string) => { for (const l of links) if (`${l.ci}|${l.a}` === nk && !set.has(l.key)) { set.add(l.key); down(`${l.ci + 1}|${l.b}`); } };
    if (hov.kind === "node") { up(hov.key); down(hov.key); }
    else {
      const l = links.find(x => x.key === hov.key);
      if (l) { set.add(l.key); up(`${l.ci}|${l.a}`); down(`${l.ci + 1}|${l.b}`); }
    }
    const ns = new Set<string>();
    for (const l of links) if (set.has(l.key)) { ns.add(`${l.ci}|${l.a}`); ns.add(`${l.ci + 1}|${l.b}`); }
    if (hov.kind === "node") ns.add(hov.key);
    return { links: set, nodes: ns };
  }, [hov, links]);

  const move = (e: React.MouseEvent) => setPt({ x: e.clientX, y: e.clientY });
  const pct = (ci: number, v: number) => (cols[ci].sum > 0 ? (v / cols[ci].sum) * 100 : 0);

  const tip = (() => {
    if (!hov) return null;
    if (hov.kind === "node") {
      const n = nodes.find(x => x.key === hov.key);
      return n ? { title: label(n.ci, n.id), v: n.v, p: pct(n.ci, n.v) } : null;
    }
    const l = links.find(x => x.key === hov.key);
    return l ? { title: `${label(l.ci, l.a)} → ${label(l.ci + 1, l.b)}`, v: l.v, p: total > 0 ? (l.v / total) * 100 : 0 } : null;
  })();

  if (nodes.length === 0) return <div data-testid="flow-chart" />;

  return (
    <div data-testid="flow-chart" data-motion={reduced ? "reduced" : "full"} style={{ overflowX: "auto" }}>
      <svg width="100%" viewBox={`0 0 ${W} ${H}`} style={{ direction: "ltr", minWidth: W, fontFamily: "var(--font-mono, monospace)" }} role="img"
        onMouseLeave={() => setHov(null)}>
        {columns.map((c, ci) => (
          <text key={c.key} x={nx(ci)} y={12} fontSize={14} fill="var(--text-secondary)">{c.title}</text>
        ))}
        <style>{CSS}</style>
        <defs>
          {links.map((l, i) => {
            const a = cur.get(`l:${l.key}`);
            if (!a) return null;
            const x1 = nx(l.ci) + NODE_W, x2 = nx(l.ci + 1);
            return (
              <g key={l.key}>
                <linearGradient id={`${uid}g${i}`} gradientUnits="userSpaceOnUse" x1={x1} y1={0} x2={x2} y2={0}>
                  <stop offset="0" stopColor={l.color} />
                  <stop offset="1" stopColor={l.color2} />
                </linearGradient>
                <clipPath id={`${uid}c${i}`}><path d={ribbon(x1, x2, a[0], a[1], a[2])} /></clipPath>
              </g>
            );
          })}
        </defs>
        <g data-testid="flow-links" shapeRendering="geometricPrecision">
          {links.map((l, i) => {
            const a = cur.get(`l:${l.key}`);
            if (!a) return null;
            const x1 = nx(l.ci) + NODE_W, x2 = nx(l.ci + 1), mx = (x1 + x2) / 2;
            const on = lit ? lit.links.has(l.key) : null;
            const base = dark ? 0.42 : 0.34;
            const alpha = on === null ? base : on ? 0.72 : 0.07;
            return (
              <g key={l.key} data-testid={`flow-link-${columns[l.ci].key}-${l.a}-${l.b}`} data-value={l.v} data-width={l.w}
                data-state={on === null ? "idle" : on ? "on" : "off"} className={reduced ? "fc-in" : undefined}
                style={{ cursor: "pointer", ...(reduced ? { animationDelay: `${(l.ci + 1) * 60}ms` } : {}) }}
                onMouseEnter={() => setHov({ kind: "link", key: l.key })} onMouseMove={move}>
                <path d={ribbon(x1, x2, a[0], a[1], a[2])} fill={`url(#${uid}g${i})`} style={{ fillOpacity: alpha, transition: "fill-opacity .18s" }} />
                <g clipPath={`url(#${uid}c${i})`} style={{ opacity: on === false ? 0 : 1 }}>
                  <path data-testid="flow-shine" className={`fc-shine${on ? " fc-hot" : ""}`} pathLength={100} d={`M ${x1} ${a[0]} C ${mx} ${a[0]}, ${mx} ${a[1]}, ${x2} ${a[1]}`}
                    fill="none" stroke="#fff" strokeWidth={a[2] + 4} strokeDasharray="9 91" style={{ animationDelay: `${-((i * 1.37) % 4.5)}s` }} />
                </g>
              </g>
            );
          })}
        </g>
        {columns.map((_c, ci) => <g key={columns[ci].key} data-testid={`flow-col-${columns[ci].key}`}>{nodes.filter(n => n.ci === ci).map(n => {
          const a = cur.get(`n:${n.key}`) ?? [n.y, n.h];
          const p = pct(ci, n.v);
          const on = lit ? lit.nodes.has(n.key) : null;
          const cy = a[0] + a[1] / 2;
          const two = n.h >= 26;
          const grow = Math.min(1, Math.max(0, a[1] / n.h)); // 标签随节点条生长淡入
          return (
            <g key={n.key} data-testid={`flow-node-${columns[ci].key}-${n.id}`} data-value={n.v} data-height={n.h}
              data-state={on === null ? "idle" : on ? "on" : "off"}
              className={reduced ? "fc-in" : undefined}
              style={{ opacity: on === false ? 0.35 : 1, transition: "opacity .18s", cursor: "pointer", ...(reduced ? { animationDelay: `${ci * 60}ms` } : {}) }}
              onMouseEnter={() => setHov({ kind: "node", key: n.key })} onMouseMove={move}>
              <rect x={nx(ci)} y={a[0]} width={NODE_W} height={a[1]} rx={2} fill={n.color} />
              <text x={nx(ci) + NODE_W + 6} y={two ? cy - 2 : cy + 3.5} fontSize={14} fill="var(--foreground)" opacity={grow}
                stroke={surface} strokeWidth={4} strokeLinejoin="round" paintOrder="stroke">
                {label(ci, n.id).slice(0, 22)}<title>{label(ci, n.id)}</title>
                {!two && <tspan fill="var(--text-tertiary)" dx={6} fontSize={14}>{fmtValue(metric, n.v)}<tspan data-testid="flow-pct" data-value={p} dx={6}>{formatPct(p)}</tspan></tspan>}
              </text>
              {two && (
                <text x={nx(ci) + NODE_W + 6} y={cy + 11} fontSize={14} fill="var(--text-tertiary)" opacity={grow}
                  stroke={surface} strokeWidth={4} strokeLinejoin="round" paintOrder="stroke">
                  {fmtValue(metric, n.v)}
                  <tspan data-testid="flow-pct" data-value={p} dx={6}>{formatPct(p)}</tspan>
                </text>
              )}
            </g>
          );
        })}</g>)}
      </svg>
      {tip && createPortal(
        <div data-testid="flow-tooltip" style={{
          position: "fixed", left: pt.x + 12, top: pt.y + 12, zIndex: 60, pointerEvents: "none", maxWidth: 280,
          background: "var(--popover)", color: "var(--popover-foreground)", border: "1px solid var(--border)",
          borderRadius: 6, padding: "6px 9px", fontSize: 14, boxShadow: "0 8px 24px -8px rgba(0,0,0,.45)",
        }}>
          <div style={{ wordBreak: "break-all" }}>{tip.title}</div>
          <div style={{ fontFamily: "var(--font-mono, monospace)", color: "var(--text-secondary)" }}>
            {fmtValue(metric, tip.v)} · {formatPct(tip.p)}
          </div>
        </div>,
        document.body,
      )}
    </div>
  );
}
