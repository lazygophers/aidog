import { createPortal } from "react-dom";
import { type SkillAgent } from "../../services/api";
import { SkillDetailView } from "../SkillDetailView";
import { ShareModal } from "../../components/platforms/ShareModal";
import { AGENTS, AGENT_ICONS } from "./constants";
import type { SkillsData } from "./useSkillsData";

/**
 * 全部 modal（自原 Skills.tsx L973-1304 外迁，全部 createPortal 弹窗聚簇）：
 * confirmUninstall / uninstallSingle / align / detail / share / pasteImport / importConfirm。
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
      {/* 一键卸载二次确认 modal（破坏性，禁 native confirm） */}
      {/* createPortal 到 document.body：脱离 Skills 页 transform 祖先，fixed 始终相对 viewport 居中 */}
      {confirmUninstall && createPortal(
        <div
          onClick={() => setConfirmUninstall(false)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 200,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,0.4)",
            animation: "fadeIn 150ms ease both",
          }}
        >
          <div
            className="glass-elevated"
            onClick={(e) => e.stopPropagation()}
            style={{
              maxWidth: 380,
              padding: 24,
              display: "flex",
              flexDirection: "column",
              gap: 16,
            }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.uninstallAll", "卸载全部")}</div>
            <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t("skills.uninstallAllConfirm", "将删除当前范围下所有平台的全部 {{count}} 个 skills，不可恢复。确认？", { count: installed.length })}
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setConfirmUninstall(false)}>
                {t("action.cancel", "取消")}
              </button>
              <button className="btn btn-danger" style={{ fontSize: 13 }} onClick={handleUninstallAll}>
                {t("skills.uninstallAll", "卸载全部")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}

      {/* 单条卸载二次确认 modal（破坏性，禁 native confirm） */}
      {uninstallTarget && createPortal(
        <div
          onClick={() => setUninstallTarget(null)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 200,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,0.4)",
            animation: "fadeIn 150ms ease both",
          }}
        >
          <div
            className="glass-elevated"
            onClick={(e) => e.stopPropagation()}
            style={{
              maxWidth: 380,
              padding: 24,
              display: "flex",
              flexDirection: "column",
              gap: 16,
            }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.uninstall", "卸载")}</div>
            <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>
              {t("skills.uninstallConfirm", "将删除 skill {{name}} 及其在所有 agent 的启用配置，不可恢复。确认？", { name: uninstallTarget.name })}
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setUninstallTarget(null)}>
                {t("action.cancel", "取消")}
              </button>
              <button className="btn btn-danger" style={{ fontSize: 13 }} onClick={handleUninstallSingle}>
                {t("skills.uninstall", "卸载")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}

      {/* 对齐配置 modal：使 to 与 from 启用配置一致 */}
      {alignOpen && createPortal(
        <div
          onClick={() => setAlignOpen(false)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 200,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,0.4)",
            animation: "fadeIn 150ms ease both",
          }}
        >
          <div
            className="glass-elevated"
            onClick={(e) => e.stopPropagation()}
            style={{
              maxWidth: 400,
              padding: 24,
              display: "flex",
              flexDirection: "column",
              gap: 16,
            }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.alignTitle", "对齐配置")}</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
                <span style={{ minWidth: 72 }} className="text-secondary">{t("skills.alignFrom", "源 agent")}</span>
                <select className="input" style={{ flex: 1 }} value={alignFrom} onChange={(e) => setAlignFrom(e.target.value as SkillAgent)}>
                  {AGENTS.map((a) => (
                    <option key={a} value={a}>{t(`skills.agent.${a}`, a)}</option>
                  ))}
                </select>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
                <span style={{ minWidth: 72 }} className="text-secondary">{t("skills.alignTo", "目标 agent")}</span>
                <select className="input" style={{ flex: 1 }} value={alignTo} onChange={(e) => setAlignTo(e.target.value as SkillAgent)}>
                  {AGENTS.map((a) => (
                    <option key={a} value={a}>{t(`skills.agent.${a}`, a)}</option>
                  ))}
                </select>
              </label>
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
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setAlignOpen(false)}>
                {t("action.cancel", "取消")}
              </button>
              <button
                className="btn btn-primary"
                style={{ fontSize: 13 }}
                disabled={alignFrom === alignTo}
                onClick={handleAlign}
              >
                {t("skills.alignTitle", "对齐配置")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}

      {/* 已装 skill 详情 modal（只读） */}
      {detailTarget && createPortal(
        <SkillDetailView skill={detailTarget} onClose={() => setDetailTarget(null)} />,
        document.body,
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

      {/* 粘贴分享文本导入 modal */}
      {pasteOpen && createPortal(
        <div
          onClick={() => setPasteOpen(false)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 200,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,0.4)",
            animation: "fadeIn 150ms ease both",
          }}
        >
          <div
            className="glass-elevated"
            onClick={(e) => e.stopPropagation()}
            style={{ width: "min(560px, 90vw)", padding: 20, display: "flex", flexDirection: "column", gap: 10 }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.pasteTitle", "从分享导入")}</div>
            <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.5 }}>
              {t("skills.pasteHint", "粘贴他人分享的 skill 文本（base64 / JSON / aidog:// 链接的 data 部分）。")}
            </div>
            <textarea
              className="input"
              style={{ minHeight: 160, fontFamily: "var(--font-mono, monospace)", resize: "vertical" }}
              value={pasteText}
              placeholder="aidog://skill/import?data=... 或 base64 或 JSON 数组"
              onChange={(e) => setPasteText(e.target.value)}
            />
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setPasteOpen(false)}>
                {t("action.cancel", "取消")}
              </button>
              <button
                className="btn btn-primary"
                style={{ fontSize: 13 }}
                onClick={handlePasteImport}
                disabled={!pasteText.trim()}
              >
                {t("skills.importBtn", "导入")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}

      {/* 批量导入确认 modal（列出将装 skill id + scope/agents 选择） */}
      {importIds && createPortal(
        <div
          onClick={() => !importBusy && setImportIds(null)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 220,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,0.45)",
            animation: "fadeIn 150ms ease both",
            padding: 24,
          }}
        >
          <div
            className="glass-elevated"
            onClick={(e) => e.stopPropagation()}
            style={{ width: "min(560px, 92vw)", maxHeight: "82vh", padding: 22, display: "flex", flexDirection: "column", gap: 12 }}
          >
            <div style={{ fontSize: 15, fontWeight: 700 }}>{t("skills.importConfirmTitle", "确认导入 skills")}</div>
            <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.5 }}>
              {t("skills.importConfirmDesc", "将安装以下 {{count}} 个 skill（npx skills add，需联网）：", { count: importIds.length })}
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 4, maxHeight: "30vh", overflow: "auto", padding: 8, border: "1px solid var(--border)", borderRadius: 8, background: "var(--bg-glass)" }}>
              {importIds.map((id) => (
                <div key={id} style={{ fontSize: 12, fontFamily: "var(--font-mono, monospace)" }}>{id}</div>
              ))}
            </div>
            {/* scope 选择 */}
            <div style={{ display: "flex", gap: 8, alignItems: "center", fontSize: 12 }}>
              <span className="text-secondary" style={{ minWidth: 56 }}>{t("skills.scope", "范围")}</span>
              <select
                className="input"
                style={{ width: "auto" }}
                value={importScopeKind}
                onChange={(e) => setImportScopeKind(e.target.value as "global" | "project")}
                disabled={importBusy}
              >
                <option value="global">{t("skills.scopeGlobal", "用户级（全局）")}</option>
                <option value="project">{t("skills.scopeProject", "项目级")}</option>
              </select>
              {importScopeKind === "project" && (
                <input
                  className="input"
                  style={{ flex: 1 }}
                  placeholder={t("skills.projectPathPlaceholder", "项目目录绝对路径")}
                  value={importProjectPath}
                  onChange={(e) => setImportProjectPath(e.target.value)}
                  disabled={importBusy}
                />
              )}
            </div>
            {/* agent 多选 */}
            <div style={{ display: "flex", gap: 10, alignItems: "center", fontSize: 12 }}>
              <span className="text-secondary" style={{ minWidth: 56 }}>{t("skills.importAgents", "目标 agent")}</span>
              {AGENTS.map((a) => {
                const on = importAgents.has(a);
                return (
                  <button
                    key={a}
                    type="button"
                    className="glass"
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
                      borderRadius: 8,
                      border: on ? "1.5px solid var(--accent)" : "1px solid var(--border)",
                      background: on ? "var(--accent-subtle)" : "transparent",
                      opacity: on ? 1 : 0.5,
                      fontSize: 12,
                    }}
                  >
                    <img src={AGENT_ICONS[a]} alt={a} style={{ width: 16, height: 16 }} />
                    {t(`skills.agent.${a}`, a)}
                  </button>
                );
              })}
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
              <button className="btn btn-ghost" style={{ fontSize: 13 }} onClick={() => setImportIds(null)} disabled={importBusy}>
                {t("action.cancel", "取消")}
              </button>
              <button
                className="btn btn-primary"
                style={{ fontSize: 13 }}
                onClick={() => void handleImport()}
                disabled={importBusy || importAgents.size === 0 || (importScopeKind === "project" && importProjectPath.trim() === "") || !env?.npx_available}
              >
                {importBusy ? t("skills.installing", "导入中…") : t("skills.importBtn", "导入")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}
    </>
  );
}
