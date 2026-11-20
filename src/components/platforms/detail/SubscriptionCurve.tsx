// 订阅消耗曲线（T3 板块 B，方向 5 密度终端）：实际剩余平滑拟合线 + 计划虚线 +
// 重置竖线 + today 游标 + 快照校准点（偏差 >5% 图注）。纯渲染，数据映射在 chartMath。
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
  const { windowStart, resetAt, actual, plan, calibrations, maxDeviation, nowPct } = curve;

  const fitPts = actual.map(p => ({ x: xScale(p.t, windowStart, resetAt), y: yScale(p.pct) }));
  const fit = monotonePath(fitPts);
  const area = fitPts.length > 1
    ? `${fit} L ${fitPts[fitPts.length - 1].x} ${Y1} L ${fitPts[0].x} ${Y1} Z`
    : "";
  const planA = { x: xScale(plan[0].t, windowStart, resetAt), y: yScale(plan[0].pct) };
  const planB = { x: xScale(plan[1].t, windowStart, resetAt), y: yScale(plan[1].pct) };
  const todayX = xScale(nowMs, windowStart, resetAt);
  const resetLabel = formatDateTime(resetAt);
  const deviationFlag = maxDeviation != null && maxDeviation > 5;

  const calibLabel = (c: CalibrationPoint) =>
    t("platform.detail.snapshot", "快照 {{pct}}%").replace("{{pct}}", `${Math.round(c.pct)}`);

  return (
    <div data-testid="detail-t3-curve">
      <svg viewBox="0 0 760 268" width="100%" role="img" aria-label={t("platform.detail.curve", "订阅消耗曲线")}>
        {/* 横向网格线 + Y 轴刻度 */}
        {[100, 75, 50, 25, 0].map((p, i) => (
          <g key={p}>
            <line x1={X0} y1={yScale(p)} x2={X1} y2={yScale(p)}
              stroke={i === 4 ? "var(--input)" : "var(--border)"} strokeWidth={1} />
            <text x={X0 - 6} y={yScale(p) + 4} textAnchor="end" fontSize={10}
              fill="var(--muted-foreground)" fontFamily="var(--font-mono, monospace)">{p}</text>
          </g>
        ))}
        {/* 计划消耗线（虚线） */}
        <line x1={planA.x} y1={planA.y} x2={planB.x} y2={planB.y}
          stroke="var(--muted-foreground)" strokeWidth={1.5} strokeDasharray="6 5" />
        {/* 实际剩余平滑拟合线 + 淡填充 */}
        {area && <path d={area} fill="var(--accent-subtle)" stroke="none" />}
        {fit && <path d={fit} stroke="var(--data-primary)" strokeWidth={2.5} fill="none" strokeLinejoin="round" />}
        {/* 原始记账点（弱化叠加） */}
        {fitPts.map((p, i) => (
          <circle key={i} cx={p.x} cy={p.y} r={3} fill="var(--data-primary)" opacity={0.35} />
        ))}
        {/* 快照校准点 */}
        {calibrations.map((c, i) => {
          const cx = xScale(c.t, windowStart, resetAt);
          return (
            <g key={i}>
              <circle cx={cx} cy={yScale(c.pct)} r={5} fill="var(--color-card)"
                stroke="var(--color-warning)" strokeWidth={2} />
              <text x={cx + 12} y={yScale(c.pct) - 6} fontSize={10} fill="var(--color-warning)"
                fontFamily="var(--font-mono, monospace)">{calibLabel(c)}</text>
            </g>
          );
        })}
        {/* today 游标 */}
        <line x1={todayX} y1={Y0 + 2} x2={todayX} y2={Y1} stroke="var(--accent)" strokeWidth={1.5} />
        {nowPct != null && (
          <>
            <circle cx={todayX} cy={yScale(nowPct)} r={4.5} fill="var(--data-primary)" />
            <text x={Math.min(todayX, X1 - 60)} y={Y0 - 8} fontSize={10} fill="var(--accent)"
              textAnchor="middle" fontFamily="var(--font-mono, monospace)">
              {t("platform.detail.todayCursor", "今天 · 剩 {{pct}}%").replace("{{pct}}", `${Math.round(nowPct)}`)}
            </text>
          </>
        )}
        {/* 重置竖线 + 标签 */}
        <line x1={planB.x} y1={Y0} x2={planB.x} y2={Y1}
          stroke="var(--muted-foreground)" strokeWidth={1.5} strokeDasharray="4 4" />
        <text x={planB.x} y={256} fontSize={10} fill="var(--muted-foreground)" textAnchor="end"
          fontFamily="var(--font-mono, monospace)">
          {t("platform.detail.resetAt", "重置于 {{time}}").replace("{{time}}", resetLabel ?? "-")}
        </text>
      </svg>
      <div style={{ display: "flex", gap: 14, flexWrap: "wrap", fontSize: 11, color: "var(--muted-foreground)", marginTop: 4 }}>
        <LegendSwatch kind="line" label={t("platform.detail.actualLine", "实际剩余（本地记账）")} />
        <LegendSwatch kind="dot" label={t("platform.detail.rawPoints", "原始记账点")} />
        <LegendSwatch kind="dash" label={t("platform.detail.planLine", "计划消耗线")} />
        <LegendSwatch kind="calib" label={t("platform.detail.calibPoints", "额度快照校准点")} />
        <LegendSwatch kind="vline" label={t("platform.detail.resetDay", "重置日")} />
      </div>
      <div style={{ marginTop: 8, fontSize: 12, color: "var(--text-secondary)" }}>
        {t("platform.detail.remainNow", "当前剩余 {{pct}}%").replace("{{pct}}", `${nowPct != null ? Math.round(nowPct) : "-"}`)}
        {deviationFlag && (
          <div data-testid="detail-t3-deviation" style={{ color: "var(--color-warning)", marginTop: 4 }}>
            {t("platform.detail.deviationNote", "快照与本地记账偏差 {{pct}}%，可能存在未记录的用量")
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
