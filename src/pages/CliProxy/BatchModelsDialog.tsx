import { useTranslation } from "react-i18next";
import { makeRipple } from "@/utils/motion";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
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

// 批量覆盖 models。shadcn Dialog（Radix Portal → document.body，portal 用法从原文件原样搬出）。
interface BatchModelsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selectedCount: number;
  value: string;
  onChange: (v: string) => void;
  busy: boolean;
  onSave: () => void;
}

export function BatchModelsDialog({
  open, onOpenChange, selectedCount, value, onChange, busy, onSave,
}: BatchModelsDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="glass-elevated" style={{ maxWidth: 520, padding: 20 }}>
        <DialogHeader>
          <DialogTitle style={{ fontSize: 15, fontWeight: 600 }}>
            {t("cliProxy.batchModelsTitle")}
          </DialogTitle>
          <DialogDescription style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
            {t("cliProxy.selectedCount", { count: selectedCount })}
          </DialogDescription>
        </DialogHeader>
        <label style={fieldLabel}>
          <Textarea
            style={{ minHeight: 120, resize: "vertical" }}
            value={value}
            onChange={e => onChange(e.target.value)}
            placeholder={t("cliProxy.batchModelsPlaceholder")}
          />
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
