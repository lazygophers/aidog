// ── BalanceBar ──
// 余额 / 配额进度条：remaining 占 total 的百分比条 + 数值。
// 遵「无数据隐藏整行」约定：remaining 缺值（null/undefined/NaN）时不渲染（返回 null）。
// 全走 CSS 变量，明暗双模式对比度由 globals.css 语义色保证。

import { formatCost } from "../../utils/formatters";
import type { ColorLevel } from "./colorScale";
import { levelColor } from "./colorScale";

export interface BalanceBarProps {
  /** 剩余额度；null/undefined/NaN 时整条不渲染。 */
  remaining: number | null | undefined;
  /** 总额度；为 null 时只显数值不显进度条（无分母无法算占比）。 */
  total?: number | null;
  /** 货币符号前缀，默认 "$"。空串 = 无前缀（ACU 等非货币单位用）。 */
  currency?: string;
  /** 进度条已用部分的语义色，默认按剩余占比自动分档。 */
  level?: ColorLevel;
  /** 是否在数值后显示 total（如 "$12.30 / $50.00"）。默认 true。 */
  showTotal?: boolean;
  /** 数值下方次级标签（如 "ACU 用量" / "手动预算"）。无 label 不渲染。 */
  label?: string;
}

/** 剩余占比（0–100）→ 语义级别：越低越危险。total<=0 时 neutral。 */
function remainingLevel(pct: number): ColorLevel {
  if (pct >= 50) return "success";
  if (pct >= 20) return "warning";
  return "danger";
}

export function BalanceBar({ remaining, total, currency = "$", level, showTotal = true, label }: BalanceBarProps) {
  if (remaining == null || Number.isNaN(remaining)) return null;

  const hasTotal = typeof total === "number" && total > 0;
  const pct = hasTotal ? Math.max(0, Math.min(100, (remaining / total!) * 100)) : null;
  const barLevel = level ?? (pct != null ? remainingLevel(pct) : "neutral");
  const color = levelColor(barLevel);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 3, minWidth: 0 }}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 4, fontSize: 12 }}>
        <span style={{ fontWeight: 700, color }}>
          {currency}
          {formatCost(remaining)}
        </span>
        {hasTotal && showTotal && (
          <span style={{ fontSize: 10, color: "var(--text-tertiary)" }}>
            / {currency}
            {formatCost(total!)}
          </span>
        )}
      </div>
      {pct != null && (
        <div
          style={{
            height: 4,
            borderRadius: "var(--radius-sm)",
            background: "var(--bg-glass)",
            overflow: "hidden",
          }}
        >
          <div
            style={{
              width: `${pct}%`,
              height: "100%",
              background: color,
              borderRadius: "var(--radius-sm)",
              transition: "width 0.3s ease",
            }}
          />
        </div>
      )}
      {label && (
        <span style={{ fontSize: 9, fontWeight: 700, color: "var(--text-tertiary)" }}>{label}</span>
      )}
    </div>
  );
}
