import { createPortal } from "react-dom";
import { type McpTransport } from "../../services/api";
import { ShareModal } from "../../components/platforms/ShareModal";
import type { McpData } from "./useMcpData";
import { summaryOf } from "./constants";
import { TransportBadge, KVRows } from "./primitives";
import {
  btnPrimary,
  btnGhost,
  btnDanger,
  modalOverlay,
  modalBody,
  fieldLabel,
  inputStyle,
} from "./styles";

/**
 * 全部 modal（自原 Mcp.tsx L538-819 外迁，全部 createPortal 弹窗聚簇）：
 * scanImport / pasteImport / delete / edit / share。
 */
export function McpModals({ d }: { d: McpData }) {
  const {
    t,
    scanOpen, setScanOpen, scanItems, scanning, selected, setSelected, importing,
    toggleSelect, handleImport,
    pasteOpen, setPasteOpen, pasteText, setPasteText, pasteBusy, handlePasteImport,
    deleteTarget, setDeleteTarget, handleDelete,
    editTarget, editOpen, setEditTarget, setEditOpen, editForm, setEditForm, handleEditSave,
    shareData, setShareData, busyKey, setMessage,
  } = d;

  return (
    <>
      {/* 扫描导入 modal */}
      {scanOpen &&
        createPortal(
          <div style={modalOverlay} onClick={() => !scanning && !importing && setScanOpen(false)}>
            <div style={modalBody} onClick={(e) => e.stopPropagation()}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12 }}>
                <h2 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
                  {t("mcp.scanTitle", "扫描导入 MCP")}
                </h2>
                <span style={{ color: "var(--text-tertiary)", fontSize: 12 }}>
                  {scanning ? t("mcp.scanning", "扫描中…") : `${scanItems.length}`}
                </span>
                <div style={{ flex: 1 }} />
                <button
                  onClick={() => {
                    const all =
                      selected.size === scanItems.filter((i) => !i.alreadyImported).length;
                    setSelected(
                      all
                        ? new Set()
                        : new Set(
                            scanItems.filter((i) => !i.alreadyImported).map((i) => i.name),
                          ),
                    );
                  }}
                  disabled={scanning || importing}
                  style={btnGhost}
                >
                  {t("mcp.toggleAll", "全选/反选")}
                </button>
              </div>

              <div style={{ maxHeight: "50vh", overflow: "auto", display: "flex", flexDirection: "column", gap: 6 }}>
                {scanning ? (
                  <div style={{ padding: 24, textAlign: "center", color: "var(--text-tertiary)", fontSize: 13 }}>
                    {t("mcp.scanning", "扫描中…")}
                  </div>
                ) : scanItems.length === 0 ? (
                  <div style={{ padding: 24, textAlign: "center", color: "var(--text-tertiary)", fontSize: 13 }}>
                    {t("mcp.scanEmpty", "两 agent 配置无 MCP")}
                  </div>
                ) : (
                  scanItems.map((it) => {
                    const checked = selected.has(it.name);
                    const disabled = it.alreadyImported;
                    return (
                      <label
                        key={it.name}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 10,
                          padding: "8px 10px",
                          borderRadius: 8,
                          border: `1px solid var(--border)`,
                          background: disabled ? "var(--bg-elevated)" : "transparent",
                          opacity: disabled ? 0.5 : 1,
                          cursor: disabled ? "not-allowed" : "pointer",
                          fontSize: 13,
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={checked || disabled}
                          disabled={disabled}
                          onChange={() => toggleSelect(it.name)}
                        />
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                            <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>{it.name}</span>
                            <TransportBadge transport={it.transport} />
                            {it.foundInAgents.map((a) => (
                              <span
                                key={a}
                                style={{
                                  fontSize: 10,
                                  padding: "1px 5px",
                                  borderRadius: 4,
                                  background: "var(--bg-elevated)",
                                  color: "var(--text-tertiary)",
                                }}
                              >
                                {t(`mcp.agent.${a}`, a)}
                              </span>
                            ))}
                            {disabled && (
                              <span style={{ fontSize: 10, color: "var(--success)" }}>
                                {t("mcp.alreadyImported", "已导入")}
                              </span>
                            )}
                          </div>
                          <div style={{ color: "var(--text-tertiary)", fontSize: 11, marginTop: 2, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                            {summaryOf(it)}
                          </div>
                        </div>
                      </label>
                    );
                  })
                )}
              </div>

              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 12 }}>
                <button onClick={() => setScanOpen(false)} disabled={importing} style={btnGhost}>
                  {t("common.cancel", "取消")}
                </button>
                <button
                  onClick={handleImport}
                  disabled={importing || selected.size === 0}
                  style={btnPrimary}
                >
                  {importing
                    ? t("mcp.importing", "导入中…")
                    : t("mcp.import", { count: selected.size, defaultValue: `导入 ${selected.size}` })}
                </button>
              </div>
            </div>
          </div>,
          document.body,
        )}

      {/* 粘贴 JSON 导入 modal */}
      {pasteOpen &&
        createPortal(
          <div style={modalOverlay} onClick={() => !pasteBusy && setPasteOpen(false)}>
            <div style={modalBody} onClick={(e) => e.stopPropagation()}>
              <h2 style={{ margin: "0 0 8px", fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
                {t("mcp.pasteTitle", "粘贴 JSON 导入")}
              </h2>
              <p style={{ margin: "0 0 12px", fontSize: 12, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
                {t("mcp.pasteHint", "粘贴 Claude 格式 MCP 配置（含 mcpServers 包裹或直接 名称→配置 映射）。同名跳过，导入后逐 agent 启用。")}
              </p>
              <textarea
                style={{ ...inputStyle, minHeight: 220, fontFamily: "var(--font-mono, monospace)", resize: "vertical" }}
                value={pasteText}
                placeholder={'{\n  "mcpServers": {\n    "filesystem": {\n      "command": "npx",\n      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path"]\n    }\n  }\n}'}
                onChange={(e) => setPasteText(e.target.value)}
              />
              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 12 }}>
                <button onClick={() => setPasteOpen(false)} disabled={pasteBusy} style={btnGhost}>
                  {t("common.cancel", "取消")}
                </button>
                <button
                  onClick={handlePasteImport}
                  disabled={pasteBusy || !pasteText.trim()}
                  style={btnPrimary}
                >
                  {pasteBusy ? t("mcp.importing", "导入中…") : t("mcp.import", "导入")}
                </button>
              </div>
            </div>
          </div>,
          document.body,
        )}

      {/* 删除确认 modal */}
      {deleteTarget &&
        createPortal(
          <div style={modalOverlay} onClick={() => setDeleteTarget(null)}>
            <div style={modalBody} onClick={(e) => e.stopPropagation()}>
              <h2 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
                {t("mcp.deleteTitle", "删除 MCP")}
              </h2>
              <p style={{ margin: "12px 0", fontSize: 13, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
                {t("mcp.deleteConfirm", {
                  name: deleteTarget.name,
                  defaultValue: `将从 DB + 所有 enabled agent 配置移除「${deleteTarget.name}」，不可恢复`,
                })}
              </p>
              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
                <button onClick={() => setDeleteTarget(null)} style={btnGhost}>
                  {t("common.cancel", "取消")}
                </button>
                <button onClick={handleDelete} style={btnDanger}>
                  {t("common.delete", "删除")}
                </button>
              </div>
            </div>
          </div>,
          document.body,
        )}

      {/* 编辑/添加 modal（editTarget=null = 添加模式） */}
      {editOpen &&
        createPortal(
          <div style={modalOverlay} onClick={() => busyKey === null && (setEditTarget(null), setEditOpen(false))}>
            <div style={modalBody} onClick={(e) => e.stopPropagation()}>
              <h2 style={{ margin: "0 0 12px", fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
                {editTarget === null
                  ? t("mcp.addTitle", "添加 MCP")
                  : t("mcp.editTitle", "编辑 MCP")}
              </h2>
              <div style={{ display: "flex", flexDirection: "column", gap: 10, overflow: "auto", paddingRight: 4 }}>
                <label style={fieldLabel}>
                  <span>{t("mcp.field.name", "名称")}</span>
                  <input
                    style={inputStyle}
                    value={editForm.name}
                    onChange={(e) => setEditForm((f) => ({ ...f, name: e.target.value }))}
                  />
                </label>
                <label style={fieldLabel}>
                  <span>{t("mcp.field.transport", "传输")}</span>
                  <select
                    style={inputStyle}
                    value={editForm.transport}
                    onChange={(e) => setEditForm((f) => ({ ...f, transport: e.target.value as McpTransport }))}
                  >
                    <option value="stdio">stdio</option>
                    <option value="http">http</option>
                    <option value="sse">sse</option>
                  </select>
                </label>
                {editForm.transport === "stdio" ? (
                  <>
                    <label style={fieldLabel}>
                      <span>{t("mcp.field.command", "命令")}</span>
                      <input
                        style={inputStyle}
                        value={editForm.command}
                        placeholder="npx"
                        onChange={(e) => setEditForm((f) => ({ ...f, command: e.target.value }))}
                      />
                    </label>
                    <label style={fieldLabel}>
                      <span>{t("mcp.field.args", "参数（每行一个）")}</span>
                      <textarea
                        style={{ ...inputStyle, minHeight: 64, fontFamily: "var(--font-mono)" }}
                        value={editForm.argsText}
                        onChange={(e) => setEditForm((f) => ({ ...f, argsText: e.target.value }))}
                      />
                    </label>
                    <KVRows
                      label={t("mcp.field.env", "环境变量")}
                      rows={editForm.envRows}
                      onChange={(envRows) => setEditForm((f) => ({ ...f, envRows }))}
                    />
                  </>
                ) : (
                  <>
                    <label style={fieldLabel}>
                      <span>{t("mcp.field.url", "URL")}</span>
                      <input
                        style={inputStyle}
                        value={editForm.url}
                        placeholder="https://..."
                        onChange={(e) => setEditForm((f) => ({ ...f, url: e.target.value }))}
                      />
                    </label>
                    <KVRows
                      label={t("mcp.field.headers", "请求头")}
                      rows={editForm.headersRows}
                      onChange={(headersRows) => setEditForm((f) => ({ ...f, headersRows }))}
                    />
                  </>
                )}
              </div>
              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 12 }}>
                <button onClick={() => { setEditTarget(null); setEditOpen(false); }} disabled={busyKey !== null} style={btnGhost}>
                  {t("common.cancel", "取消")}
                </button>
                <button onClick={handleEditSave} disabled={busyKey !== null} style={btnPrimary}>
                  {busyKey !== null ? t("mcp.saving", "保存中…") : t("mcp.save", "保存")}
                </button>
              </div>
            </div>
          </div>,
          document.body,
        )}

      {/* 分享 modal（泛化 ShareModal，3 格式切换 + 复制为 aidog://mcp/import 链接） */}
      {shareData && (
        <ShareModal
          share={shareData.share}
          title={shareData.name}
          titleKey="mcp.share.title"
          warningKey="mcp.share.warning"
          urlScheme="aidog://mcp/import"
          copyUrlKey="mcp.share.copyUrl"
          onToast={(text, ok) => setMessage({ kind: ok ? "ok" : "err", text })}
          onClose={() => setShareData(null)}
        />
      )}
    </>
  );
}
