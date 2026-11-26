// 内核更新区块（settings「API」tab，magpie 票 07）。
// 纯操作界面：状态/进度/失败原因全部来自内核管理接口轮询，本地不持任何更新逻辑。
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { kernelUpdateApi, type KernelUpdateStatus } from "../../services/api";
import { formatBytes } from "../../utils/formatters";
import { Button } from "@/components/ui/button";
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

const POLL_MS = 1000;

export function KernelUpdateSection() {
  const { t } = useTranslation();
  const [st, setSt] = useState<KernelUpdateStatus | null>(null);
  const [unreachable, setUnreachable] = useState(false); // 从未拿到过状态（含桌面 IPC 无此命令）
  const [available, setAvailable] = useState<string | null>(null); // check 出的新版本
  const [installed, setInstalled] = useState(false); // install 已成功（重启待生效）
  const [upToDate, setUpToDate] = useState(false);
  const [busy, setBusy] = useState(false); // 任一操作 RPC 在途：重复触发互斥
  const [actionErr, setActionErr] = useState("");
  const [pendingConfirm, setPendingConfirm] = useState(false);
  const restarting = useRef(false); // 内核退出期间轮询会失败，凭它撑住「重启中」显示
  const gotStatus = useRef(false);

  const refresh = useCallback(async () => {
    try {
      const s = await kernelUpdateApi.status();
      gotStatus.current = true;
      setSt(s);
      setUnreachable(false);
      restarting.current = s.phase.phase === "restarting";
    } catch {
      // 已有状态时轮询失败不覆盖旧显示：重启中连接掐断是预期路径
      if (!gotStatus.current && !restarting.current) setUnreachable(true);
    }
  }, []);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, POLL_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  const phase = st?.phase.phase ?? "idle";
  const run = async (fn: () => Promise<unknown>) => {
    if (busy) return;
    setBusy(true);
    setActionErr("");
    try {
      await fn();
    } catch (e) {
      setActionErr(`${t("kernelUpdate.actionFailed", "Action failed")}: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  const check = () =>
    run(async () => {
      const r = await kernelUpdateApi.check();
      setAvailable(r.available ? (r.version ?? "?") : null);
      setUpToDate(!r.available);
    });
  const download = () => run(() => kernelUpdateApi.download());
  const install = () =>
    run(async () => {
      await kernelUpdateApi.install();
      setInstalled(true);
    });
  const confirm = () => {
    setPendingConfirm(false);
    restarting.current = true;
    run(() => kernelUpdateApi.confirm());
  };

  // 各动作的可用相（与内核 try_begin / ReadyToInstall 门槛一致）；busy 兜住双击窗口。
  const canCheck = phase === "idle" || phase === "failed";
  const canDownload = canCheck && available !== null;
  const canInstall = phase === "ready_to_install";
  const dl = st?.phase.phase === "downloading" ? st.phase : null;
  const progress = dl
    ? dl.total !== null
      ? `${formatBytes(dl.downloaded)} / ${formatBytes(dl.total)}`
      : formatBytes(dl.downloaded)
    : "";

  return (
    <div
      className="glass-surface"
      data-testid="kernel-update-section"
      data-phase={phase}
      style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 10 }}
    >
      <div>
        <div style={{ fontSize: 14, fontWeight: 600 }}>{t("kernelUpdate.title", "Kernel Update")}</div>
        <div className="text-secondary" style={{ fontSize: 14, marginTop: 2 }}>
          {t(
            "kernelUpdate.desc",
            "Checks the running kernel's new version and drives its self-update flow via the management API.",
          )}
        </div>
      </div>

      {unreachable && (
        <div className="text-secondary" data-testid="kernel-update-unreachable" style={{ fontSize: 14 }}>
          {t("kernelUpdate.unavailable", "Kernel management API unreachable")}
        </div>
      )}

      {st && (
        <>
          <div className="text-secondary" data-testid="kernel-update-version" data-version={st.current_version} style={{ fontSize: 14 }}>
            {t("kernelUpdate.currentVersion", { version: st.current_version, defaultValue: "Current version v{{version}}" })}
          </div>

          {phase === "checking" && (
            <div className="text-secondary" style={{ fontSize: 14 }}>{t("kernelUpdate.checking", "Checking…")}</div>
          )}
          {dl && (
            <div data-testid="kernel-update-progress" style={{ fontSize: 14 }}>
              <div className="text-secondary">{t("kernelUpdate.downloading", { progress, defaultValue: "Downloading {{progress}}" })}</div>
              {dl.total !== null && dl.total > 0 && (
                <div style={{ marginTop: 6, height: 4, borderRadius: 2, background: "var(--border)", overflow: "hidden" }}>
                  <div style={{ width: `${Math.min(100, (dl.downloaded / dl.total) * 100)}%`, height: "100%", background: "var(--primary)" }} />
                </div>
              )}
            </div>
          )}
          {phase === "ready_to_install" && st.phase.phase === "ready_to_install" && (
            <div className="text-secondary" data-testid="kernel-update-ready" style={{ fontSize: 14 }}>
              {installed
                ? t("kernelUpdate.restartHint", { version: st.phase.version, defaultValue: "Installed v{{version}}; restart the kernel to apply" })
                : t("kernelUpdate.ready", { version: st.phase.version, defaultValue: "v{{version}} downloaded and verified, ready to install" })}
            </div>
          )}
          {phase === "installing" && (
            <div className="text-secondary" style={{ fontSize: 14 }}>{t("kernelUpdate.installing", "Installing…")}</div>
          )}
          {phase === "restarting" && (
            <div className="text-secondary" data-testid="kernel-update-restarting" style={{ fontSize: 14 }}>
              {t("kernelUpdate.restarting", { count: st.inflight, defaultValue: "Restarting: new requests rejected, draining {{count}} in-flight…" })}
            </div>
          )}
          {phase === "failed" && st.phase.phase === "failed" && (
            <div data-testid="kernel-update-failed" style={{ fontSize: 14, color: "var(--color-danger)" }}>
              {t("kernelUpdate.failedLabel", "Update failed")}: {st.phase.error}
            </div>
          )}
          {phase === "idle" && upToDate && (
            <div className="text-secondary" data-testid="kernel-update-uptodate" style={{ fontSize: 14 }}>
              {t("kernelUpdate.upToDate", "Already up to date")}
            </div>
          )}
          {phase === "idle" && available !== null && (
            <div className="text-secondary" data-testid="kernel-update-available" data-version={available} style={{ fontSize: 14 }}>
              {t("kernelUpdate.available", { version: available, defaultValue: "New version v{{version}} available" })}
            </div>
          )}
          {actionErr && (
            <div data-testid="kernel-update-action-error" style={{ fontSize: 14, color: "var(--color-danger)" }}>
              {actionErr}
            </div>
          )}

          <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
            <Button
              data-testid="kernel-update-check"
              size="sm"
              variant="outline"
              disabled={busy || !canCheck}
              onClick={check}
            >
              {phase === "checking" ? t("kernelUpdate.checking", "Checking…") : t("kernelUpdate.check", "Check for updates")}
            </Button>
            {available !== null && (
              <Button data-testid="kernel-update-download" size="sm" variant="outline" disabled={busy || !canDownload} onClick={download}>
                {t("kernelUpdate.download", "Download update")}
              </Button>
            )}
            {canInstall && (
              <Button data-testid="kernel-update-install" size="sm" variant="outline" disabled={busy} onClick={install}>
                {t("kernelUpdate.install", "Install update")}
              </Button>
            )}
            {canInstall && installed && (
              <Button data-testid="kernel-update-restart" size="sm" disabled={busy} onClick={() => setPendingConfirm(true)}>
                {t("kernelUpdate.restart", "Restart kernel")}
              </Button>
            )}
          </div>
        </>
      )}

      <AlertDialog open={pendingConfirm} onOpenChange={(o) => !o && setPendingConfirm(false)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("kernelUpdate.confirmTitle", "Restart the kernel?")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t(
                "kernelUpdate.confirmDesc",
                "After confirmation the kernel rejects new requests, drains in-flight ones and restarts into the new version. Until you confirm, the kernel keeps serving normally.",
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel data-testid="kernel-update-cancel">{t("common.cancel", "Cancel")}</AlertDialogCancel>
            <AlertDialogAction data-testid="kernel-update-confirm" onClick={confirm}>
              {t("kernelUpdate.restart", "Restart kernel")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
