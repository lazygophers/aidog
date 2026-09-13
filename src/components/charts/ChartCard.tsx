// ── 图表玻璃卡容器（spec §B2 公共层）──
// 复用既有 .glass-surface（含 hover 琥珀描边），只加标题/副题排版与诚实空态。
// flow-border：hover conic 流光描边走既有 opt-in 模式（挂法对齐 CompactCard）。
// 无数据不画零值假图：empty=true 时 children 不渲染，文案走 i18n。
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";

export interface ChartCardProps {
  title?: ReactNode;
  subtitle?: ReactNode;
  /** 无数据 → 诚实空态。调用方以 data.length === 0 传入。 */
  empty?: boolean;
  /** 空态副行（如筛选原因提示），可选。 */
  emptyHint?: ReactNode;
  className?: string;
  children: ReactNode;
}

export function ChartCard({ title, subtitle, empty, emptyHint, className, children }: ChartCardProps) {
  const { t } = useTranslation();
  return (
    <div className={cn("glass-surface flow-border", className)} style={{ padding: "16px 20px" }}>
      {title != null && (
        <div style={{ marginBottom: 12 }}>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{title}</div>
          {subtitle != null && (
            <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2 }}>{subtitle}</div>
          )}
        </div>
      )}
      {empty ? (
        <div
          style={{
            minHeight: 160,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: 4,
            color: "var(--text-secondary)",
          }}
        >
          <span style={{ fontSize: 13 }}>{t("charts.noData", "暂无数据")}</span>
          {emptyHint != null && (
            <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>{emptyHint}</span>
          )}
        </div>
      ) : (
        children
      )}
    </div>
  );
}
