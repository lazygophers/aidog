// ─── Unsaved changes confirm modal ──────────────────────────
// Custom in-app confirm (NOT browser `confirm`, which breaks Tauri).
// shadcn Dialog (Radix Portal) satisfies createPortal(document.body) centering rule.

import { useTranslation } from "react-i18next";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { F, S } from "./editors";
import { Button } from "@/components/ui/button";

export interface UnsavedChangesModalProps {
  /** Save the draft, then leave. */
  onSave: () => void;
  /** Discard changes and leave. */
  onDiscard: () => void;
  /** Stay on the page. */
  onCancel: () => void;
  /** Disable actions while a save is in flight. */
  saving?: boolean;
}

export function UnsavedChangesModal({
  onSave,
  onDiscard,
  onCancel,
  saving,
}: UnsavedChangesModalProps) {
  const { t } = useTranslation();
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onCancel(); }}>
      <DialogContent data-testid="unsaved-modal" className="glass-elevated" style={{ maxWidth: 420, padding: "22px 24px" }}>
        <DialogHeader>
          <DialogTitle style={{ fontSize: F.title, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("settings.unsavedTitle", "Unsaved changes")}
          </DialogTitle>
          <DialogDescription style={{ fontSize: F.body, color: "var(--text-secondary)", lineHeight: 1.6 }}>
            {t("settings.unsavedBody", "You have unsaved changes. What would you like to do before leaving this page?")}
          </DialogDescription>
        </DialogHeader>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <Button variant="ghost" style={{ fontSize: F.body, padding: S.btnPad }} onClick={onCancel} disabled={saving}>
            {t("action.cancel", "Cancel")}
          </Button>
          <Button data-testid="unsaved-discard" variant="ghost" style={{ fontSize: F.body, padding: S.btnPad, color: "var(--color-danger)" }} onClick={onDiscard} disabled={saving}>
            {t("settings.discardChanges", "Discard changes")}
          </Button>
          <Button variant="default" style={{ fontSize: F.body, padding: S.btnPad, minWidth: 96 }} onClick={onSave} disabled={saving}>
            {saving ? t("status.loading", "Loading...") : t("settings.saveAndLeave", "Save and leave")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
