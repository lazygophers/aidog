// ── StatChip ──
// 小统计 chip（图标 + 值 + 标签），可选语义色编码。
// 视觉对齐 Groups.tsx 原 4-chip：glass 底 + 圆角 + 粗体值 + 次级标签。
// 外壳走 shadcn Badge（variant 由 level 派生），值文字色保持内联（测试依赖）。

import type { ReactNode } from "react";
import type { ColorLevel } from "./colorScale";
import { levelColor } from "./colorScale";
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
  /** 语义级别 → 自动取 var(--color-*) 作为值文字颜色。 */
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
        gap: 5,
        padding: "4px 10px",
        borderRadius: "var(--radius-sm)",
        background: "var(--bg-glass)",
        border: "1px solid var(--border)",
        fontSize: 12,
      }}
    >
      {icon && <span style={{ fontSize: 13, display: "inline-flex" }}>{icon}</span>}
      <span style={{ fontWeight: 700, color: valueColor }}>{value}</span>
      <span style={{ fontSize: 10, color: "var(--text-tertiary)", fontWeight: 500 }}>{label}</span>
    </Badge>
  );
}
