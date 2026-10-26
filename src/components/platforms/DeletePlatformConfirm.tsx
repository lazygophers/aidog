// DeletePlatformConfirm — 删除平台确认浮层（G3 票 04 / 旧票 03）。
// Dialog 本身经 createPortal(document.body)（ui/dialog 铁律，见 CLAUDE.md modal 条）。
// 文案说清三件事：删什么（密钥/端点/模型配置）、日志保留、不可撤销。
import { useTranslation } from "react-i18next";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

export function DeletePlatformConfirm({ open, name, busy, onConfirm, onCancel }: {
  open: boolean;
  name?: string;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onOpenChange={v => { if (!v) onCancel(); }}>
      <DialogContent id="g3-delete-confirm" data-state={open ? "open" : "closed"} className="max-w-sm">
        <DialogHeader>
          <DialogTitle>{t("platform.deleteConfirm.title", "删除平台？")}</DialogTitle>
        </DialogHeader>
        <div style={{ fontSize: 12.5, color: "var(--text-secondary)", display: "flex", flexDirection: "column", gap: 6 }}>
          {name && <b style={{ color: "var(--foreground)" }}>{name}</b>}
          <span>{t("platform.deleteConfirm.body", "将删除该平台的密钥、端点与模型配置，并移出所在分组。")}</span>
          <span className="text-tertiary">{t("platform.deleteConfirm.logsKept", "历史请求日志保留，不受影响。")}</span>
          <span style={{ color: "var(--color-danger)" }}>{t("platform.deleteConfirm.irreversible", "此操作不可撤销。")}</span>
        </div>
        <DialogFooter>
          <Button variant="ghost" id="g3-delete-cancel" onClick={onCancel}>{t("common.cancel", "取消")}</Button>
          <Button id="g3-delete-ok" data-busy={busy ? "yes" : "no"} disabled={busy}
            style={{ background: "var(--color-danger)", color: "#fff" }} onClick={onConfirm}>
            {t("platform.delete", "删除")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
