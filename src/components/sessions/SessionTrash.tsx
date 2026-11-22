// 会话回收站（扩展-会话页 票 09）：列出「从列表隐藏」的会话，可恢复；居中 Dialog（Radix Portal 挂 body）。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { clientUsageApi, type ClientUsageHiddenSessionRow } from "../../services/api/clientUsage";
import { formatRelativeTime } from "../../utils/formatters";
import { Button } from "../ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "../ui/dialog";
import { ClientIcon, clientName } from "./ClientIcon";

export function SessionTrash({ open, onOpenChange, onRestored }: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 恢复成功后通知父页重拉列表。 */
  onRestored: () => void;
}) {
  const { t, i18n } = useTranslation();
  const [rows, setRows] = useState<ClientUsageHiddenSessionRow[] | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!open) return;
    let alive = true;
    setRows(null);
    setError("");
    clientUsageApi.hiddenSessions()
      .then((r) => { if (alive) setRows(r); })
      .catch((e) => { if (alive) { setRows([]); setError(String(e)); } });
    return () => { alive = false; };
  }, [open]);

  const restore = async (r: ClientUsageHiddenSessionRow) => {
    setError("");
    try {
      await clientUsageApi.sessionUnhide(r.clientCode, r.sessionId);
      setRows((rs) => (rs ?? []).filter((x) => !(x.clientCode === r.clientCode && x.sessionId === r.sessionId)));
      onRestored();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl" data-testid="sessions-trash-dialog">
        <DialogHeader>
          <DialogTitle>{t("sessions.trash", "Trash")}</DialogTitle>
          <DialogDescription>{t("sessions.trashDesc", "Hidden sessions are only removed from the list; client files and usage records are kept.")}</DialogDescription>
        </DialogHeader>
        {error && (
          <p role="alert" className="text-sm break-all text-destructive" data-testid="sessions-trash-error">
            {t("sessions.trashFailed", "Trash action failed: {{error}}", { error })}
          </p>
        )}
        {rows === null ? (
          <div className="h-16 animate-pulse rounded-lg bg-muted" aria-busy="true" />
        ) : rows.length === 0 ? (
          <p className="py-6 text-center text-sm text-muted-foreground" data-testid="sessions-trash-empty">
            {t("sessions.trashEmpty", "Trash is empty")}
          </p>
        ) : (
          <ul className="max-h-[60vh] divide-y overflow-y-auto">
            {rows.map((r) => (
              <li
                key={`${r.clientCode}:${r.sessionId}`}
                className="flex items-center gap-3 py-2.5"
                data-testid="sessions-trash-row"
                data-session={r.sessionId}
              >
                <ClientIcon code={r.clientCode} size={16} />
                <div className="min-w-0 flex-1">
                  <div className="truncate" title={r.title || r.sessionId}><bdi data-testid="sessions-trash-title">{r.title || r.sessionId}</bdi></div>
                  {/* bdi 隔离各段方向：RTL 下路径固定 ltr、不与「·」和相对时间混排（票 13） */}
                  <div className="truncate text-sm text-muted-foreground">
                    <bdi>{clientName(r.clientCode)}</bdi>
                    {r.workdir && <>{" · "}<bdi dir="ltr" title={r.workdir} data-testid="sessions-trash-path">{r.workdir}</bdi></>}
                    {" · "}
                    <bdi>{t("sessions.hiddenAt", "Hidden {{time}}", { time: formatRelativeTime(r.hiddenAt, i18n.language) })}</bdi>
                  </div>
                </div>
                <Button size="sm" variant="outline" data-testid="sessions-trash-restore" onClick={() => void restore(r)}>
                  {t("sessions.restore", "Restore")}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </DialogContent>
    </Dialog>
  );
}
