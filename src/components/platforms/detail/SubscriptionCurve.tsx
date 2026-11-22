// 订阅消耗曲线（T3 板块 B，方向 5 密度终端）：实际剩余平滑拟合线 + 计划虚线 +
// 重置竖线 + today 游标 + 快照校准点（偏差 >5% 图注）。纯渲染，数据映射在 chartMath。
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { monotonePath, type SubscriptionCurve, type CalibrationPoint } from "./chartMath";
import { formatDateTime } from "../../../utils/formatters";

const X0 = 44, X1 = 700, Y0 = 28, Y1 = 212;

const xScale = (t: number, ws: number, re: number) =>
  X0 + (X1 - X0) * Math.max(0, Math.min(1, (t - ws) / Math.max(1, re - ws)));
const yScale = (pct: number) => Y1 + (Y0 - Y1) * (Math.max(0, Math.min(100, pct)) / 100);

export function SubscriptionCurveView({ curve, nowMs }: {
  curve: SubscriptionCurve;
  nowMs: number;
}) {
  const { t } = useTranslation();
  const chartRef = useRef<HTMLDivElement>(null);
  const [chartWidth, setChartWidth] = useState(760);
  useEffect(() => {
    const element = chartRef.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    setChartWidth(element.getBoundingClientRect().width);
    const observer = new ResizeObserver(([entry]) => setChartWidth(entry.contentRect.width));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const tickFontSize = 14 * 850 / Math.max(chartWidth, 1);
  const yTicks = chartWidth < 480 ? [100, 50, 0] : [100, 75, 50, 25, 0];
  const { windowStart, resetAt, actual, plan, calibrations, maxDeviation, snapshotPct } = curve;
  const latestActual = actual[actual.length - 1] ?? null;

  const fitPts = actual.map(p => ({ x: xScale(p.t, windowStart, resetAt), y: yScale(p.pct) }));
  const fit = actual.length >= 3 ? monotonePath(fitPts) : "";
  const area = fit
    ? `${fit} L ${fitPts[fitPts.length - 1].x} ${Y1} L ${fitPts[0].x} ${Y1} Z`
    : "";
  const planA = { x: xScale(plan[0].t, windowStart, resetAt), y: yScale(plan[0].pct) };
  const planB = { x: xScale(plan[1].t, windowStart, resetAt), y: yScale(plan[1].pct) };
  const todayX = xScale(nowMs, windowStart, resetAt);
  const resetLabel = formatDateTime(resetAt);
  const deviationFlag = maxDeviation != null && maxDeviation > 5;
  const dataNote = actual.length === 0
    ? t(
        snapshotPct != null ? "platform.detail.curveNoHistory" : "platform.detail.curveUnknown",
        snapshotPct != null
          ? "No complete, unit-matched local history; showing the official snapshot only."
          : "Window coverage or quota unit is unknown; remaining cannot be established.",
      )
    : actual.length < 3
      ? t("platform.detail.curveSparse", "Only {{count}} verified daily samples; no smooth trend is drawn.")
          .replace("{{count}}", `${actual.length}`)
      : snapshotPct == null
        ? t("platform.detail.curveCurrentUnknown", "The local curve ends at the latest sample; current remaining is unknown.")
        : null;

  const calibLabel = (c: CalibrationPoint) =>
    t("platform.detail.snapshot", "Snapshot {{pct}}%").replace("{{pct}}", `${Math.round(c.pct)}`);

  return (
    <div data-testid="detail-t3-curve" ref={chartRef}>
      <svg viewBox="-90 -20 850 344" width="100%" direction="ltr" role="img" aria-label={t("platform.detail.curve", "Subscription Usage Curve")}>
        {/* 横向网格线 + Y 轴刻度 */}
        {[100, 75, 50, 25, 0].map((p, i) => (
          <g key={p}>
            <line x1={X0} y1={yScale(p)} x2={X1} y2={yScale(p)}
              stroke={i === 4 ? "var(--input)" : "var(--border)"} strokeWidth={1} />
            {yTicks.includes(p) && (
              <text x={X0 - 6} y={yScale(p) + 4} textAnchor="end" fontSize={tickFontSize}
                fill="var(--muted-foreground)" fontFamily="var(--font-mono, monospace)">{p}</text>
            )}
          </g>
        ))}
        {/* 计划消耗线（虚线） */}
        <line x1={planA.x} y1={planA.y} x2={planB.x} y2={planB.y}
          stroke="var(--muted-foreground)" strokeWidth={1.5} strokeDasharray="6 5" />
        {/* 实际剩余平滑拟合线 + 淡填充 */}
        {area && <path d={area} fill="var(--accent-subtle)" stroke="none" />}
        {fit && <path data-testid="detail-t3-fit" d={fit} stroke="var(--data-primary)" strokeWidth={2.5} fill="none" strokeLinejoin="round" />}
        {/* 原始记账点（弱化叠加） */}
        {fitPts.map((p, i) => (
          <circle key={i} data-testid="detail-t3-raw-point" cx={p.x} cy={p.y} r={3} fill="var(--data-primary)" opacity={0.35} />
        ))}
        {/* 快照校准点 */}
        {calibrations.map((c, i) => {
          const cx = xScale(c.t, windowStart, resetAt);
          return (
            <g key={`${c.name}-${i}`}>
              <circle cx={cx} cy={yScale(c.pct)} r={5} fill="var(--color-card)"
                stroke="var(--color-warning)" strokeWidth={2}>
                <title>{calibLabel(c)}</title>
              </circle>
            </g>
          );
        })}
        {/* today 游标 */}
        <line x1={todayX} y1={Y0 + 2} x2={todayX} y2={Y1} stroke="var(--accent)" strokeWidth={1.5} />
        {[windowStart, (windowStart + resetAt) / 2, resetAt].map((time, i) => (
          <text key={i} x={xScale(time, windowStart, resetAt)} y={Y1 + 90} fontSize={tickFontSize}
            fill="var(--muted-foreground)" textAnchor={i === 0 ? "start" : i === 2 ? "end" : "middle"}
            fontFamily="var(--font-mono, monospace)">
            {new Date(time).toLocaleDateString(undefined, { month: "2-digit", day: "2-digit" })}
          </text>
        ))}
        {/* 重置线标签与校准说明放在图外，避免压住采样点与 today 游标。 */}
        <line x1={planB.x} y1={Y0} x2={planB.x} y2={Y1}
          stroke="var(--muted-foreground)" strokeWidth={1.5} strokeDasharray="4 4" />
      </svg>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, flexWrap: "wrap", fontSize: 14, color: "var(--muted-foreground)", marginTop: 4 }}>
        <span>{t("platform.detail.todayLabel", "Today")}</span>
        <span>{t("platform.detail.resetAt", "Resets {{time}}").replace("{{time}}", resetLabel ?? "-")}</span>
      </div>
      {calibrations.length > 0 && (
        <div data-testid="detail-t3-calibrations" style={{ display: "flex", gap: 10, flexWrap: "wrap", fontSize: 14, color: "var(--color-warning)", marginTop: 6 }}>
          {calibrations.map((c, i) => <span key={`${c.name}-${i}`}>{calibLabel(c)}</span>)}
        </div>
      )}
      <div style={{ display: "flex", gap: 14, flexWrap: "wrap", fontSize: 14, color: "var(--muted-foreground)", marginTop: 8 }}>
        <LegendSwatch kind="line" label={t("platform.detail.actualLine", "Local remaining (daily samples; shape-preserving interpolation)")} />
        <LegendSwatch kind="dot" label={t("platform.detail.rawPoints", "Raw ledger points")} />
        <LegendSwatch kind="dash" label={t("platform.detail.planLine", "Planned usage line")} />
        <LegendSwatch kind="calib" label={t("platform.detail.calibPoints", "Quota snapshot calibration points")} />
        <LegendSwatch kind="vline" label={t("platform.detail.resetDay", "Reset day")} />
      </div>
      {dataNote && (
        <div data-testid="detail-t3-sparse" role="note" style={{ marginTop: 8, fontSize: 14, color: "var(--muted-foreground)" }}>
          {dataNote}
        </div>
      )}
      <div style={{ marginTop: 8, fontSize: 14, color: "var(--text-secondary)" }}>
        {snapshotPct != null && calibrations[0] && (
          <div data-testid="detail-t3-snapshot">
            {t("platform.detail.quotaSnapshotAt", "Official snapshot: {{pct}}% · {{time}}")
              .replace("{{pct}}", `${Math.round(snapshotPct)}`)
              .replace("{{time}}", formatDateTime(calibrations[0].t) ?? "-")}
          </div>
        )}
        {snapshotPct == null && latestActual && (
          <div data-testid="detail-t3-latest">
            {t("platform.detail.latestRecorded", "Latest local daily sample: {{pct}}% · {{time}}")
              .replace("{{pct}}", `${Math.round(latestActual.pct)}`)
              .replace("{{time}}", formatDateTime(latestActual.t) ?? "-")}
          </div>
        )}
        {deviationFlag && (
          <div data-testid="detail-t3-deviation" style={{ color: "var(--color-warning)", marginTop: 4 }}>
            {t("platform.detail.deviationNote", "Snapshot deviates {{pct}}% from local ledger; usage may be unrecorded")
              .replace("{{pct}}", maxDeviation!.toFixed(1))}
          </div>
        )}
      </div>
    </div>
  );
}

function LegendSwatch({ kind, label }: { kind: "line" | "dot" | "dash" | "calib" | "vline"; label: string }) {
  const swatch: Record<string, React.ReactNode> = {
    line: <span style={{ width: 14, height: 0, borderTop: "2.5px solid var(--data-primary)", display: "inline-block" }} />,
    dot: <span style={{ width: 7, height: 7, borderRadius: "50%", background: "var(--data-primary)", opacity: 0.45, display: "inline-block" }} />,
    dash: <span style={{ width: 14, height: 0, borderTop: "1.5px dashed var(--muted-foreground)", display: "inline-block" }} />,
    calib: <span style={{ width: 9, height: 9, borderRadius: "50%", border: "2px solid var(--color-warning)", display: "inline-block" }} />,
    vline: <span style={{ width: 0, height: 11, borderLeft: "1.5px dashed var(--muted-foreground)", display: "inline-block" }} />,
  };
  return (
    <span style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
      {swatch[kind]}{label}
    </span>
  );
}
