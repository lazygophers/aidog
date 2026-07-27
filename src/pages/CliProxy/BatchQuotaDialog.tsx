import { useTranslation } from "react-i18next";
import { makeRipple } from "@/utils/motion";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog";

const fieldLabel: React.CSSProperties = {
  display: "flex", flexDirection: "column", gap: 4,
  fontSize: 12, color: "var(--text-secondary)",
};

// 批量设置 quota。shadcn Dialog（Radix Portal → document.body，portal 用法从原文件原样搬出）。
interface BatchQuotaDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selectedCount: number;
  quotaType: "none" | "newapi";
  onQuotaTypeChange: (v: "none" | "newapi") => void;
  busy: boolean;
  onSave: () => void;
}

export function BatchQuotaDialog({
  open, onOpenChange, selectedCount, quotaType, onQuotaTypeChange, busy, onSave,
}: BatchQuotaDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="glass-elevated" style={{ maxWidth: 420, padding: 20 }}>
        <DialogHeader>
          <DialogTitle style={{ fontSize: 15, fontWeight: 600 }}>
            {t("cliProxy.batchQuotaTitle")}
          </DialogTitle>
          <DialogDescription style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
            {t("cliProxy.selectedCount", { count: selectedCount })}
          </DialogDescription>
        </DialogHeader>
        <label style={fieldLabel}>
          {t("cliProxy.quotaType")}
          <Select
            value={quotaType}
            onValueChange={v => onQuotaTypeChange(v as "none" | "newapi")}
          >
            <SelectTrigger style={{ width: "100%" }}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="none">{t("cliProxy.quotaTypeNone")}</SelectItem>
              <SelectItem value="newapi">{t("cliProxy.quotaTypeNewapi")}</SelectItem>
            </SelectContent>
          </Select>
        </label>
        <DialogFooter>
          <Button
            variant="ghost"
            className="ripple"
            onClick={(e) => { makeRipple(e); onOpenChange(false); }}
            disabled={busy}
          >
            {t("cliProxy.cancel")}
          </Button>
          <Button
            variant="default"
            className="ripple"
            onClick={(e) => { makeRipple(e); onSave(); }}
            disabled={busy}
          >
            {t("cliProxy.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
