// 详情页区块骨架（方向 5 密度终端）：1px 边界分块、13px 标题 + mono hint，
// 供 T2 配置/快捷操作与 T3 数据板块共用。
import type { ReactNode } from "react";

export function Blk({ title, hint, testid, children }: {
  title: ReactNode;
  hint?: ReactNode;
  testid?: string;
  children: ReactNode;
}) {
  return (
    <section
      data-testid={testid}
      style={{ borderBottom: "1px solid var(--border)", padding: "16px 20px 18px" }}
    >
      <h2 style={{
        fontSize: 13, fontWeight: 600, color: "var(--text-primary)",
        margin: "0 0 12px", display: "flex", alignItems: "baseline", gap: 8,
      }}>
        {title}
        {hint != null && (
          <span style={{
            fontFamily: "var(--font-mono, monospace)", fontSize: 11,
            color: "var(--text-tertiary)", marginLeft: "auto", fontWeight: 400,
          }}>{hint}</span>
        )}
      </h2>
      {children}
    </section>
  );
}
