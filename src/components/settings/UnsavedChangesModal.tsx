// ─── Unsaved changes confirm modal ──────────────────────────
// Custom in-app confirm (NOT browser `confirm`, which breaks Tauri).
// Mirrors the ImportDiffModal overlay/glass-elevated visual language.

import { useTranslation } from "react-i18next";
import { F, S } from "./editors";

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
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1100,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "rgba(0,0,0,0.5)",
        animation: "fadeIn 150ms ease both",
      }}
      onClick={onCancel}
    >
      <div
        className="glass-elevated"
        style={{
          width: 420,
          maxWidth: "90vw",
          display: "flex",
          flexDirection: "column",
          borderRadius: "var(--radius-lg)",
          animation: "fadeIn 200ms ease both",
          padding: "22px 24px",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div
          style={{
            fontSize: F.title,
            fontWeight: 600,
            color: "var(--text-primary)",
            marginBottom: 8,
          }}
        >
          {t("settings.unsavedTitle", "未保存的更改")}
        </div>
        <div
          style={{
            fontSize: F.body,
            color: "var(--text-secondary)",
            lineHeight: 1.6,
            marginBottom: 20,
          }}
        >
          {t(
            "settings.unsavedBody",
            "你有尚未保存的更改。离开此页面前要如何处理？",
          )}
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button
            className="btn btn-ghost"
            style={{ fontSize: F.body, padding: S.btnPad }}
            onClick={onCancel}
            disabled={saving}
          >
            {t("action.cancel", "取消")}
          </button>
          <button
            className="btn btn-ghost"
            style={{ fontSize: F.body, padding: S.btnPad, color: "var(--color-danger)" }}
            onClick={onDiscard}
            disabled={saving}
          >
            {t("settings.discardChanges", "放弃更改")}
          </button>
          <button
            className="btn btn-primary"
            style={{ fontSize: F.body, padding: S.btnPad, minWidth: 96 }}
            onClick={onSave}
            disabled={saving}
          >
            {saving
              ? t("status.loading", "加载中…")
              : t("settings.saveAndLeave", "保存并离开")}
          </button>
        </div>
      </div>
    </div>
  );
}
