// ── StatChip ──
// 小统计 chip（图标 + 值 + 标签），可选语义色编码。
// 萤火虫玻璃签名：glass 底 + 柔和语义背景（--color-*-bg）+ pill 圆角 + 粗体值。
// 外壳走 shadcn Badge（variant 由 level 派生），值文字色保持内联（测试依赖）。

import type { ReactNode } from "react";
import type { ColorLevel } from "./colorScale";
import { levelColor, levelBg } from "./colorScale";
import { Badge } from "@/components/ui/badge";

export interface StatChipProps {
  /** 可选图标（来自 icons.tsx，禁 emoji）。 */
  icon?: ReactNode;
  /** 已格式化的值（如 "1.2M" / "$0.034" / "98.7%"）。 */
  value: string;
  /** 次级标签（如 "tokens" / "cost" / "ok"）。 */
  label: string;
  /** 直接指定值文字颜色（CSS 变量或 var()）；优先级高于 level。 */
  color?: string;
  /** 语义级别 → 自动取 var(--color-*) 作为值文字颜色 + 柔和背景。 */
  level?: ColorLevel;
}

/** ColorLevel → Badge variant 映射（外壳语义），值文字色仍走 levelColor 内联。 */
function levelToBadgeVariant(level: ColorLevel | undefined) {
  switch (level) {
    case "danger": return "destructive" as const;
    case "success": return "default" as const;
    case "warning": return "secondary" as const;
    case "neutral": return "secondary" as const;
    default: return "outline" as const;
  }
}

export function StatChip({ icon, value, label, color, level }: StatChipProps) {
  const valueColor = color ?? (level ? levelColor(level) : "var(--text-primary)");
  return (
    <Badge
      variant={levelToBadgeVariant(level)}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        padding: "5px 12px",
        borderRadius: "999px",
        background: level ? levelBg(level) : "var(--bg-glass)",
        border: "1px solid var(--border)",
        fontSize: 12,
        fontWeight: 500,
        transition: "transform 200ms cubic-bezier(0.4,0,0.2,1), box-shadow 200ms cubic-bezier(0.4,0,0.2,1)",
      }}
    >
      {icon && (
        <span style={{ fontSize: 13, display: "inline-flex", color: valueColor }}>{icon}</span>
      )}
      <span className="counter" style={{ fontWeight: 700, color: valueColor }}>{value}</span>
      <span style={{ fontSize: 10, color: "var(--text-tertiary)", fontWeight: 500 }}>{label}</span>
    </Badge>
  );
}
