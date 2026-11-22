// 「正在执行」区行内迷你 token 走势（票 12 / 15）：最近 5 分钟逐秒，每秒一格（入 + 出 + 缓存读写，记在调用时间所在秒）；
// 全部置顶行共用一个 1 秒计时器滑动窗口（页面隐藏时暂停）。数据复用会话详情的时间序列，会话有新调用（lastCallMs 变）才重读。
import { useEffect, useState, useSyncExternalStore } from "react";
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

export function SessionSparkline({ clientCode, sessionId, lastCallMs }: { clientCode: string; sessionId: string; lastCallMs: number }) {
  const { t } = useTranslation();
  const [points, setPoints] = useState<ClientUsageTokenPoint[]>([]);
  const now = useNowMs();
  useEffect(() => {
    let alive = true;
    clientUsageApi.sessionDetail(clientCode, sessionId, "points").then(
      (d) => { if (alive) setPoints(d.points); },
      () => {},
    );
    return () => { alive = false; };
  }, [clientCode, sessionId, lastCallMs]);
  const bars = secondBuckets(points, now);
  const max = Math.max(1, ...bars);
  const sum = bars.reduce((a, b) => a + b, 0);
  // 300 格挤进 56–160px：每个非零秒画一条 1px 竖线（non-scaling-stroke），同一像素列内多秒重叠即取最高者。
  const d = bars.map((v, i) => (v > 0 ? `M${i + 0.5} ${H}V${H - Math.max(1, (v / max) * H)}` : "")).join("");
  return (
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
    >
      <path d={`M0 ${H - 0.5}H${SECS}`} stroke="currentColor" strokeOpacity={0.25} vectorEffect="non-scaling-stroke" />
      {d && <path d={d} stroke="currentColor" strokeWidth={1} vectorEffect="non-scaling-stroke" />}
    </svg>
  );
}
