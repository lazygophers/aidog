// 会话页顶部客户端四格（设计稿方向 3）：启用格可点选筛选；未启用格虚线记号 +「去设置启用」。
import { useTranslation } from "react-i18next";
import { Button } from "../ui/button";
import { Progress } from "../ui/progress";
import type { ClientUsageImportState, ClientUsageSessionRow } from "../../services/api/clientUsage";
import { formatRelativeTime } from "../../utils/formatters";
import { ClientGlyph, SESSION_CLIENTS } from "./ClientGlyph";
import { SessionCost } from "./SessionCost";

interface Props {
  enabled: Record<string, boolean>;
  rows: ClientUsageSessionRow[];
  importState?: ClientUsageImportState;
  pick: string | null;
  onPick: (code: string) => void;
  onGoSettings: () => void;
}

export function ClientTiles({ enabled, rows, importState, pick, onPick, onGoSettings }: Props) {
  const { t } = useTranslation();
  return (
    <div className="grid grid-cols-4 overflow-hidden rounded-xl border border-border bg-card" data-testid="sessions-tiles">
      {SESSION_CLIENTS.map((c) => {
        const on = !!enabled[c.code];
        const base = "relative grid min-h-[118px] grid-cols-[56px_minmax(0,1fr)] gap-3.5 border-s border-border p-4 text-start first:border-s-0";
        if (!on) {
          return (
            <div key={c.code} className={base} data-testid={`sessions-tile-${c.code}`} data-state="off" data-selected="false">
              <ClientGlyph code={c.code} size={52} on={false} />
              <div className="min-w-0">
                <div className="font-semibold">{c.name}</div>
                <div className="mt-0.5 text-sm text-muted-foreground">{t("sessions.tileOff", "未启用读取")}</div>
                <Button size="sm" variant="outline" className="mt-3" data-testid={`sessions-tile-go-${c.code}`} onClick={onGoSettings}>
                  {t("clientUsage.guideGo", "去设置启用")}
                </Button>
              </div>
            </div>
          );
        }
        const mine = rows.filter((r) => r.clientCode === c.code);
        const calls = mine.reduce((a, r) => a + r.calls, 0);
        const unpriced = mine.reduce((a, r) => a + r.unpricedCalls, 0);
        const cost = mine.reduce((a, r) => a + r.cost, 0);
        const last = mine.reduce((a, r) => Math.max(a, r.lastCallMs), 0);
        const imp = importState?.running && importState.clientCode === c.code ? importState : null;
        const pct = imp && imp.filesTotal > 0 ? Math.round((imp.filesDone / imp.filesTotal) * 100) : 0;
        const selected = pick === c.code;
        return (
          <button
            key={c.code}
            type="button"
            aria-pressed={selected}
            className={`${base} cursor-pointer hover:bg-muted ${selected ? "bg-[var(--accent-subtle)] after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:bg-primary" : ""}`}
            data-testid={`sessions-tile-${c.code}`}
            data-state="on"
            data-selected={selected ? "true" : "false"}
            onClick={() => onPick(c.code)}
          >
            <ClientGlyph code={c.code} size={52} on />
            <div className="min-w-0">
              <div className="font-semibold">{c.name}</div>
              <div className="mt-0.5 text-sm text-[var(--color-success)]">
                {imp ? t("sessions.importing", "正在导入 {{done}}/{{total}}", { done: imp.filesDone, total: imp.filesTotal }) : t("sessions.tileOn", "已启用")}
              </div>
              {imp && <Progress value={pct} className="mt-2 h-1 w-4/5" data-testid={`sessions-tile-import-${c.code}`} data-progress={String(pct)} />}
              <div className="mt-3 flex gap-4 text-sm text-muted-foreground">
                <div>
                  <b className="block font-mono text-lg font-medium text-foreground tabular-nums" data-testid={`sessions-tile-count-${c.code}`} data-count={String(mine.length)}>{mine.length}</b>
                  {t("sessions.tileSessions", "会话")}
                </div>
                <div>
                  <b className="block font-mono text-lg font-medium text-foreground tabular-nums">
                    <SessionCost cost={cost} calls={calls} unpricedCalls={unpriced} />
                  </b>
                  {t("sessions.tileCost", "成本")}
                </div>
                <div className="min-w-0">
                  <b className="block truncate pt-1 font-medium text-foreground">{last ? formatRelativeTime(last) : "—"}</b>
                  {t("sessions.tileLast", "最近")}
                </div>
              </div>
            </div>
          </button>
        );
      })}
    </div>
  );
}
