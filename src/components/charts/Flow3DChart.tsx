// Flow3DChart — 请求流向 3D 立体交叉图（桑基的可切换替代视图，选择不持久化）。
// 设计真值源：.scratch/流量-流向细节/design-demos/3D立体交叉流向.html（canvas 2D + 手写透视投影）。
// 每列一块纵深排开的半透明板；每条完整链路 = 一根穿过所有列的管道（粗细平方根映射，颜色按实际模型 Top 6，其余灰）。
// 数据与列定义复用宿主给 FlowChart 的同一份；每列 Top 6 外合并为「其他」，再按列元组聚合链路。
// 交互：拖动旋转（限位）、滚轮缩放、重置视角；悬停管道 / 节点 / 图例高亮并在左下角出信息面板。
// 动效：入场 ≤1.2s + 流动光点；prefers-reduced-motion 下静态直出。jsdom 无 canvas（getContext 为 null）时跳过绘制。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { formatMoney, formatNumber } from "../../utils/formatters";
import { useThemeMode } from "../../themes/useThemeMode";
import { formatPct, type FlowChartColumn, type FlowChartProps, type FlowChartRow, type FlowMetric } from "./FlowChart";

export const FLOW3D_OTHER = "__other__";
export interface Flow3DPathAgg { ids: string[]; value: number; share: number }

/** 每列 Top N 外合并为 FLOW3D_OTHER，再按列元组聚合完整链路；share = 占全部的百分比。值 ≤0 的行忽略。 */
export function aggregateFlowPaths(rows: FlowChartRow[], columns: FlowChartColumn[], metric: FlowMetric, topN = 6) {
  const val = (r: FlowChartRow) => Number(r[metric]) || 0;
  const idOf = (r: FlowChartRow, key: string) => String(r[key] ?? "");
  const keep = columns.map(c => {
    const m = new Map<string, number>();
    for (const r of rows) { const v = val(r); if (v > 0) m.set(idOf(r, c.key), (m.get(idOf(r, c.key)) ?? 0) + v); }
    return new Set([...m].sort((a, b) => b[1] - a[1]).slice(0, topN).map(e => e[0]));
  });
  const agg = new Map<string, Flow3DPathAgg>();
  let total = 0;
  for (const r of rows) {
    const v = val(r);
    if (v <= 0) continue;
    total += v;
    const ids = columns.map((c, i) => (keep[i].has(idOf(r, c.key)) ? idOf(r, c.key) : FLOW3D_OTHER));
    const k = JSON.stringify(ids);
    const p = agg.get(k);
    if (p) p.value += v; else agg.set(k, { ids, value: v, share: 0 });
  }
  const paths = [...agg.values()].sort((a, b) => b.value - a.value);
  paths.forEach(p => { p.share = p.value / total * 100; });
  const layers = columns.map((_, ci) => {
    const m = new Map<string, number>();
    paths.forEach(p => m.set(p.ids[ci], (m.get(p.ids[ci]) ?? 0) + p.value));
    return [...m].map(([id, value]) => ({ id, value }))
      .sort((a, b) => (a.id === FLOW3D_OTHER ? 1 : b.id === FLOW3D_OTHER ? -1 : b.value - a.value));
  });
  return { layers, paths, total };
}

// ---------- 场景几何 ----------
const LX = 270, PH = 245, PD = 200; // 层间距、板半高、板半深
const WMIN = 4, WMAX = 22, DOTS_TOTAL = 110, SPEED = 0.25, SEG = 24, CH = 4, TOP_PATHS = 8;
const Z_PATTERN = [-30, 60, -70, 40, -50, 80, 0];
// 实际模型 Top 6 的管道色（与设计稿同值，light / dark 两套）；其余灰。
const PALETTE = {
  light: ["#7A46C9", "#3A72D0", "#C8506A", "#279270", "#CC7A2E", "#1F8E9A"],
  dark: ["#A27BE6", "#6A97E6", "#E0738A", "#4DB892", "#E39A55", "#45B3BE"],
};
const GRAY = { light: "#9097A3", dark: "#7D8492" };
const SANS = '"IBM Plex Sans","IBM Plex Sans SC","PingFang SC","Hiragino Sans GB","Microsoft YaHei",system-ui,sans-serif';
const MONO = '"IBM Plex Mono",ui-monospace,SFMono-Regular,Menlo,Consolas,monospace';
const VIEW0 = { yaw: -0.42, pitch: 0.3, dist: 2600 };
const PAD = { l: 28, r: 132, t: 30, b: 18 };
const clamp = (v: number, a = 0, b = 1) => Math.max(a, Math.min(b, v));

type Pt = { sx: number; sy: number; d: number; s: number; t: number };
interface N3 { ci: number; id: string; name: string; x: number; y: number; z: number; flow: number; paths: P3[]; r: number; slot: Map<P3, number>; poly?: Pt[]; c?: Pt }
interface P3 { i: number; ids: string[]; value: number; share: number; w: number; nodes: N3[]; spans: { x: number; y: number; z: number; t: number }[][]; pj?: Pt[][]; nd: number; clock: number; color: number }
type Focus = { kind: "node"; n: N3 } | { kind: "path"; p: P3 } | { kind: "rest" };

function buildScene(agg: ReturnType<typeof aggregateFlowPaths>, nameOf: (ci: number, id: string) => string) {
  const L = agg.layers.length;
  const nodes: N3[] = [];
  const byKey = new Map<string, N3>();
  agg.layers.forEach((arr, ci) => arr.forEach((a, k) => {
    const y = arr.length === 1 ? 0 : PH * 0.85 - k * (PH * 1.7 / (arr.length - 1));
    const n: N3 = { ci, id: a.id, name: nameOf(ci, a.id), x: (ci - (L - 1) / 2) * LX, y, z: Z_PATTERN[k % Z_PATTERN.length] * 1.6, flow: 0, paths: [], r: 0, slot: new Map() };
    nodes.push(n); byKey.set(`${ci}|${a.id}`, n);
  }));
  const vmax = agg.paths[0]?.value || 1;
  const rank = new Map((agg.layers[L - 1] ?? []).filter(a => a.id !== FLOW3D_OTHER).slice(0, 6).map((a, i) => [a.id, i] as const));
  const paths: P3[] = agg.paths.map((p, i) => ({
    i, ids: p.ids, value: p.value, share: p.share, w: WMIN + (WMAX - WMIN) * Math.sqrt(p.value / vmax),
    nodes: p.ids.map((id, ci) => byKey.get(`${ci}|${id}`)!), spans: [],
    nd: Math.max(1, Math.round(p.value / (agg.total || 1) * DOTS_TOTAL)), clock: (i * 0.37) % 1, color: rank.get(p.ids[L - 1]) ?? -1,
  }));
  paths.forEach(p => p.nodes.forEach(n => { n.flow += p.value; n.paths.push(p); }));
  // 真实数据链路数可能远多于设计稿：节点内堆叠总宽超限时整体等比收窄
  const maxSum = Math.max(1, ...nodes.map(n => n.paths.reduce((s, p) => s + p.w, 0)));
  if (maxSum > 150) paths.forEach(p => { p.w = Math.max(2, p.w * 150 / maxSum); });
  nodes.forEach(n => {
    const sumW = n.paths.reduce((s, p) => s + p.w, 0);
    n.r = sumW / 2 * 1.12 + 6;
    // 节点内按相邻层平均高度排序堆叠，减少节点处自扭
    const key = (p: P3) => {
      const a = p.nodes[n.ci - 1], b = p.nodes[n.ci + 1];
      return -((a ? a.y : n.y) + (b ? b.y : n.y)) - (b ? b.y : 0) * 0.01;
    };
    let off = sumW / 2;
    n.paths.slice().sort((p, q) => key(p) - key(q)).forEach(p => { n.slot.set(p, off - p.w / 2); off -= p.w; });
  });
  // 每条链路沿 x 方向的三次贝塞尔样条采样，按跨段切 chunk
  paths.forEach(p => {
    const anchor = p.nodes.map(n => ({ x: n.x, y: n.y + n.slot.get(p)!, z: n.z }));
    for (let s = 0; s < L - 1; s++) {
      const a = anchor[s], b = anchor[s + 1], h = (b.x - a.x) * 0.5, pts = [];
      for (let k = 0; k <= SEG; k++) {
        const t = k / SEG, u = 1 - t, c0 = u * u * u, c1 = 3 * u * u * t, c2 = 3 * u * t * t, c3 = t * t * t;
        pts.push({ x: c0 * a.x + c1 * (a.x + h) + c2 * (b.x - h) + c3 * b.x, y: (c0 + c1) * a.y + (c2 + c3) * b.y, z: (c0 + c1) * a.z + (c2 + c3) * b.z, t: s + t });
      }
      p.spans.push(pts);
    }
  });
  const corners: [number, number, number][] = [];
  for (let i = 0; i < L; i++) for (const [y, z] of [[PH + 45, -PD], [PH + 45, PD], [-PH, PD], [-PH, -PD]]) corners.push([(i - (L - 1) / 2) * LX, y, z]);
  const rest = paths.slice(TOP_PATHS);
  return { L, nodes, paths, top: paths.slice(0, TOP_PATHS), rest, restValue: rest.reduce((s, p) => s + p.value, 0), total: agg.total, corners };
}
type Scene = ReturnType<typeof buildScene>;

// ---------- 绘制引擎（canvas 2D，按设计稿移植；颜色每 500ms 从主题 CSS 变量重读，主题切换自动跟随） ----------
interface Colors { dark: boolean; bg: string; text: string; muted: string; primary: string }
function readColors(host: HTMLElement, surface?: string): Colors {
  const probe = document.createElement("span");
  host.appendChild(probe);
  const rc = (v: string) => { probe.style.color = ""; probe.style.color = v; return getComputedStyle(probe).color; };
  const c = { dark: document.documentElement.dataset.mode === "dark", bg: rc(surface ?? "var(--card)"), text: rc("var(--foreground)"), muted: rc("var(--text-secondary)"), primary: rc("var(--data-primary)") };
  probe.remove();
  return c;
}
const colorOf = (p: P3, dark: boolean) => (p.color >= 0 ? PALETTE[dark ? "dark" : "light"][p.color] : GRAY[dark ? "dark" : "light"]);

interface EngineIO {
  cam: typeof VIEW0;
  colors: () => Colors;
  hl: () => Set<P3> | null;
  hoverNode: () => N3 | null;
  onHover: (f: Focus | null) => void;
  layerTitles: string[];
  fmtV: (v: number) => string;
  reserve: (w: number) => { w: number; h: number };
}

function startEngine(cv: HTMLCanvasElement, S: Scene, io: EngineIO): () => void {
  let ctx: CanvasRenderingContext2D | null = null;
  try { ctx = cv.getContext("2d"); } catch { ctx = null; }
  if (!ctx) return () => {};
  const g = ctx;
  const REDUCED = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
  const cam = io.cam, L = S.L, total = S.total || 1;
  let W = 0, H = 0, DPR = 1, F = 1, OX = 0, OY = 0, cy = 1, sy = 0, cp = 1, sp = 0, G = 0, raf = 0;
  let drag: { x: number; y: number } | null = null;
  const t0 = performance.now();
  let last = t0;
  const setRot = () => { cy = Math.cos(cam.yaw); sy = Math.sin(cam.yaw); cp = Math.cos(cam.pitch); sp = Math.sin(cam.pitch); };
  const proj = (x: number, y: number, z: number): Pt => {
    const xr = x * cy + z * sy, zr = -x * sy + z * cy, d = cam.dist - (y * sp + zr * cp), s = F / d;
    return { sx: OX + xr * s, sy: OY - (y * cp - zr * sp) * s, d, s, t: 0 };
  };
  const frame = (fit: boolean): void => {
    OX = OY = 0;
    const ps = S.corners.map(c => proj(...c));
    const xs = ps.map(p => p.sx), ys = ps.map(p => p.sy);
    const x0 = Math.min(...xs), x1 = Math.max(...xs), y0 = Math.min(...ys), y1 = Math.max(...ys);
    const aw = W - PAD.l - PAD.r, ah = H - PAD.t - PAD.b;
    if (fit) { F *= Math.min(aw / (x1 - x0), ah / (y1 - y0)); return frame(false); }
    // 偏顶对齐：底部空余留给左下角信息面板
    OX = PAD.l + aw / 2 - (x0 + x1) / 2; OY = PAD.t - y0 + Math.max(0, ah - (y1 - y0)) * 0.15;
  };
  const resize = () => {
    const r = cv.getBoundingClientRect();
    W = r.width; H = r.height;
    if (!W || !H) return;
    DPR = Math.min(2, window.devicePixelRatio || 1);
    cv.width = W * DPR; cv.height = H * DPR;
    // 左下角信息面板占位：板角要么在面板右侧、要么在面板上方，面板不压板子
    const rv = io.reserve(W), rw = rv.w + 28, lim = H - 16 - rv.h - 12;
    const keep = { ...cam };
    Object.assign(cam, VIEW0); setRot();
    F = 1; frame(true);
    for (let k = 0; k < 40 && !S.corners.every(c => { const p = proj(...c); return p.sx > rw || p.sy < lim; }); k++) { F *= 0.97; frame(false); }
    Object.assign(cam, keep);
  };

  const drawChunk = (p: P3, pts: Pt[], a: number, b: number, C: Colors, hl: Set<P3> | null) => {
    const on = !hl || hl.has(p);
    let end = b;
    while (end > a && pts[end].t > G) end--;
    if (end <= a) return;
    const w = Math.max(2, p.w * (pts[a].s * 0.5 + pts[end].s * 0.5));
    g.beginPath(); g.moveTo(pts[a].sx, pts[a].sy);
    for (let k = a + 1; k <= end; k++) g.lineTo(pts[k].sx, pts[k].sy);
    g.globalAlpha = on ? 1 : 0.12;
    // halo：与背景同色的描边，交叉处分清前后
    if (on) { g.lineCap = "butt"; g.strokeStyle = C.bg; g.lineWidth = w + 4; g.stroke(); }
    g.lineCap = on ? "round" : "butt"; g.strokeStyle = colorOf(p, C.dark); g.lineWidth = w; g.stroke();
    // 流动光点：只画落在本 chunk 内的，保持遮挡次序
    if (!REDUCED && G >= L - 1) {
      const s0 = pts[0].t, r = clamp(w * 0.2, 1.4, 2.8);
      g.fillStyle = "#FFFFFF"; g.globalAlpha = on ? 0.85 : 0.15;
      for (let k = 0; k < p.nd; k++) {
        const t = (p.clock + k * (L - 1) / p.nd) % (L - 1);
        if (t < s0 || t >= s0 + 1) continue;
        const loc = (t - s0) * SEG, j = Math.floor(loc);
        if (j < a || j >= end) continue;
        const f = loc - j, q0 = pts[j], q1 = pts[j + 1];
        g.beginPath(); g.arc(q0.sx + (q1.sx - q0.sx) * f, q0.sy + (q1.sy - q0.sy) * f, r, 0, Math.PI * 2); g.fill();
      }
    }
    g.globalAlpha = 1;
  };

  type Label = { layer?: boolean; text?: string; node?: N3; end?: P3; x: number; y?: number; top?: number; bot?: number; al: number; hot?: boolean };
  const drawLabels = (labels: Label[], C: Colors) => {
    const placed: { x: number; y: number; w: number; h: number }[] = [];
    const hit = (r: { x: number; y: number; w: number; h: number }) => r.x < 2 || r.y < 2 || r.x + r.w > W - 2 || r.y + r.h > H - 2 ||
      placed.some(q => r.x < q.x + q.w && r.x + r.w > q.x && r.y < q.y + q.h && r.y + r.h > q.y);
    // 层名 > 高亮相关 > 前 8 末端数字 > 其余节点按流量
    const pri = (l: Label) => (l.layer ? 4e9 : (l.hot ? 3e9 : l.end ? 2e9 : 0) + (l.end ? l.end.value / total : l.node!.flow / total) * 1e9);
    labels.sort((a, b) => pri(b) - pri(a));
    type Part = [string, string, string];
    const width = (parts: Part[]) => parts.reduce((s, [t, f]) => { g.font = f; return s + g.measureText(t).width; }, 0);
    const NF = `500 12px ${SANS}`, MF = `12px ${MONO}`;
    labels.forEach(l => {
      let variants: Part[][], h: number, cands: (w: number) => { x: number; y: number; w: number; h: number }[];
      if (l.layer) {
        variants = [[[l.text!, `600 15px ${SANS}`, C.primary]]]; h = 22;
        cands = w => [0, -14, 14, -28, 28].map(dy => ({ x: l.x - w / 2, y: l.y! - h + dy, w, h }));
      } else if (l.end) {
        variants = [[[`${io.fmtV(l.end.value)} · ${formatPct(l.end.share)}`, `600 12px ${MONO}`, C.text]]]; h = 20;
        cands = w => [0, 10, -10, 20, -20, 30, -30].map(dy => ({ x: l.x, y: l.y! - h / 2 + dy, w, h }));
      } else {
        const n = l.node!, nm: Part = [n.name, NF, C.text], pc = formatPct(n.flow / total * 100);
        variants = [[nm, [` · ${io.fmtV(n.flow)} · ${pc}`, MF, C.muted]], [nm, [` · ${pc}`, MF, C.muted]]]; h = 20;
        const top = l.top!, bot = l.bot!;
        cands = w => [{ x: l.x - w / 2, y: top - 3 - h, w, h }, { x: l.x - w / 2, y: top - 11 - h, w, h },
          { x: l.x - w / 2, y: bot + 3, w, h }, { x: l.x - w / 2, y: bot + 11, w, h },
          { x: l.x - w - 10, y: (top + bot) / 2 - h / 2, w, h }, { x: l.x - w - 10, y: top, w, h }];
      }
      let r: { x: number; y: number; w: number; h: number } | undefined, parts: Part[] | null = null;
      for (const v of variants) {
        r = cands(width(v) + (l.end ? 26 : 12)).find(c => !hit(c));
        if (r) { parts = v; break; }
      }
      if (!r && l.layer) { parts = variants[0]; r = cands(width(parts) + 12)[0]; } // 层名任何视角都画
      if (!r || !parts) return; // 空间不够才隐藏
      placed.push(r);
      if (!l.layer) { g.globalAlpha = l.al * 0.92; g.fillStyle = C.bg; g.beginPath(); g.roundRect(r.x, r.y, r.w, r.h, 5); g.fill(); }
      g.globalAlpha = l.al;
      g.textBaseline = "middle"; g.textAlign = "left";
      let tx = r.x + 6;
      if (l.end) { g.fillStyle = colorOf(l.end, C.dark); g.beginPath(); g.roundRect(tx, r.y + 5, 10, 10, 2); g.fill(); tx += 16; }
      parts.forEach(([t, f, col]) => { g.font = f; g.fillStyle = col; g.fillText(t, tx, r!.y + h / 2 + 1); tx += g.measureText(t).width; });
      g.globalAlpha = 1;
    });
  };

  const render = (now: number) => {
    raf = requestAnimationFrame(render);
    if (!W || !H) return;
    const el = REDUCED ? 1e9 : now - t0, C = io.colors(), hl = io.hl(), hv = io.hoverNode();
    const dt = Math.min(0.05, (now - last) / 1000); last = now;
    setRot(); frame(false);
    // 入场：板子 0~750ms 依次浮现，管道 250~1150ms 从首列生长到末列，之后静止
    G = clamp((el - 250) / 900) * (L - 1);
    if (!REDUCED && G >= L - 1) S.paths.forEach(p => { if (!hl || hl.has(p)) p.clock = (p.clock + dt * SPEED) % (L - 1); });
    const camX = -cam.dist * cp * sy;
    g.setTransform(DPR, 0, 0, DPR, 0, 0);
    g.clearRect(0, 0, W, H);
    S.paths.forEach(p => { p.pj = p.spans.map(sp_ => sp_.map(q => { const r = proj(q.x, q.y, q.z); r.t = q.t; return r; })); });
    const items: { key: number; pane?: number; span?: number }[] = [];
    for (let i = 0; i < L; i++) items.push({ key: (i - (L - 1) / 2) * LX, pane: i });
    for (let s = 0; s < L - 1; s++) items.push({ key: (s + 0.5 - (L - 1) / 2) * LX, span: s });
    items.sort((a, b) => Math.abs(b.key - camX) - Math.abs(a.key - camX));
    const labels: Label[] = [];
    items.forEach(it => {
      if (it.pane !== undefined) {
        const i = it.pane, x = (i - (L - 1) / 2) * LX, al = clamp((el - i * 80) / 350);
        if (al <= 0) return;
        const cs = [[PH, -PD], [PH, PD], [-PH, PD], [-PH, -PD]].map(([y, z]) => proj(x, y, z));
        g.beginPath(); cs.forEach((c, k) => (k ? g.lineTo(c.sx, c.sy) : g.moveTo(c.sx, c.sy))); g.closePath();
        g.globalAlpha = al * (C.dark ? 0.07 : 0.05); g.fillStyle = C.primary; g.fill();
        g.globalAlpha = al * (C.dark ? 0.38 : 0.3); g.strokeStyle = C.primary; g.lineWidth = 1; g.stroke();
        labels.push({ layer: true, x: (cs[0].sx + cs[1].sx) / 2, y: Math.min(cs[0].sy, cs[1].sy) - 6, text: io.layerTitles[i], al });
        S.nodes.filter(n => n.ci === i).map(n => ({ n, c: proj(n.x, n.y, n.z) })).sort((a, b) => b.c.d - a.c.d).forEach(({ n, c }) => {
          const on = !hl || n.paths.some(p => hl.has(p)), a0 = al * (on ? 1 : 0.25);
          n.poly = [];
          g.beginPath();
          for (let k = 0; k < 28; k++) {
            const ang = k / 28 * Math.PI * 2, q = proj(n.x, n.y + n.r * Math.cos(ang), n.z + n.r * Math.sin(ang));
            n.poly.push(q); if (k) g.lineTo(q.sx, q.sy); else g.moveTo(q.sx, q.sy);
          }
          g.closePath();
          g.globalAlpha = a0 * 0.88; g.fillStyle = C.bg; g.fill();
          const hot = hv === n;
          g.globalAlpha = a0; g.strokeStyle = hot ? C.primary : C.muted; g.lineWidth = hot ? 2.5 : 1.2; g.stroke();
          n.c = c;
          const ys = n.poly.map(q => q.sy);
          labels.push({ node: n, x: c.sx, top: Math.min(...ys), bot: Math.max(...ys), al: al * (on ? 1 : 0.35), hot: !!hl && on });
        });
        g.globalAlpha = 1;
      } else {
        const s = it.span!, chunks: { p: P3; pts: Pt[]; a: number; b: number; d: number }[] = [];
        S.paths.forEach(p => { const pts = p.pj![s]; for (let a = 0; a < SEG; a += CH) { const b = Math.min(SEG, a + CH); chunks.push({ p, pts, a, b, d: (pts[a].d + pts[b].d) / 2 }); } });
        chunks.sort((u, v) => (hl ? (Number(hl.has(u.p)) - Number(hl.has(v.p))) * 1e6 : 0) + (v.d - u.d));
        chunks.forEach(c => drawChunk(c.p, c.pts, c.a, c.b, C, hl));
      }
    });
    // 前 8 条链路：末列管道末尾常驻「数值 · 占全部%」
    const endAl = clamp((G - (L - 1.5)) / 0.5);
    if (endAl > 0) S.top.forEach(p => {
      const n = p.nodes[L - 1];
      if (!n.poly) return;
      labels.push({ end: p, x: Math.max(...n.poly.map(q => q.sx)) + 8, y: p.pj![L - 2][SEG].sy, al: endAl * (!hl || hl.has(p) ? 1 : 0.35), hot: !!hl && hl.has(p) });
    });
    drawLabels(labels, C);
  };

  const inPoly = (x: number, y: number, poly: Pt[]) => {
    let c = false;
    for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
      const a = poly[i], b = poly[j];
      if ((a.sy > y) !== (b.sy > y) && x < (b.sx - a.sx) * (y - a.sy) / (b.sy - a.sy) + a.sx) c = !c;
    }
    return c;
  };
  const segDist = (px: number, py: number, a: Pt, b: Pt) => {
    const dx = b.sx - a.sx, dy = b.sy - a.sy, l2 = dx * dx + dy * dy || 1, t = clamp(((px - a.sx) * dx + (py - a.sy) * dy) / l2);
    return Math.hypot(px - a.sx - t * dx, py - a.sy - t * dy);
  };
  const pick = (x: number, y: number): Focus | null => {
    let bn: { n: N3; d: number } | null = null;
    S.nodes.forEach(n => { if (n.poly && n.c && inPoly(x, y, n.poly) && (!bn || n.c.d < bn.d)) bn = { n, d: n.c.d }; });
    if (bn) return { kind: "node", n: (bn as { n: N3 }).n };
    let bp: { p: P3; d: number } | null = null;
    S.paths.forEach(p => p.pj?.forEach(pts => { for (let k = 0; k < pts.length - 1; k++) {
      if (segDist(x, y, pts[k], pts[k + 1]) < Math.max(3, p.w * pts[k].s / 2 + 2) && (!bp || pts[k].d < bp.d)) bp = { p, d: pts[k].d };
    } }));
    return bp ? { kind: "path", p: (bp as { p: P3 }).p } : null;
  };

  const onDown = (e: PointerEvent) => { drag = { x: e.clientX, y: e.clientY }; cv.setPointerCapture?.(e.pointerId); };
  const onMove = (e: PointerEvent) => {
    const r = cv.getBoundingClientRect();
    if (drag) {
      const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
      if (Math.abs(dx) + Math.abs(dy) > 2) {
        cv.style.cursor = "grabbing"; io.onHover(null);
        // yaw 限幅：层与层不会叠成一列，层名始终可读
        cam.yaw = clamp(cam.yaw - dx * 0.004, -1.15, 0.6); cam.pitch = clamp(cam.pitch + dy * 0.004, 0.1, 1.2);
        drag = { x: e.clientX, y: e.clientY };
      }
      return;
    }
    io.onHover(pick(e.clientX - r.left, e.clientY - r.top));
  };
  const onUp = () => { drag = null; cv.style.cursor = "grab"; };
  const onLeave = () => { if (!drag) io.onHover(null); };
  const onWheel = (e: WheelEvent) => { e.preventDefault(); cam.dist = clamp(cam.dist * Math.exp(e.deltaY * 0.001), 1300, 4600); };
  cv.addEventListener("pointerdown", onDown);
  cv.addEventListener("pointermove", onMove);
  cv.addEventListener("pointerup", onUp);
  cv.addEventListener("pointerleave", onLeave);
  cv.addEventListener("wheel", onWheel, { passive: false });
  const ro = typeof ResizeObserver !== "undefined" ? new ResizeObserver(resize) : null;
  if (ro) ro.observe(cv); else window.addEventListener("resize", resize);
  resize();
  raf = requestAnimationFrame(render);
  return () => {
    cancelAnimationFrame(raf);
    ro?.disconnect(); window.removeEventListener("resize", resize);
    cv.removeEventListener("pointerdown", onDown);
    cv.removeEventListener("pointermove", onMove);
    cv.removeEventListener("pointerup", onUp);
    cv.removeEventListener("pointerleave", onLeave);
    cv.removeEventListener("wheel", onWheel);
  };
}

// ---------- React 外壳：画布 + 左下信息面板 + 右侧链路排行图例 ----------
const sameFocus = (a: Focus | null, b: Focus | null) => !!a && !!b && a.kind === b.kind &&
  (a.kind === "node" ? a.n === (b as { n: N3 }).n : a.kind === "path" ? a.p === (b as { p: P3 }).p : true);
const shortModel = (s: string) => s.replace(/^claude-/, "");

export function Flow3DChart({ columns, rows, metric, surface }: FlowChartProps) {
  const { t } = useTranslation();
  const dark = useThemeMode() === "dark";
  const wrapRef = useRef<HTMLDivElement>(null);
  const cvRef = useRef<HTMLCanvasElement>(null);
  const camRef = useRef({ ...VIEW0 });
  const focusRef = useRef<Focus | null>(null);
  const hlRef = useRef<Set<P3> | null>(null);
  const [focus, setFocus] = useState<Focus | null>(null);

  const fmtV = useCallback((v: number) => (metric === "cost" ? formatMoney(v) : formatNumber(Math.round(v))), [metric]);
  const scene = useMemo(() => buildScene(aggregateFlowPaths(rows, columns, metric), (ci, id) => (id === FLOW3D_OTHER
    ? t("group.flow.other", "其他")
    : (columns[ci].labelOf?.(id) ?? (id === "" ? t("flow.model.none", "未指定") : id)))), [rows, columns, metric, t]);

  const applyFocus = useCallback((f: Focus | null) => {
    if (f === focusRef.current || sameFocus(f, focusRef.current)) return;
    focusRef.current = f;
    hlRef.current = f ? new Set(f.kind === "node" ? f.n.paths : f.kind === "path" ? [f.p] : scene.rest) : null;
    setFocus(f);
  }, [scene]);

  useEffect(() => {
    const cv = cvRef.current, host = wrapRef.current;
    if (!cv || !host) return;
    let colors = readColors(host, surface), readAt = performance.now();
    const stop = startEngine(cv, scene, {
      cam: camRef.current,
      colors: () => {
        if (performance.now() - readAt > 500) { colors = readColors(host, surface); readAt = performance.now(); }
        return colors;
      },
      hl: () => hlRef.current,
      hoverNode: () => (focusRef.current?.kind === "node" ? focusRef.current.n : null),
      onHover: applyFocus,
      layerTitles: columns.map(c => c.title),
      fmtV,
      reserve: w => ({ w: Math.min(420, w * 0.5), h: 150 }),
    });
    return () => { stop(); focusRef.current = null; hlRef.current = null; setFocus(null); };
  }, [scene, columns, fmtV, applyFocus, surface]);

  const L = scene.L, mid = L > 3 ? 2 : -1, total = scene.total || 1;
  const pal = (p: P3) => colorOf(p, dark);
  const chip = (c: string) => <span style={{ width: 12, height: 12, borderRadius: 3, background: c, flex: "none", display: "inline-block" }} />;
  const ofTotal = (v: number) => `${fmtV(v)} · ${t("flow.3d.ofTotal", { p: formatPct(v / total * 100), defaultValue: "占全部 {{p}}" })}`;
  const totStyle = { marginTop: 8, paddingTop: 8, borderTop: "1px solid var(--border)", fontFamily: MONO, fontWeight: 600 } as const;
  const secStyle = { marginTop: 8, color: "var(--text-secondary)", fontSize: 12 } as const;

  const pathCard = (p: P3) => {
    const half = Math.ceil(L / 2);
    return (<>
      <div style={{ fontWeight: 600, marginBottom: 6, display: "flex", alignItems: "center", gap: 8 }}>{chip(pal(p))}{t("flow.3d.rank", { n: p.i + 1, defaultValue: "完整链路 · 第 {{n}} 大" })}</div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0 16px" }}>
        {[0, half].map(o => (
          <div key={o}>{p.nodes.slice(o, o + half).map((n, i) => (
            <div key={i} style={{ display: "grid", gridTemplateColumns: "72px 1fr", gap: 8 }}>
              <span style={{ color: "var(--text-secondary)" }}>{columns[o + i].title}</span><span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{n.name}</span>
            </div>
          ))}</div>
        ))}
      </div>
      <div style={totStyle}>{ofTotal(p.value)}</div>
    </>);
  };
  const dist = (n: N3, layer: number) => {
    const m = new Map<N3, number>();
    n.paths.forEach(p => m.set(p.nodes[layer], (m.get(p.nodes[layer]) ?? 0) + p.value));
    return [...m].sort((a, b) => b[1] - a[1]).map(([tn, v]) => {
      const col = layer === L - 1 ? pal(tn.paths[0]) : "var(--data-primary)";
      return (
        <div key={tn.id} style={{ display: "grid", gridTemplateColumns: "110px 1fr 48px", gap: 8, alignItems: "center" }}>
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{tn.name}</span>
          <i style={{ display: "block", height: 8, borderRadius: 4, width: `${v / n.flow * 100}%`, background: col }} />
          <b style={{ fontFamily: MONO, fontWeight: 500, textAlign: "right" }}>{formatPct(v / n.flow * 100)}</b>
        </div>
      );
    });
  };
  const nodeCard = (n: N3) => {
    const secs: [string, number][] = n.ci < L - 1
      ? [...(mid > n.ci ? [mid] : []), L - 1].map(l => [t("flow.3d.toCol", { col: columns[l].title, defaultValue: "去往{{col}}" }), l])
      : [0, ...(L - 3 > 0 ? [L - 3] : [])].map(l => [t("flow.3d.fromCol", { col: columns[l].title, defaultValue: "来自{{col}}" }), l]);
    return (<>
      <div style={{ fontWeight: 600, marginBottom: 6, display: "flex", alignItems: "center", gap: 8 }}>{n.name}<span style={{ fontWeight: 400, fontSize: 12, color: "var(--text-secondary)" }}>{columns[n.ci].title}</span></div>
      <div style={{ fontFamily: MONO, fontWeight: 600 }}>{t("flow.3d.via", "经过此节点")} {ofTotal(n.flow)}</div>
      <div style={{ display: "grid", gridTemplateColumns: secs.length > 1 ? "1fr 1fr" : "1fr", gap: "0 16px" }}>
        {secs.map(([title, l]) => <div key={l}><div style={secStyle}>{title}</div>{dist(n, l)}</div>)}
      </div>
    </>);
  };
  const othersLabel = t("flow.3d.others", { n: scene.rest.length, defaultValue: "其他 {{n}} 条" });
  const legendIdx = [...new Set([0, mid, L - 3])].filter(i => i >= 0 && i < L - 1);
  const legendRow = (key: string, color: string, name: string, v: number, f: Focus, on: boolean) => (
    <div key={key} data-testid={`flow-3d-legend-${key}`} data-state={on ? "on" : "off"}
      onMouseEnter={() => applyFocus(f)} onMouseLeave={() => applyFocus(null)}
      style={{ display: "grid", gridTemplateColumns: "14px 1fr", gap: "2px 10px", padding: 8, borderRadius: 8, background: on ? "var(--accent-subtle)" : "transparent" }}>
      <span style={{ width: 14, height: 14, borderRadius: 3, marginTop: 3, background: color }} />
      <div style={{ fontSize: 13, lineHeight: 1.45, wordBreak: "break-word" }}>{name}</div>
      <div style={{ gridColumn: 2, fontFamily: MONO, fontSize: 13, color: "var(--text-secondary)" }}>{fmtV(v)} · {formatPct(v / total * 100)}</div>
    </div>
  );

  return (
    <div ref={wrapRef} data-testid="flow-3d" style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr) 240px", height: 500, fontSize: 13 }}>
      <div style={{ position: "relative", overflow: "hidden", minWidth: 0 }}>
        <canvas ref={cvRef} aria-label={t("flow.3d.aria", "请求流向立体图")} style={{ display: "block", width: "100%", height: "100%", cursor: "grab", touchAction: "none" }} />
        <button type="button" data-testid="flow-3d-reset" onClick={() => Object.assign(camRef.current, VIEW0)}
          style={{ position: "absolute", top: 6, right: 8, cursor: "pointer", fontSize: 13, padding: "2px 9px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)", background: "transparent", color: "var(--text-secondary)" }}>
          {t("flow.3d.reset", "重置视角")}
        </button>
        <div data-testid="flow-3d-info" role="status" style={{ position: "absolute", left: 12, bottom: 12, width: focus ? 420 : "auto", maxWidth: "calc(100% - 24px)", background: "var(--card)", border: "1px solid var(--border)", borderRadius: 10, boxShadow: "0 6px 20px rgba(0,0,0,.14)", padding: "10px 12px", lineHeight: 1.5, pointerEvents: "none", color: "var(--foreground)" }}>
          {!focus ? <div style={{ color: "var(--text-secondary)" }}>{t("flow.3d.hint", "悬停管道、节点或右侧图例查看完整链路 · 拖动旋转 · 滚轮缩放")}</div>
            : focus.kind === "path" ? pathCard(focus.p)
            : focus.kind === "node" ? nodeCard(focus.n)
            : (<><div style={{ fontWeight: 600 }}>{othersLabel}</div><div style={totStyle}>{ofTotal(scene.restValue)}</div></>)}
        </div>
      </div>
      <aside style={{ borderLeft: "1px solid var(--border)", padding: "4px 10px", overflow: "auto" }}>
        <div style={{ fontSize: 14, fontWeight: 600, margin: "4px 0 6px" }}>{t("flow.3d.legend", "链路排行")}</div>
        {scene.top.map(p => legendRow(String(p.i), pal(p),
          `${legendIdx.map(i => shortModel(p.nodes[i].name)).join(" · ")} → ${shortModel(p.nodes[L - 1].name)}`, p.value,
          { kind: "path", p }, focus?.kind === "path" && focus.p === p))}
        {scene.rest.length > 0 && legendRow("rest", GRAY[dark ? "dark" : "light"], othersLabel, scene.restValue, { kind: "rest" }, focus?.kind === "rest")}
      </aside>
    </div>
  );
}
