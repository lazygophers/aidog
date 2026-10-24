// ── Pill ──
// 行卡徽标 pill（终稿 Linear 精密密度）：语义色 token + color-mix 淡底/描边。
// 原 PlatformCard 内 9 处重复的内联 pill idiom 收口成组件；fontSize 12（可读性底线 ≥12px）。
// color 传 CSS 变量名（如 "var(--color-success)"），用于 color-mix 派生底/边。

import type { ReactNode } from "react";

export function Pill({ color, title, onClick, expanded, children, style }: {
  color: string;
  title?: string;
  onClick?: () => void;
  /** aria-expanded（last_error 块可折叠时用） */
  expanded?: boolean;
  children: ReactNode;
  style?: React.CSSProperties;
}) {
  return (
    <div
      style={{
        display: "inline-flex", alignItems: "center", gap: 4,
        fontSize: 12, fontWeight: 600, color,
        background: `color-mix(in srgb, ${color} 12%, transparent)`,
        border: `1px solid color-mix(in srgb, ${color} 30%, transparent)`,
        borderRadius: 5, padding: "1px 6px", whiteSpace: "nowrap",
        ...(onClick ? { cursor: "pointer" } : {}),
        ...style,
      }}
      onClick={onClick}
      aria-expanded={expanded}
      title={title}
    >
      {children}
    </div>
  );
}
