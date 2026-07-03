// 导入结果卡：applied(成功区) / skipped(中性区) / errors(危险区)。
// 抽自原 ImportExport.tsx L1205-1297（report 域）。

import type { TFunction } from "i18next";
import type { ImportReport } from "../../../services/api";
import { SectionIcon } from "../editors";
import { IconCheck } from "../../icons";
import { StatChip } from "../../shared/StatChip";

export function ReportView({
  report,
  t,
  scopeLabel,
}: {
  report: ImportReport;
  t: TFunction;
  scopeLabel: (s: string) => string;
}) {
  const applied = Object.entries(report.applied);
  const skipped = Object.entries(report.skipped);
  const appliedTotal = applied.reduce((a, [, v]) => a + v, 0);
  const skippedTotal = skipped.reduce((a, [, v]) => a + v, 0);

  return (
    <div className="glass-surface" style={{ padding: 14, borderRadius: "var(--radius-lg)", display: "flex", flexDirection: "column", gap: 12 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
        <strong style={{ fontSize: 14, color: "var(--text-primary)" }}>{t("importExport.reportTitle", "导入结果")}</strong>
        <StatChip value={String(appliedTotal)} label={t("importExport.applied", "已应用")} level="success" />
        <StatChip value={String(skippedTotal)} label={t("importExport.skipped", "已跳过")} level="neutral" />
        {report.errors.length > 0 && (
          <StatChip value={String(report.errors.length)} label={t("importExport.errorsLabel", "错误")} level="danger" />
        )}
      </div>

      {applied.length > 0 && (
        <ReportSection
          title={t("importExport.applied", "已应用")}
          color="var(--color-success)"
          bg="var(--color-success-bg)"
          icon={<IconCheck size={13} color="var(--color-success)" strokeWidth={2.5} />}
          rows={applied.map(([k, v]) => `${scopeLabel(k)}: ${v}`)}
        />
      )}
      {skipped.length > 0 && (
        <ReportSection
          title={t("importExport.skipped", "已跳过")}
          color="var(--text-tertiary)"
          bg="var(--color-neutral-bg)"
          icon={<SectionIcon name="status" size={13} style={{ color: "var(--text-tertiary)" }} />}
          rows={skipped.map(([k, v]) => `${scopeLabel(k)}: ${v}`)}
        />
      )}
      {report.errors.length > 0 && (
        <ReportSection
          title={t("importExport.errors", "错误（{{n}}）", { n: report.errors.length })}
          color="var(--color-danger)"
          bg="var(--color-danger-bg)"
          rows={report.errors}
        />
      )}
    </div>
  );
}

/** report 单语义区：小标题 + 行列表。 */
function ReportSection({
  title,
  color,
  bg,
  icon,
  rows,
}: {
  title: string;
  color: string;
  bg: string;
  icon?: React.ReactNode;
  rows: string[];
}) {
  return (
    <div
      style={{
        padding: 10,
        borderRadius: "var(--radius-md)",
        background: bg,
        border: `1px solid ${color}`,
        display: "flex",
        flexDirection: "column",
        gap: 4,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 13, fontWeight: 600, color }}>
        {icon}
        {title}
      </div>
      {rows.map((r, i) => (
        <div key={i} style={{ fontSize: 12, color: "var(--text-secondary)", paddingLeft: icon ? 19 : 0, wordBreak: "break-all" }}>
          {r}
        </div>
      ))}
    </div>
  );
}
