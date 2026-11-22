// 「正在执行」区行内迷你 token 走势（票 12）：纯 SVG 细柱，无坐标轴；数据复用会话详情的时间序列（每次调用一点，
// 入 + 出 + 缓存读写），超过 MAX_BARS 次调用按时间等宽分桶求和。只在置顶区挂载，会话有新调用（lastCallMs 变）才重读。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { clientUsageApi, type ClientUsageTokenPoint } from "../../services/api/clientUsage";
import { formatNumber } from "../../utils/formatters";

const W = 160;
const H = 24;
const MAX_BARS = 20;

const total = (p: ClientUsageTokenPoint) => p.inputTokens + p.outputTokens + p.cacheReadTokens + p.cacheWriteTokens;

export function sparkBars(points: ClientUsageTokenPoint[]): number[] {
  if (points.length <= MAX_BARS) return points.map(total);
  const t0 = points[0].callTimeMs;
  const span = Math.max(1, points[points.length - 1].callTimeMs - t0);
  const bars = new Array<number>(MAX_BARS).fill(0);
  for (const p of points) bars[Math.min(MAX_BARS - 1, Math.floor(((p.callTimeMs - t0) / span) * MAX_BARS))] += total(p);
  return bars;
}

export function SessionSparkline({ clientCode, sessionId, lastCallMs }: { clientCode: string; sessionId: string; lastCallMs: number }) {
  const { t } = useTranslation();
  const [bars, setBars] = useState<number[] | null>(null);
  useEffect(() => {
    let alive = true;
    clientUsageApi.sessionDetail(clientCode, sessionId, "points").then(
      (d) => { if (alive) setBars(sparkBars(d.points)); },
      () => {},
    );
    return () => { alive = false; };
  }, [clientCode, sessionId, lastCallMs]);
  const list = bars ?? [];
  const max = Math.max(1, ...list);
  const sum = list.reduce((a, b) => a + b, 0);
  const step = list.length ? W / list.length : 0;
  const bw = Math.max(1, step * 0.6);
  return (
    <svg
      height={H}
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={t("sessions.sparkLabel", "token 走势，共 {{n}}", { n: formatNumber(sum) })}
      className="w-14 text-[var(--primary)] @md:w-40"
      data-testid="sessions-row-spark"
      data-bars={String(list.length)}
      data-total={String(sum)}
    >
      {list.map((v, i) => {
        const h = v > 0 ? Math.max(1, (v / max) * H) : 0;
        // biome-ignore lint/suspicious/noArrayIndexKey: 柱序即时间序，无更稳定的键
        return <rect key={i} x={i * step + (step - bw) / 2} y={H - h} width={bw} height={h} rx={0.5} fill="currentColor" />;
      })}
    </svg>
  );
}
