// ─── 发现新版本提醒 modal ───────────────────────────────────
// 自定义 in-app modal (禁原生 confirm/alert，破坏 Tauri)。
// shadcn Dialog (Radix Portal) satisfies createPortal(document.body) centering rule.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { Update } from "@tauri-apps/plugin-updater";
import { abortDrain, beginDrain, downloadUpdate, installUpdate, relaunchWhenDrained } from "../services/updater";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { makeRipple } from "../utils/motion";
import { formatBytes } from "../utils/formatters";
import { Progress } from "@/components/ui/progress";

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
  const [progress, setProgress] = useState<{ downloaded: number; contentLength?: number; phase: "downloading" | "ready" | "installing" | "draining" } | null>(null);

  const handleDownload = async () => {
    setBusy(true);
    setError("");
    setProgress({ downloaded: 0, phase: "downloading" });
    try {
      await downloadUpdate(update, (downloaded, contentLength) => {
        setProgress({ downloaded, contentLength, phase: "downloading" });
      });
      // download() 返回前已完成签名校验；安装必须等用户确认。
      setProgress((p) => ({ downloaded: p?.downloaded ?? 0, contentLength: p?.contentLength, phase: "ready" }));
      setBusy(false);
    } catch (e) {
      setBusy(false);
      setError(`${t("updater.updateFailed", "Update failed")}: ${String(e)}`);
    }
  };

  const handleInstall = async () => {
    setBusy(true);
    setError("");
    try {
      // 确认即拒新请求；确认前不排空（在途请求不受影响）。
      await beginDrain();
      setProgress((p) => p && { ...p, phase: "installing" });
      await installUpdate(update);
      setProgress((p) => p && { ...p, phase: "draining" });
      // 在途清零后内核侧重启进新版，应用随后退出。
      await relaunchWhenDrained();
    } catch (e) {
      // 排空已置位：失败即解除，恢复代理正常服务；界面回到 ready 可重试。
      await abortDrain().catch(() => {});
      setBusy(false);
      setProgress((p) => p && { ...p, phase: "ready" });
      setError(`${t("updater.updateFailed", "Update failed")}: ${String(e)}`);
    }
  };
  // 已知总大小 → 百分比 + <progress> 语义；未知 → 只报已下载字节，不伪造百分比。
  const pct = progress?.contentLength ? Math.min(100, Math.round((progress.downloaded / progress.contentLength) * 100)) : null;

  // ponytail: 原 Modal closeOnEscape={false} + closeOnBackdrop={!busy} → Radix Dialog 统一
  // onOpenChange，busy 时拦截所有非用户主动关闭（backdrop/escape），非 busy 时允许。
  return (
    <Dialog open onOpenChange={(o) => { if (!o && busy) return; if (!o) onClose(); }}>
      <DialogContent className="glass-elevated" style={{ maxWidth: 460, padding: "22px 24px" }}>
        <DialogHeader>
          <DialogTitle style={{ fontSize: 16, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("updater.foundTitle", "New version available")} v{update.version}
          </DialogTitle>
          {update.body && (
            <DialogDescription
              asChild
              style={{
                fontSize: 14,
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
                  {t("updater.releaseNotes", "Release notes")}
                </div>
                {update.body}
              </div>
            </DialogDescription>
          )}
        </DialogHeader>
        {progress && (
          <div data-testid="update-progress" data-phase={progress.phase} data-pct={pct ?? undefined} style={{ marginBottom: 12, display: "flex", flexDirection: "column", gap: 6 }}>
            <div aria-live="polite" style={{ fontSize: 13, color: "var(--text-secondary)" }}>
              {progress.phase === "downloading"
                ? t("updater.downloading", "Downloading update")
                : progress.phase === "ready"
                  ? t("updater.readyToRestart", "Update downloaded and verified. Restart to install.")
                  : progress.phase === "draining"
                    ? t("updater.draining", "Waiting for in-flight requests to finish, then restarting…")
                    : t("updater.installing", "Installing update…")}
            </div>
            {progress.phase === "downloading" ? (
              <>
                {pct !== null ? (
                  <>
                    <Progress value={pct} aria-label={t("updater.downloading", "Downloading")} />
                    <div style={{ fontSize: 13, color: "var(--text-secondary)", fontVariantNumeric: "tabular-nums" }}>
                      {t("updater.downloadProgress", "{{downloaded}} of {{total}} ({{pct}}%)", {
                        downloaded: formatBytes(progress.downloaded),
                        total: formatBytes(progress.contentLength!),
                        pct,
                      })}
                    </div>
                  </>
                ) : (
                  <div style={{ fontSize: 13, color: "var(--text-secondary)", fontVariantNumeric: "tabular-nums" }}>
                    {t("updater.downloadedUnknown", "{{downloaded}} downloaded…", { downloaded: formatBytes(progress.downloaded) })}
                  </div>
                )}
              </>
            ) : null}
          </div>
        )}
        {error && (
          <div data-testid="update-error" style={{ fontSize: 14, color: "var(--color-danger)", marginBottom: 12, wordBreak: "break-all" }}>
            {error}
          </div>
        )}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <Button
            variant="ghost"
            className="ripple hover-lift"
            style={{ fontSize: 14, padding: "6px 14px" }}
            onClick={(e) => { makeRipple(e); abortDrain().catch(() => {}); onClose(); }}
            disabled={busy}
            data-testid="update-later"
          >
            {t("updater.later", "Later")}
          </Button>
          <Button
            variant="default"
            className="ripple hover-lift"
            style={{
              fontSize: 14,
              padding: "6px 14px",
              minWidth: 96,
              // 萤火虫：主 CTA 流光描边 + 呼吸 glow
              boxShadow: "0 0 12px color-mix(in srgb, var(--accent) 28%, transparent)",
            }}
            onClick={(e) => { makeRipple(e); progress?.phase === "ready" ? handleInstall() : handleDownload(); }}
            disabled={busy || progress?.phase === "installing"}
            data-testid={progress?.phase === "ready" ? "update-restart" : "update-start"}
          >
            {busy
              ? t("updater.updating", "Updating…")
              : progress?.phase === "ready"
                ? t("updater.restartNow", "Restart and install")
                : t("updater.updateNow", "Update now")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
