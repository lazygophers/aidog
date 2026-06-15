// ─── 发现新版本提醒 modal ───────────────────────────────────
// 自定义 in-app modal (禁原生 confirm/alert，破坏 Tauri)。
// 视觉沿用 UnsavedChangesModal 的 overlay + glass-elevated 语言。
// 不用 transform 终态 (css-transform-breaks-fixed-modal)；glass-elevated 走 var(--bg-floating) 不透明。

import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { Update } from "@tauri-apps/plugin-updater";
import { runUpdate } from "../services/updater";

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
      onClick={busy ? undefined : onClose}
    >
      <div
        className="glass-elevated"
        style={{
          width: 460,
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
            fontSize: 16,
            fontWeight: 600,
            color: "var(--text-primary)",
            marginBottom: 8,
          }}
        >
          {t("updater.foundTitle", "发现新版本")} v{update.version}
        </div>
        {update.body && (
          <div
            style={{
              fontSize: 13,
              color: "var(--text-secondary)",
              lineHeight: 1.6,
              marginBottom: 16,
              maxHeight: 220,
              overflowY: "auto",
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
            }}
          >
            <div style={{ fontWeight: 600, marginBottom: 4, color: "var(--text-primary)" }}>
              {t("updater.releaseNotes", "更新内容")}
            </div>
            {update.body}
          </div>
        )}
        {error && (
          <div style={{ fontSize: 12, color: "var(--color-danger)", marginBottom: 12, wordBreak: "break-all" }}>
            {error}
          </div>
        )}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 13, padding: "6px 14px" }}
            onClick={onClose}
            disabled={busy}
          >
            {t("updater.later", "稍后")}
          </button>
          <button
            className="btn btn-primary"
            style={{ fontSize: 13, padding: "6px 14px", minWidth: 96 }}
            onClick={handleUpdate}
            disabled={busy}
          >
            {busy
              ? t("updater.updating", "更新中…")
              : t("updater.updateNow", "立即更新")}
          </button>
        </div>
      </div>
    </div>
  );
}
