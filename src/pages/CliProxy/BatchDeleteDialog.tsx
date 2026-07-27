import { useTranslation } from "react-i18next";
import { makeRipple } from "@/utils/motion";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import type { CliProxyProvider } from "@/services/api";

// 批量删除确认。用 shadcn AlertDialog（Radix Portal → document.body，满足弹窗窗口居中规则，
// 见 memory modal-window-center-rule），从 CliProxy/index.tsx 原地搬出，portal 用法未变。
interface BatchDeleteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  providers: CliProxyProvider[];
  selectedIds: Set<number>;
  busy: boolean;
  onConfirm: () => void;
}

export function BatchDeleteDialog({
  open, onOpenChange, providers, selectedIds, busy, onConfirm,
}: BatchDeleteDialogProps) {
  const { t } = useTranslation();
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent className="glass-elevated" style={{ maxWidth: 420, padding: 20 }}>
        <AlertDialogHeader>
          <AlertDialogTitle style={{ fontSize: 15, fontWeight: 600 }}>
            {t("cliProxy.batchDeleteTitle")}
          </AlertDialogTitle>
          <AlertDialogDescription style={{ fontSize: 13, color: "var(--text-secondary)" }}>
            {selectedIds.size <= 5
              ? providers
                  .filter(p => selectedIds.has(p.id))
                  .map(p => `${p.name} (${p.wire_protocol})`)
                  .join("、")
              : t("cliProxy.batchDeleteConfirm", { count: selectedIds.size })}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
          {t("cliProxy.batchDeleteConfirm", { count: selectedIds.size })}
        </div>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy}>{t("cliProxy.cancel")}</AlertDialogCancel>
          <AlertDialogAction
            className="ripple"
            onClick={(e) => { makeRipple(e); onConfirm(); }}
            disabled={busy}
            style={{ background: "var(--color-danger)" }}
          >
            {t("cliProxy.batchDelete")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
