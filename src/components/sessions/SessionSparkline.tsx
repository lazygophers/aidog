// 「正在执行」区行内迷你 token 走势（票 12 / 15 / 16 / 18 / 22）：最近 5 分钟按绝对时间对齐的 5 秒桶（入 + 出 + 缓存读写，
// 记在调用时间所在桶），桶值 = 桶内总和 ÷ 5（tokens/s），经高斯平滑后用单调三次插值画曲线；悬浮显示指针所在桶区间的时分秒（票 19）与原始平均速度。
// 全部置顶行共用一个对齐 5 秒整的计时器滑动窗口（页面隐藏时暂停）。数据复用会话详情的时间序列，会话有新调用（lastCallMs 变）才重读。
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { clientUsageApi, type ClientUsageTokenPoint } from "../../services/api/clientUsage";
import { formatClockTime, formatNumber } from "../../utils/formatters";

const BUCKET_MS = 5000;
const N = 60; // 60 × 5 秒 = 近 300 秒
const H = 24;

const total = (p: ClientUsageTokenPoint) => p.inputTokens + p.outputTokens + p.cacheReadTokens + p.cacheWriteTokens;

/** 以 nowMs 所在 5 秒桶（按绝对时间对齐 5 秒整）为最后一格，往前 60 桶逐桶求 token 总和；窗口外的调用丢弃。 */
export function windowBuckets(points: ClientUsageTokenPoint[], nowMs: number): number[] {
  const first = Math.floor(nowMs / BUCKET_MS) - (N - 1);
  const sums = new Array<number>(N).fill(0);
  for (const p of points) {
    const i = Math.floor(p.callTimeMs / BUCKET_MS) - first;
    if (i >= 0 && i < N) sums[i] += total(p);
  }
  return sums;
}

// 高斯核 σ=1 桶、半径 3 桶（等效原逐秒的 σ=5 秒、半径 15 秒）：单次调用的尖峰摊成平滑鼓包；核权重和为 1，窗口内部总量守恒（两端截断处略漏）。
const SIGMA = 1;
const RADIUS = 3;
const KERNEL = (() => {
  const w = Array.from({ length: 2 * RADIUS + 1 }, (_, k) => Math.exp(-((k - RADIUS) ** 2) / (2 * SIGMA * SIGMA)));
  const s = w.reduce((a, b) => a + b, 0);
  return w.map((x) => x / s);
})();

export function smoothSeries(v: number[]): number[] {
  return v.map((_, i) => {
    let acc = 0;
    for (let k = -RADIUS; k <= RADIUS; k++) {
      const j = i + k;
      if (j >= 0 && j < v.length) acc += v[j] * KERNEL[k + RADIUS];
    }
    return acc;
  });
}

/** 第 i 桶中心在 viewBox 的 y：留上下 1.5 / 0.5 余量，描边不被裁。 */
const yOf = (v: number, max: number, h: number) => h - 0.5 - (v / max) * (h - 2);
const f = (n: number) => String(+n.toFixed(2));

/**
 * 单调三次插值（Fritsch–Carlson 族，斜率取法同 d3 curveMonotoneX，等距 x）：内点斜率 =
 * (sign δ0 + sign δ1)·min(|δ0|, |δ1|, |(δ0+δ1)/2|/2)，异号为 0；控制点不越出相邻两点之间，曲线不过冲、不低于基线。
 */
export function curvePath(values: number[], max: number, h: number): string {
  const ys = values.map((v) => yOf(v, max, h));
  const n = ys.length;
  const d = ys.slice(1).map((y, k) => y - ys[k]);
  const m = ys.map((_, k) => {
    if (k === 0) return d[0] ?? 0;
    if (k === n - 1) return d[n - 2];
    const a = d[k - 1];
    const b = d[k];
    return (Math.sign(a) + Math.sign(b)) * Math.min(Math.abs(a), Math.abs(b), 0.5 * Math.abs((a + b) / 2));
  });
  let p = `M${f(0.5)},${f(ys[0])}`;
  for (let k = 0; k < n - 1; k++) {
    const x = k + 0.5;
    p += `C${f(x + 1 / 3)},${f(ys[k] + m[k] / 3)} ${f(x + 2 / 3)},${f(ys[k + 1] - m[k + 1] / 3)} ${f(x + 1)},${f(ys[k + 1])}`;
  }
  return p;
}

// 共享计时器：有订阅者且页面可见时才跑；单次 setTimeout 对齐到下一个 5 秒整再续排，窗口恰在桶跳变时前移、不累积漂移。
const listeners = new Set<() => void>();
let nowMs = Date.now();
let timer: ReturnType<typeof setTimeout> | undefined;
const tick = () => {
  nowMs = Date.now();
  timer = setTimeout(tick, BUCKET_MS - (nowMs % BUCKET_MS));
  for (const l of listeners) l();
};
const sync = () => {
  const run = listeners.size > 0 && document.visibilityState !== "hidden";
  if (run && !timer) tick();
  if (!run && timer) { clearTimeout(timer); timer = undefined; }
};
const subscribe = (l: () => void) => {
  if (listeners.size === 0) document.addEventListener("visibilitychange", sync);
  listeners.add(l);
  sync();
  return () => {
    listeners.delete(l);
    if (listeners.size === 0) document.removeEventListener("visibilitychange", sync);
    sync();
  };
};
const useNowMs = () => useSyncExternalStore(subscribe, () => nowMs);

// b0..b1：指针所在像素对齐后的桶区间 [b0, b1)；i：辅助线 / 点所在桶（区间中间格）
type Hover = { b0: number; b1: number; i: number; x: number; top: number; bottom: number };

export function SessionSparkline({ clientCode, sessionId, lastCallMs }: { clientCode: string; sessionId: string; lastCallMs: number }) {
  const { t } = useTranslation();
  const [points, setPoints] = useState<ClientUsageTokenPoint[]>([]);
  const [hover, setHover] = useState<Hover | null>(null);
  const tipRef = useRef<HTMLDivElement>(null);
  const now = useNowMs();
  useEffect(() => {
    let alive = true;
    clientUsageApi.sessionDetail(clientCode, sessionId, "points").then(
      (d) => { if (alive) setPoints(d.points); },
      () => {},
    );
    return () => { alive = false; };
  }, [clientCode, sessionId, lastCallMs]);
  // tooltip 横向跟指针、纵向放在整行上方（不压标题 / token 列），右侧放不下翻到左侧、顶部放不下放到下方，并夹在视口内
  useLayoutEffect(() => {
    const el = tipRef.current;
    if (!el || !hover) return;
    const { width: w, height: h } = el.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let left = hover.x + 12;
    if (left + w > vw - 4) left = hover.x - 12 - w;
    let top = hover.top - h - 24;
    if (top < 4) top = hover.bottom + 24;
    el.style.left = `${Math.max(4, Math.min(left, vw - w - 4))}px`;
    el.style.top = `${Math.max(4, Math.min(top, vh - h - 4))}px`;
  });
  const sums = windowBuckets(points, now);
  const sum = sums.reduce((a, b) => a + b, 0);
  const bars = sums.map((v) => v / (BUCKET_MS / 1000));
  const smooth = smoothSeries(bars);
  const max = Math.max(1e-9, ...smooth);
  const curve = sum > 0 ? curvePath(smooth, max, H) : "";
  const hx = hover ? (hover.b0 + hover.b1) / 2 : 0;
  const hy = hover ? yOf(smooth[hover.i], max, H) : 0;
  // 区间起止时刻 = 桶边界：末桶 = now 所在桶，与曲线同一时间基准；止为区间末桶的结束边界
  const firstMs = (Math.floor(now / BUCKET_MS) - (N - 1)) * BUCKET_MS;
  const startMs = hover ? firstMs + hover.b0 * BUCKET_MS : 0;
  const endMs = hover ? firstMs + hover.b1 * BUCKET_MS : 0;
  const rate = hover ? bars.slice(hover.b0, hover.b1).reduce((a, b) => a + b, 0) / (hover.b1 - hover.b0) : 0;
  return (
    <>
      <svg
        height={H}
        viewBox={`0 0 ${N} ${H}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={t("sessions.spark5mLabel", "Tokens in the last 5 minutes, {{n}} total", { n: formatNumber(sum) })}
        className="w-14 text-[var(--primary)] @md:w-40"
        data-testid="sessions-row-spark"
        data-bars={String(bars.length)}
        data-total={String(sum)}
        onMouseMove={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          if (r.width <= 0) return;
          // 一像素 ≤1 桶时取像素中心所在桶（跨桶边界的像素也恰 5 秒）；窄宽一像素覆盖多桶时，覆盖区间向外对齐到桶边界求平均
          const px = Math.floor(e.clientX - r.left);
          const wide = r.width >= N;
          const b0 = Math.max(0, Math.min(N - 1, Math.floor(((px + (wide ? 0.5 : 0)) * N) / r.width)));
          const b1 = wide ? b0 + 1 : Math.max(b0 + 1, Math.min(N, Math.ceil(((px + 1) * N) / r.width)));
          setHover({ b0, b1, i: Math.floor((b0 + b1 - 1) / 2), x: e.clientX, top: r.top, bottom: r.bottom });
        }}
        onMouseLeave={() => setHover(null)}
        onClick={(e) => e.stopPropagation()}
      >
        <path d={`M0 ${H - 0.5}H${N}`} stroke="currentColor" strokeOpacity={0.25} vectorEffect="non-scaling-stroke" />
        {curve && (
          <>
            <path d={`${curve}L${N - 0.5},${H}L0.5,${H}Z`} fill="currentColor" fillOpacity={0.15} stroke="none" />
            <path data-testid="sessions-spark-curve" d={curve} fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
          </>
        )}
        {hover && (
          <>
            <path data-testid="sessions-spark-guide" d={`M${hx} 0V${H}`} stroke="currentColor" strokeOpacity={0.6} strokeWidth={1} vectorEffect="non-scaling-stroke" />
            {/* 零长线段 + 圆端点 = 屏幕空间正圆（viewBox 非等比缩放下 circle 会被压扁） */}
            <path d={`M${hx},${f(hy)}h0`} stroke="currentColor" strokeWidth={5} strokeLinecap="round" vectorEffect="non-scaling-stroke" />
          </>
        )}
      </svg>
      {hover && createPortal(
        <div
          ref={tipRef}
          data-testid="sessions-spark-tip"
          data-rate={String(rate)}
          data-span={`${hover.b0}-${hover.b1}`}
          data-start={String(startMs)}
          data-end={String(endMs)}
          className="pointer-events-none fixed z-50 whitespace-nowrap rounded-md border bg-popover px-2 py-1 text-sm text-popover-foreground shadow-md"
          style={{ left: hover.x + 12, top: hover.top - 64 }}
        >
          <div className="font-mono tabular-nums text-muted-foreground">
            <bdi dir="ltr">{`${formatClockTime(startMs)}–${formatClockTime(endMs)}`}</bdi>
          </div>
          <div className="font-mono tabular-nums">{t("sessions.sparkRate", "{{n}} tokens/s", { n: formatNumber(rate) })}</div>
        </div>,
        document.body,
      )}
    </>
  );
}
