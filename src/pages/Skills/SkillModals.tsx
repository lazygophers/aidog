import { type SkillAgent } from "../../services/api";
import { SkillDetailView } from "../SkillDetailView";
import { ShareModal } from "../../components/platforms/ShareModal";
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
 * 全部 modal（自原 Skills.tsx L973-1304 外迁，全部 createPortal 弹窗聚簇）：
 * confirmUninstall / uninstallSingle / align / detail / share / pasteImport / importConfirm。
 *
 * 破坏性二次确认走 AlertDialog（Radix Portal，禁原生 confirm）；
 * 编辑/输入类走 Dialog（Radix Portal）。脱离 Skills 页 transform 祖先，fixed 始终相对 viewport 居中。
 */
export function SkillModals({ s }: { s: SkillsData }) {
  const {
    t, env, installed,
    confirmUninstall, setConfirmUninstall, handleUninstallAll,
    uninstallTarget, setUninstallTarget, handleUninstallSingle,
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
              {t("skills.uninstallAll", "卸载全部")}
            </AlertDialogTitle>
            <AlertDialogDescription style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t("skills.uninstallAllConfirm", "将删除当前范围下所有平台的全部 {{count}} 个 skills，不可恢复。确认？", { count: installed.length })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel style={{ fontSize: 13 }}>
              {t("action.cancel", "取消")}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={handleUninstallAll}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              style={{ fontSize: 13 }}
            >
              {t("skills.uninstallAll", "卸载全部")}
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
              {t("skills.uninstall", "卸载")}
            </AlertDialogTitle>
            <AlertDialogDescription style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {uninstallTarget && t("skills.uninstallConfirm", "将删除 skill {{name}} 及其在所有 agent 的启用配置，不可恢复。确认？", { name: uninstallTarget.name })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel style={{ fontSize: 13 }}>
              {t("action.cancel", "取消")}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={handleUninstallSingle}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              style={{ fontSize: 13 }}
            >
              {t("skills.uninstall", "卸载")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 对齐配置 Dialog：使 to 与 from 启用配置一致 */}
      <Dialog open={alignOpen} onOpenChange={setAlignOpen}>
        <DialogContent className="glass-elevated" style={{ maxWidth: 400, padding: 24, gap: 16 }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 15, fontWeight: 700 }}>
              {t("skills.alignTitle", "对齐配置")}
            </DialogTitle>
          </DialogHeader>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
              <span style={{ minWidth: 72 }} className="text-secondary">{t("skills.alignFrom", "源 agent")}</span>
              <Select value={alignFrom} onValueChange={(v) => setAlignFrom(v as SkillAgent)}>
                <SelectTrigger style={{ flex: 1, fontSize: 13 }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {AGENTS.map((a) => (
                    <SelectItem key={a} value={a}>{t(`skills.agent.${a}`, a)}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
              <span style={{ minWidth: 72 }} className="text-secondary">{t("skills.alignTo", "目标 agent")}</span>
              <Select value={alignTo} onValueChange={(v) => setAlignTo(v as SkillAgent)}>
                <SelectTrigger style={{ flex: 1, fontSize: 13 }}>
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
          <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.6 }}>
            {alignFrom === alignTo
              ? t("skills.alignSameAgent", "源与目标不能相同")
              : t("skills.alignConfirm", "将使 {{to}} 的启用配置与 {{from}} 完全一致（启用 {{from}} 已启用的、关闭 {{from}} 未启用的）。", {
                  from: t(`skills.agent.${alignFrom}`, alignFrom),
                  to: t(`skills.agent.${alignTo}`, alignTo),
                })}
          </div>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <Button variant="outline" style={{ fontSize: 13 }} onClick={() => setAlignOpen(false)}>
              {t("action.cancel", "取消")}
            </Button>
            <Button
              style={{ fontSize: 13 }}
              disabled={alignFrom === alignTo}
              onClick={handleAlign}
            >
              {t("skills.alignTitle", "对齐配置")}
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
              {t("skills.pasteTitle", "从分享导入")}
            </DialogTitle>
          </DialogHeader>
          <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.5 }}>
            {t("skills.pasteHint", "粘贴他人分享的 skill 文本（base64 / JSON / aidog:// 链接的 data 部分）。")}
          </div>
          <Textarea
            style={{ minHeight: 160, fontFamily: "var(--font-mono, monospace)", resize: "vertical" }}
            value={pasteText}
            placeholder="aidog://skill/import?data=... 或 base64 或 JSON 数组"
            onChange={(e) => setPasteText(e.target.value)}
          />
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
            <Button variant="outline" style={{ fontSize: 13 }} onClick={() => setPasteOpen(false)}>
              {t("action.cancel", "取消")}
            </Button>
            <Button
              style={{ fontSize: 13 }}
              onClick={handlePasteImport}
              disabled={!pasteText.trim()}
            >
              {t("skills.importBtn", "导入")}
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
              {t("skills.importConfirmTitle", "确认导入 skills")}
            </DialogTitle>
          </DialogHeader>
          <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.5 }}>
            {importIds !== null && t("skills.importConfirmDesc", "将安装以下 {{count}} 个 skill（npx skills add，需联网）：", { count: importIds.length })}
          </div>
          {importIds !== null && (
            <div style={{ display: "flex", flexDirection: "column", gap: 4, maxHeight: "30vh", overflow: "auto", padding: 8, border: "1px solid var(--border)", borderRadius: 8, background: "var(--bg-glass)" }}>
              {importIds.map((id) => (
                <div key={id} style={{ fontSize: 12, fontFamily: "var(--font-mono, monospace)" }}>{id}</div>
              ))}
            </div>
          )}
          {/* scope 选择 */}
          <div style={{ display: "flex", gap: 8, alignItems: "center", fontSize: 12 }}>
            <span className="text-secondary" style={{ minWidth: 56 }}>{t("skills.scope", "范围")}</span>
            <Select
              value={importScopeKind}
              onValueChange={(v) => setImportScopeKind(v as "global" | "project")}
              disabled={importBusy}
            >
              <SelectTrigger style={{ width: "auto", fontSize: 12 }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="global">{t("skills.scopeGlobal", "用户级（全局）")}</SelectItem>
                <SelectItem value="project">{t("skills.scopeProject", "项目级")}</SelectItem>
              </SelectContent>
            </Select>
            {importScopeKind === "project" && (
              <Input
                style={{ flex: 1 }}
                placeholder={t("skills.projectPathPlaceholder", "项目目录绝对路径")}
                value={importProjectPath}
                onChange={(e) => setImportProjectPath(e.target.value)}
                disabled={importBusy}
              />
            )}
          </div>
          {/* agent 多选 */}
          <div style={{ display: "flex", gap: 10, alignItems: "center", fontSize: 12, flexWrap: "wrap" }}>
            <span className="text-secondary" style={{ minWidth: 56 }}>{t("skills.importAgents", "目标 agent")}</span>
            {AGENTS.map((a) => {
              const on = importAgents.has(a);
              return (
                <Button
                  key={a}
                  type="button"
                  variant={on ? "default" : "outline"}
                  onClick={() =>
                    setImportAgents((prev) => {
                      const next = new Set(prev);
                      if (next.has(a)) next.delete(a);
                      else next.add(a);
                      return next;
                    })
                  }
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    padding: "4px 10px",
                    height: "auto",
                    opacity: on ? 1 : 0.5,
                    fontSize: 12,
                  }}
                >
                  <img src={AGENT_ICONS[a]} alt={a} style={{ width: 16, height: 16 }} />
                  {t(`skills.agent.${a}`, a)}
                </Button>
              );
            })}
          </div>
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
            <Button variant="outline" style={{ fontSize: 13 }} onClick={() => setImportIds(null)} disabled={importBusy}>
              {t("action.cancel", "取消")}
            </Button>
            <Button
              style={{ fontSize: 13 }}
              onClick={() => void handleImport()}
              disabled={importBusy || importAgents.size === 0 || (importScopeKind === "project" && importProjectPath.trim() === "") || !env?.npx_available}
            >
              {importBusy ? t("skills.installing", "导入中…") : t("skills.importBtn", "导入")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
