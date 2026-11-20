// 按量平台近 7 天每日用量柱线图（T3 板块 B'）：费用柱 + 请求数趋势线，无平滑线。
// 纯渲染，数据映射在 chartMath.buildDailyBars。
import { useTranslation } from "react-i18next";
import type { DailyBar } from "./chartMath";
import { formatMoney } from "../../../utils/formatters";

const X0 = 44, X1 = 720, Y0 = 30, Y1 = 176;
const BAR_W = 44;

export function PaygoBarsView({ bars }: { bars: DailyBar[] }) {
  const { t } = useTranslation();
  const n = Math.max(bars.length, 1);
  const step = (X1 - X0) / n;
  const maxCost = Math.max(...bars.map(b => b.cost), 0.0001);
  const maxReq = Math.max(...bars.map(b => b.requests), 1);
  const moneyTicks = [maxCost, maxCost * 2 / 3, maxCost / 3];

  const cx = (i: number) => X0 + step * (i + 0.5);
  const barY = (c: number) => Y1 - (c / maxCost) * (Y1 - Y0);
  const reqY = (r: number) => Y1 - (r / maxReq) * (Y1 - Y0) * 0.85;
  const linePts = bars.map((b, i) => `${cx(i)},${reqY(b.requests)}`).join(" L");

  return (
    <div data-testid="detail-t3-paygo">
      <svg viewBox="0 0 760 224" width="100%" role="img" aria-label={t("platform.detail.paygo", "近 7 天用量")}>
        {/* 网格 + 费用刻度 */}
        {moneyTicks.map((v, i) => {
          const y = barY(v);
          return (
            <g key={i}>
              <line x1={X0} y1={y} x2={X1} y2={y} stroke={i === 0 ? "var(--input)" : "var(--border)"} strokeWidth={1} />
              <text x={X0 - 6} y={y + 4} textAnchor="end" fontSize={10} fill="var(--muted-foreground)"
                fontFamily="var(--font-mono, monospace)">{formatMoney(v)}</text>
            </g>
          );
        })}
        <line x1={X0} y1={Y1} x2={X1} y2={Y1} stroke="var(--input)" strokeWidth={1} />
        {/* 每日费用柱 */}
        {bars.map((b, i) => {
          const y = barY(b.cost);
          return <rect key={b.day} x={cx(i) - BAR_W / 2} y={y} width={BAR_W}
            height={Math.max(0, Y1 - y)} rx={3} fill="var(--data-primary)" opacity={0.9} />;
        })}
        {/* 请求数趋势线 */}
        {bars.length > 1 && (
          <>
            <path d={`M ${linePts}`} stroke="var(--color-warning)" strokeWidth={2} fill="none" strokeLinejoin="round" />
            {bars.map((b, i) => <circle key={b.day} cx={cx(i)} cy={reqY(b.requests)} r={3} fill="var(--color-warning)" />)}
          </>
        )}
        {/* 日标签 */}
        {bars.map((b, i) => (
          <text key={b.day} x={cx(i)} y={196} textAnchor="middle" fontSize={10}
            fill={i === bars.length - 1 ? "var(--accent)" : "var(--muted-foreground)"}
            fontFamily="var(--font-mono, monospace)">
            {i === bars.length - 1
              ? t("platform.detail.todayLabel", "今天")
              : b.day.slice(5)}
          </text>
        ))}
      </svg>
      <div style={{ display: "flex", gap: 14, flexWrap: "wrap", fontSize: 11, color: "var(--muted-foreground)", marginTop: 4 }}>
        <span style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
          <span style={{ width: 12, height: 9, background: "var(--data-primary)", opacity: 0.9, borderRadius: 2, display: "inline-block" }} />
          {t("platform.detail.dailyCost", "每日费用")}
        </span>
        <span style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
          <span style={{ width: 14, height: 0, borderTop: "2px solid var(--color-warning)", display: "inline-block" }} />
          {t("platform.detail.reqTrend", "请求数趋势")}
        </span>
      </div>
    </div>
  );
}
