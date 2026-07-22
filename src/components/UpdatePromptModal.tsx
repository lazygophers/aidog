// ─── 发现新版本提醒 modal ───────────────────────────────────
// 自定义 in-app modal (禁原生 confirm/alert，破坏 Tauri)。
// shadcn Dialog (Radix Portal) satisfies createPortal(document.body) centering rule.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { Update } from "@tauri-apps/plugin-updater";
import { runUpdate } from "../services/updater";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

export interface UpdatePromptModalProps {
  /** check() 返回的可用更新。 */
  update: Update;
  /** 「稍后」或更新启动后关闭。 */
  onClose: () => void;
}

export function UpdatePromptModal({ update, onClose }: UpdatePromptModalProps) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const handleUpdate = async () => {
    setBusy(true);
    setError("");
    try {
      await runUpdate(update);
      // relaunch() 成功会重启应用，下面通常不会执行到。
      onClose();
    } catch (e) {
      setBusy(false);
      setError(`${t("updater.updateFailed", "更新失败")}: ${String(e)}`);
    }
  };

  // ponytail: 原 Modal closeOnEscape={false} + closeOnBackdrop={!busy} → Radix Dialog 统一
  // onOpenChange，busy 时拦截所有非用户主动关闭（backdrop/escape），非 busy 时允许。
  return (
    <Dialog open onOpenChange={(o) => { if (!o && busy) return; if (!o) onClose(); }}>
      <DialogContent className="glass-elevated" style={{ maxWidth: 460, padding: "22px 24px" }}>
        <DialogHeader>
          <DialogTitle style={{ fontSize: 16, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("updater.foundTitle", "发现新版本")} v{update.version}
          </DialogTitle>
          {update.body && (
            <DialogDescription
              asChild
              style={{
                fontSize: 13,
                color: "var(--text-secondary)",
                lineHeight: 1.6,
                maxHeight: 220,
                overflowY: "auto",
                whiteSpace: "pre-wrap",
                wordBreak: "break-word",
              }}
            >
              <div>
                <div style={{ fontWeight: 600, marginBottom: 4, color: "var(--text-primary)" }}>
                  {t("updater.releaseNotes", "更新内容")}
                </div>
                {update.body}
              </div>
            </DialogDescription>
          )}
        </DialogHeader>
        {error && (
          <div style={{ fontSize: 12, color: "var(--color-danger)", marginBottom: 12, wordBreak: "break-all" }}>
            {error}
          </div>
        )}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <Button
            variant="ghost"
            style={{ fontSize: 13, padding: "6px 14px" }}
            onClick={onClose}
            disabled={busy}
          >
            {t("updater.later", "稍后")}
          </Button>
          <Button
            variant="default"
            style={{ fontSize: 13, padding: "6px 14px", minWidth: 96 }}
            onClick={handleUpdate}
            disabled={busy}
          >
            {busy
              ? t("updater.updating", "更新中…")
              : t("updater.updateNow", "立即更新")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
