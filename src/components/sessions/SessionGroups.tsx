// 会话列表（设计稿方向 3）：按工作目录分组，左列 sticky 项目信息，右列会话行。
// 操作列先占位：继续 / 打开目录 / 隐藏等由票 07–10 接入，未实现前不画按钮。
import { useTranslation } from "react-i18next";
import type { ClientUsageSessionRow } from "../../services/api/clientUsage";
import { formatNumber, formatRelativeTime } from "../../utils/formatters";
import { ClientGlyph, clientName } from "./ClientGlyph";
import { SessionCost } from "./SessionCost";

const ROW_GRID = "grid grid-cols-[20px_minmax(0,1fr)_170px_96px_96px_70px] items-center gap-3";

function projectName(workdir: string): string {
  return workdir.split(/[\\/]/).filter(Boolean).pop() ?? workdir;
}

export function SessionGroups({ rows }: { rows: ClientUsageSessionRow[] }) {
  const { t } = useTranslation();
  const groups = new Map<string, ClientUsageSessionRow[]>();
  for (const r of rows) {
    const g = groups.get(r.workdir);
    if (g) g.push(r);
    else groups.set(r.workdir, [r]);
  }
  const duration = (r: ClientUsageSessionRow) => {
    const d = Math.max(0, r.lastCallMs - r.firstCallMs);
    return d < 3600_000
      ? t("sessions.durMin", "{{n}} 分", { n: Math.round(d / 60_000) })
      : t("sessions.durHour", "{{n}} 小时", { n: (d / 3600_000).toFixed(1) });
  };
  return (
    <div data-testid="sessions-list">
      {[...groups.entries()].map(([workdir, items]) => {
        const calls = items.reduce((a, r) => a + r.calls, 0);
        const unpriced = items.reduce((a, r) => a + r.unpricedCalls, 0);
        const cost = items.reduce((a, r) => a + r.cost, 0);
        return (
          <div key={workdir} className="grid grid-cols-[230px_minmax(0,1fr)] border-t pt-4 pb-2.5" data-testid="sessions-group" data-workdir={workdir}>
            <div className="sticky top-0 self-start pe-4">
              <b className="block truncate text-lg font-semibold" title={workdir}>{workdir ? projectName(workdir) : t("sessions.noWorkdir", "未知目录")}</b>
              {workdir && <small className="mt-1 block break-all font-mono text-sm text-muted-foreground"><span dir="ltr">{workdir}</span></small>}
              <div className="mt-2.5 text-sm text-muted-foreground">
                {t("sessions.count", "{{n}} 个会话", { n: items.length })} · <span className="font-mono tabular-nums"><SessionCost cost={cost} calls={calls} unpricedCalls={unpriced} /></span>
              </div>
            </div>
            <div>
              {items.map((r) => (
                <div key={`${r.clientCode}:${r.sessionId}`} className={`${ROW_GRID} rounded-lg px-3 py-2.5 hover:bg-muted`} data-testid="sessions-row" data-client={r.clientCode} data-session={r.sessionId}>
                  <ClientGlyph code={r.clientCode} size={16} on />
                  <div className="min-w-0">
                    <div className="truncate" title={r.title || r.sessionId}>{r.title || r.sessionId}</div>
                    <div className="mt-0.5 truncate text-sm text-muted-foreground">
                      {clientName(r.clientCode)} · {t("sessions.calls", "{{n}} 次调用", { n: r.calls })} · {t("sessions.duration", "持续 {{d}}", { d: duration(r) })}
                    </div>
                  </div>
                  <div className="text-end font-mono text-sm tabular-nums">
                    {formatNumber(r.inputTokens + r.outputTokens)}
                    <div className="truncate text-muted-foreground">
                      {t("sessions.tokenSplit", "入 {{input}} / 出 {{output}}", { input: formatNumber(r.inputTokens), output: formatNumber(r.outputTokens) })}
                    </div>
                  </div>
                  <div className="text-end font-mono text-sm tabular-nums">
                    <SessionCost cost={r.cost} calls={r.calls} unpricedCalls={r.unpricedCalls} testId="sessions-row-cost" />
                  </div>
                  <div className="text-end text-sm text-muted-foreground">{formatRelativeTime(r.lastCallMs)}</div>
                  <div className="text-end" data-testid="sessions-row-actions" />
                </div>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}
