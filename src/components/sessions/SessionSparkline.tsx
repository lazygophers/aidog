// 「正在执行」区行内迷你 token 走势（票 12 / 15 / 16 / 18）：最近 5 分钟逐秒（入 + 出 + 缓存读写，记在调用时间所在秒），
// 逐秒值经高斯平滑后用单调三次插值画曲线；悬浮显示指针所在像素覆盖秒区间的原始平均速度。
// 全部置顶行共用一个 1 秒计时器滑动窗口（页面隐藏时暂停）。数据复用会话详情的时间序列，会话有新调用（lastCallMs 变）才重读。
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { clientUsageApi, type ClientUsageTokenPoint } from "../../services/api/clientUsage";
import { formatNumber } from "../../utils/formatters";

const SECS = 300;
const H = 24;

const total = (p: ClientUsageTokenPoint) => p.inputTokens + p.outputTokens + p.cacheReadTokens + p.cacheWriteTokens;

/** 以 nowMs 所在秒为最后一格，往前 300 秒逐秒求和；窗口外的调用丢弃。 */
export function secondBuckets(points: ClientUsageTokenPoint[], nowMs: number): number[] {
  const first = Math.floor(nowMs / 1000) - (SECS - 1);
  const bars = new Array<number>(SECS).fill(0);
  for (const p of points) {
    const i = Math.floor(p.callTimeMs / 1000) - first;
    if (i >= 0 && i < SECS) bars[i] += total(p);
  }
  return bars;
}

// 高斯核 σ=5 秒、半径 15 秒（约 31 秒窗口；窄宽 56px 时一像素约 5 秒，σ 再小曲线仍显尖）：单次调用的尖峰摊成平滑鼓包；核权重和为 1，窗口内部总量守恒（两端截断处略漏）。
const SIGMA = 5;
const RADIUS = 15;
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

/** 第 i 秒中心在 viewBox 的 y：留上下 1.5 / 0.5 余量，描边不被裁。 */
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

// 共享秒表：有订阅者且页面可见时才跑；单次 setTimeout 对齐到下一整秒再续排，窗口恰在秒跳变时前移、不累积漂移。
const listeners = new Set<() => void>();
let nowMs = Date.now();
let timer: ReturnType<typeof setTimeout> | undefined;
const tick = () => {
  nowMs = Date.now();
  timer = setTimeout(tick, 1000 - (nowMs % 1000));
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

// s0..s1：指针所在像素覆盖的秒区间 [s0, s1)；i：辅助线 / 点所在秒（区间中间格）
type Hover = { s0: number; s1: number; i: number; x: number; top: number; bottom: number };

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
  const bars = secondBuckets(points, now);
  const sum = bars.reduce((a, b) => a + b, 0);
  const smooth = smoothSeries(bars);
  const max = Math.max(1e-9, ...smooth);
  const curve = sum > 0 ? curvePath(smooth, max, H) : "";
  const hx = hover ? hover.i + 0.5 : 0;
  const hy = hover ? yOf(smooth[hover.i], max, H) : 0;
  // 时间偏移取区间最新一秒：区间含当前秒时显示「刚刚」
  const ago = hover ? SECS - hover.s1 : 0;
  const rate = hover ? bars.slice(hover.s0, hover.s1).reduce((a, b) => a + b, 0) / (hover.s1 - hover.s0) : 0;
  return (
    <>
      <svg
        height={H}
        viewBox={`0 0 ${SECS} ${H}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={t("sessions.spark5mLabel", "最近 5 分钟 token，共 {{n}}", { n: formatNumber(sum) })}
        className="w-14 text-[var(--primary)] @md:w-40"
        data-testid="sessions-row-spark"
        data-bars={String(bars.length)}
        data-total={String(sum)}
        onMouseMove={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          if (r.width <= 0) return;
          // 窄宽时一像素覆盖数秒，单秒值常为 0：取整像素覆盖的秒区间求平均；一像素 ≤1 秒时区间只含一秒
          const px = Math.floor(e.clientX - r.left);
          const s0 = Math.max(0, Math.min(SECS - 1, Math.floor((px * SECS) / r.width)));
          const s1 = Math.max(s0 + 1, Math.min(SECS, Math.floor(((px + 1) * SECS) / r.width)));
          setHover({ s0, s1, i: Math.floor((s0 + s1 - 1) / 2), x: e.clientX, top: r.top, bottom: r.bottom });
        }}
        onMouseLeave={() => setHover(null)}
        onClick={(e) => e.stopPropagation()}
      >
        <path d={`M0 ${H - 0.5}H${SECS}`} stroke="currentColor" strokeOpacity={0.25} vectorEffect="non-scaling-stroke" />
        {curve && (
          <>
            <path d={`${curve}L${SECS - 0.5},${H}L0.5,${H}Z`} fill="currentColor" fillOpacity={0.15} stroke="none" />
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
          data-second={String(hover.s0)}
          data-rate={String(rate)}
          data-span={`${hover.s0}-${hover.s1}`}
          className="pointer-events-none fixed z-50 whitespace-nowrap rounded-md border bg-popover px-2 py-1 text-sm text-popover-foreground shadow-md"
          style={{ left: hover.x + 12, top: hover.top - 64 }}
        >
          <div className="text-muted-foreground">{ago === 0 ? t("sessions.sparkNow", "刚刚") : t("sessions.sparkAgo", "{{n}} 秒前", { n: ago })}</div>
          <div className="font-mono tabular-nums">{t("sessions.sparkRate", "{{n}} tokens/s", { n: formatNumber(rate) })}</div>
        </div>,
        document.body,
      )}
    </>
  );
}
