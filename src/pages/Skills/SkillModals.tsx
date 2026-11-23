import { type SkillAgent } from "../../services/api";
import { SkillDetailView } from "../SkillDetailView";
import { ShareModal } from "../../components/platforms/ShareModal";
import { makeRipple } from "@/utils/motion";
import { AGENTS, AGENT_ICONS } from "./constants";
import type { SkillsData } from "./useSkillsData";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
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
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

/**
 * 全部 modal（自原 Skills.tsx L973-1304 外迁，全部 Radix Portal 弹窗聚簇）：
 * confirmUninstall / uninstallSingle / align / detail / share / pasteImport / importConfirm。
 *
 * 破坏性二次确认走 AlertDialog（Radix Portal，禁原生 confirm）；
 * 编辑/输入类走 Dialog（Radix Portal）。脱离 Skills 页 transform 祖先，fixed 始终相对 viewport 居中。
 * 7 弹窗 createPortal 满足核查：AlertDialog×2 + Dialog×3 + SkillDetailView(内部 Radix Dialog) +
 * ShareModal(内部 Radix Dialog) = 全 Radix Portal，已满足 modal-window-center-rule。
 */
export function SkillModals({ s }: { s: SkillsData }) {
  const {
    t, env, installed,
    confirmUninstall, setConfirmUninstall, handleUninstallAll,
    uninstallTarget, setUninstallTarget, handleUninstallSingle,
    selectedNames, confirmUninstallBatch, setConfirmUninstallBatch, handleUninstallBatch,
    alignOpen, setAlignOpen, alignFrom, setAlignFrom, alignTo, setAlignTo, handleAlign,
    detailTarget, setDetailTarget,
    shareData, setShareData, setMessage,
    pasteOpen, setPasteOpen, pasteText, setPasteText, handlePasteImport,
    importIds, setImportIds, importAgents, setImportAgents, importScopeKind, setImportScopeKind,
    importProjectPath, setImportProjectPath, importBusy, handleImport,
  } = s;

  return (
    <>
      {/* 一键卸载二次确认 AlertDialog（破坏性，禁 native confirm） */}
      <AlertDialog open={confirmUninstall} onOpenChange={setConfirmUninstall}>
        <AlertDialogContent className="glass-elevated" style={{ maxWidth: 380, padding: 24 }}>
          <AlertDialogHeader>
            <AlertDialogTitle style={{ fontSize: 15, fontWeight: 700 }}>
              {t("skills.uninstallAll", "Uninstall All")}
            </AlertDialogTitle>
            <AlertDialogDescription style={{ fontSize: 14, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t("skills.uninstallAllConfirm", "This will remove all {{count}} skills across all platforms in the current scope. This cannot be undone. Continue?", { count: installed.length })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel style={{ fontSize: 14 }}>
              {t("action.cancel", "Cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => { makeRipple(e); handleUninstallAll(); }}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90 ripple"
              style={{ fontSize: 14 }}
            >
              {t("skills.uninstallAll", "Uninstall All")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 单条卸载二次确认 AlertDialog（破坏性，禁 native confirm） */}
      <AlertDialog
        open={uninstallTarget !== null}
        onOpenChange={(next) => { if (!next) setUninstallTarget(null); }}
      >
        <AlertDialogContent className="glass-elevated" style={{ maxWidth: 380, padding: 24 }}>
          <AlertDialogHeader>
            <AlertDialogTitle style={{ fontSize: 15, fontWeight: 700 }}>
              {t("skills.uninstall", "Uninstall")}
            </AlertDialogTitle>
            <AlertDialogDescription style={{ fontSize: 14, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {uninstallTarget && t("skills.uninstallConfirm", "This will remove skill {{name}} and its enabled config for all agents. This cannot be undone. Confirm?", { name: uninstallTarget.name })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel style={{ fontSize: 14 }}>
              {t("action.cancel", "Cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => { makeRipple(e); handleUninstallSingle(); }}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90 ripple"
              style={{ fontSize: 14 }}
            >
              {t("skills.uninstall", "Uninstall")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 批量卸载二次确认 AlertDialog（破坏性，禁 native confirm） */}
      <AlertDialog open={confirmUninstallBatch} onOpenChange={setConfirmUninstallBatch}>
        <AlertDialogContent className="glass-elevated" style={{ maxWidth: 380, padding: 24 }}>
          <AlertDialogHeader>
            <AlertDialogTitle style={{ fontSize: 15, fontWeight: 700 }}>
              {t("skills.uninstallSelected", "Uninstall selected ({{count}})", { count: selectedNames.size })}
            </AlertDialogTitle>
            <AlertDialogDescription style={{ fontSize: 14, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t("skills.uninstallSelectedConfirm", "This will permanently delete the {{count}} selected skills and their enablement config across all agents. Continue?", { count: selectedNames.size })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel style={{ fontSize: 14 }}>
              {t("action.cancel", "Cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => { makeRipple(e); handleUninstallBatch(); }}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90 ripple"
              style={{ fontSize: 14 }}
            >
              {t("skills.uninstall", "Uninstall")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 对齐配置 Dialog：使 to 与 from 启用配置一致 */}
      <Dialog open={alignOpen} onOpenChange={setAlignOpen}>
        <DialogContent className="glass-elevated" style={{ maxWidth: 400, padding: 24, gap: 16 }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 15, fontWeight: 700 }}>
              {t("skills.alignTitle", "Align Config")}
            </DialogTitle>
          </DialogHeader>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 14 }}>
              <span style={{ minWidth: 72 }} className="text-secondary">{t("skills.alignFrom", "Source agent")}</span>
              <Select value={alignFrom} onValueChange={(v) => setAlignFrom(v as SkillAgent)}>
                <SelectTrigger style={{ flex: 1, fontSize: 14 }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {AGENTS.map((a) => (
                    <SelectItem key={a} value={a}>{t(`skills.agent.${a}`, a)}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 14 }}>
              <span style={{ minWidth: 72 }} className="text-secondary">{t("skills.alignTo", "Target agent")}</span>
              <Select value={alignTo} onValueChange={(v) => setAlignTo(v as SkillAgent)}>
                <SelectTrigger style={{ flex: 1, fontSize: 14 }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {AGENTS.map((a) => (
                    <SelectItem key={a} value={a}>{t(`skills.agent.${a}`, a)}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <div style={{ fontSize: 14, color: "var(--text-secondary)", lineHeight: 1.6 }}>
            {alignFrom === alignTo
              ? t("skills.alignSameAgent", "Source and target must differ")
              : t("skills.alignConfirm", "Will make {{to}}'s enabled skills match {{from}} exactly (enable what {{from}} has, disable what {{from}} lacks).", {
                  from: t(`skills.agent.${alignFrom}`, alignFrom),
                  to: t(`skills.agent.${alignTo}`, alignTo),
                })}
          </div>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <Button variant="outline" style={{ fontSize: 14 }} onClick={() => setAlignOpen(false)}>
              {t("action.cancel", "Cancel")}
            </Button>
            <Button
              className="ripple"
              style={{ fontSize: 14 }}
              disabled={alignFrom === alignTo}
              onClick={(e) => { makeRipple(e); handleAlign(); }}
            >
              {t("skills.alignTitle", "Align Config")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* 已装 skill 详情（只读 Dialog，SkillDetailView 内部自渲染 Radix Portal） */}
      {detailTarget !== null && (
        <SkillDetailView skill={detailTarget} onClose={() => setDetailTarget(null)} />
      )}

      {/* 分享 modal（泛化 ShareModal，3 格式切换 + 复制为 aidog://skill/import 链接） */}
      {shareData && (
        <ShareModal
          share={shareData.share}
          title={shareData.name}
          titleKey="skills.share.title"
          warningKey="skills.share.warning"
          urlScheme="aidog://skill/import"
          copyUrlKey="skills.share.copyUrl"
          onToast={(text) => setMessage(text)}
          onClose={() => setShareData(null)}
        />
      )}

      {/* 粘贴分享文本导入 Dialog */}
      <Dialog open={pasteOpen} onOpenChange={setPasteOpen}>
        <DialogContent className="glass-elevated" style={{ maxWidth: "min(560px, 90vw)", padding: 20, gap: 10 }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 15, fontWeight: 700 }}>
              {t("skills.pasteTitle", "Import from share")}
            </DialogTitle>
          </DialogHeader>
          <div style={{ fontSize: 14, color: "var(--text-secondary)", lineHeight: 1.5 }}>
            {t("skills.pasteHint", "Paste a shared skill text (base64 / JSON / the data portion of an aidog:// link).")}
          </div>
          <Textarea
            style={{ minHeight: 160, fontFamily: "var(--font-mono, monospace)", resize: "vertical" }}
            value={pasteText}
            placeholder={t("skills.pastePlaceholder", "aidog://skill/import?data=..., base64, or a JSON array")}
            onChange={(e) => setPasteText(e.target.value)}
          />
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
            <Button variant="outline" style={{ fontSize: 14 }} onClick={() => setPasteOpen(false)}>
              {t("action.cancel", "Cancel")}
            </Button>
            <Button
              className="ripple"
              style={{ fontSize: 14 }}
              onClick={(e) => { makeRipple(e); handlePasteImport(); }}
              disabled={!pasteText.trim()}
            >
              {t("skills.importBtn", "Import")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* 批量导入确认 Dialog（列出将装 skill id + scope/agents 选择） */}
      <Dialog
        open={importIds !== null}
        onOpenChange={(next) => { if (!next && !importBusy) setImportIds(null); }}
      >
        <DialogContent className="glass-elevated" style={{ maxWidth: "min(560px, 92vw)", maxHeight: "82vh", padding: 22, gap: 12, overflow: "auto" }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 15, fontWeight: 700 }}>
              {t("skills.importConfirmTitle", "Confirm skill import")}
            </DialogTitle>
          </DialogHeader>
          <div style={{ fontSize: 14, color: "var(--text-secondary)", lineHeight: 1.5 }}>
            {importIds !== null && t("skills.importConfirmDesc", "Will install the following {{count}} skill(s) via npx skills add (requires network):", { count: importIds.length })}
          </div>
          {importIds !== null && (
            <div style={{ display: "flex", flexDirection: "column", gap: 4, maxHeight: "30vh", overflow: "auto", padding: 8, border: "1px solid var(--border)", borderRadius: 8, background: "var(--bg-glass)" }}>
              {importIds.map((id) => (
                <div key={id} style={{ fontSize: 14, fontFamily: "var(--font-mono, monospace)" }}>{id}</div>
              ))}
            </div>
          )}
          {/* scope 选择 */}
          <div style={{ display: "flex", gap: 8, alignItems: "center", fontSize: 14 }}>
            <span className="text-secondary" style={{ minWidth: 56 }}>{t("skills.scope", "Scope")}</span>
            <Select
              value={importScopeKind}
              onValueChange={(v) => setImportScopeKind(v as "global" | "project")}
              disabled={importBusy}
            >
              <SelectTrigger style={{ width: "auto", fontSize: 14 }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="global">{t("skills.scopeGlobal", "User (global)")}</SelectItem>
                <SelectItem value="project">{t("skills.scopeProject", "Project")}</SelectItem>
              </SelectContent>
            </Select>
            {importScopeKind === "project" && (
              <Input
                style={{ flex: 1 }}
                placeholder={t("skills.projectPathPlaceholder", "Absolute project directory path")}
                value={importProjectPath}
                onChange={(e) => setImportProjectPath(e.target.value)}
                disabled={importBusy}
              />
            )}
          </div>
          {/* agent 多选 */}
          <div style={{ display: "flex", gap: 10, alignItems: "center", fontSize: 14, flexWrap: "wrap" }}>
            <span className="text-secondary" style={{ minWidth: 56 }}>{t("skills.importAgents", "Target agents")}</span>
            {AGENTS.map((a) => {
              const on = importAgents.has(a);
              return (
                <Button
                  key={a}
                  type="button"
                  variant={on ? "default" : "outline"}
                  className="ripple"
                  onClick={(e) => {
                    makeRipple(e);
                    setImportAgents((prev) => {
                      const next = new Set(prev);
                      if (next.has(a)) next.delete(a);
                      else next.add(a);
                      return next;
                    });
                  }}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    padding: "4px 10px",
                    height: "auto",
                    opacity: on ? 1 : 0.5,
                    fontSize: 14,
                  }}
                >
                  <img src={AGENT_ICONS[a]} alt={a} className="hover-lift" style={{ width: 16, height: 16 }} />
                  {t(`skills.agent.${a}`, a)}
                </Button>
              );
            })}
          </div>
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
            <Button variant="outline" style={{ fontSize: 14 }} onClick={() => setImportIds(null)} disabled={importBusy}>
              {t("action.cancel", "Cancel")}
            </Button>
            <Button
              className="ripple"
              style={{ fontSize: 14 }}
              onClick={(e) => { makeRipple(e); void handleImport(); }}
              disabled={importBusy || importAgents.size === 0 || (importScopeKind === "project" && importProjectPath.trim() === "") || !env?.npx_available}
            >
              {importBusy ? t("skills.installing", "Importing…") : t("skills.importBtn", "Import")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
