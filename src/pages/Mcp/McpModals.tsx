import { useState } from "react";
import { type McpTransport } from "../../services/api";
import { ShareModal } from "../../components/platforms/ShareModal";
import type { McpData } from "./useMcpData";
import { summaryOf, CATEGORY_ORDER, descFor, cmdSummary } from "./constants";
import { TransportBadge, KVRows } from "./primitives";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { JsonCodeEditor } from "../../components/shared";
import { Checkbox } from "@/components/ui/checkbox";
import { makeRipple } from "@/utils/motion";
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
import { useTranslation } from "react-i18next";

/**
 * 全部 modal（自原 Mcp.tsx L538-819 外迁）：
 * scanImport / pasteImport / delete / edit / share。
 *
 * 破坏性确认（delete）走 AlertDialog；编辑/扫描/粘贴走 Dialog。Radix Portal 脱离祖先 transform。
 */
export function McpModals({ d }: { d: McpData }) {
  const {
    t,
    scanOpen, setScanOpen, scanItems, scanning, selected, setSelected, importing,
    toggleSelect, handleImport,
    pasteOpen, setPasteOpen, pasteText, setPasteText, pasteBusy, handlePasteImport,
    deleteTarget, setDeleteTarget, handleDelete,
    editTarget, editOpen, setEditTarget, setEditOpen, editForm, setEditForm, handleEditSave,
    addTab, setAddTab, prefillMeta,
    shareData, setShareData, busyKey, setMessage,
  } = d;

  return (
    <>
      {/* 扫描导入 Dialog */}
      <Dialog
        open={scanOpen}
        onOpenChange={(next) => { if (!next && !scanning && !importing) setScanOpen(false); }}
      >
        <DialogContent className="glass-elevated" style={{ maxWidth: "min(560px, 90vw)", maxHeight: "80vh", gap: 12 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <DialogTitle style={{ margin: 0, fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("mcp.scanTitle", "扫描导入 MCP")}
            </DialogTitle>
            <span style={{ color: "var(--text-tertiary)", fontSize: 12 }}>
              {scanning ? t("mcp.scanning", "扫描中…") : `${scanItems.length}`}
            </span>
            <div style={{ flex: 1 }} />
            <Button
              variant="outline"
              className="ripple"
              onClick={(e) => {
                makeRipple(e);
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
            >
              {t("mcp.toggleAll", "全选/反选")}
            </Button>
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
                    <Checkbox
                      checked={checked || disabled}
                      disabled={disabled}
                      onCheckedChange={() => toggleSelect(it.name)}
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

          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
            <Button variant="outline" onClick={() => setScanOpen(false)} disabled={importing}>
              {t("common.cancel", "取消")}
            </Button>
            <Button
              className="ripple"
              onClick={(e) => { makeRipple(e); void handleImport(); }}
              disabled={importing || selected.size === 0}
            >
              {importing
                ? t("mcp.importing", "导入中…")
                : t("mcp.import", { count: selected.size, defaultValue: `导入 ${selected.size}` })}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* 粘贴 JSON 导入 Dialog */}
      <Dialog
        open={pasteOpen}
        onOpenChange={(next) => { if (!next && !pasteBusy) setPasteOpen(false); }}
      >
        <DialogContent className="glass-elevated" style={{ maxWidth: "min(560px, 90vw)", maxHeight: "80vh", gap: 12 }}>
          <DialogHeader>
            <DialogTitle style={{ margin: "0 0 8px", fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("mcp.pasteTitle", "粘贴 JSON 导入")}
            </DialogTitle>
          </DialogHeader>
          <p style={{ margin: "0 0 12px", fontSize: 12, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
            {t("mcp.pasteHint", "粘贴 Claude 格式 MCP 配置（含 mcpServers 包裹或直接 名称→配置 映射）。同名跳过，导入后逐 agent 启用。")}
          </p>
          <JsonCodeEditor
            value={pasteText}
            onChange={setPasteText}
            minHeight={220}
            maxHeight={360}
            placeholder={'{\n  "mcpServers": {\n    "filesystem": {\n      "command": "npx",\n      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path"]\n    }\n  }\n}'}
          />
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
            <Button variant="outline" onClick={() => setPasteOpen(false)} disabled={pasteBusy}>
              {t("common.cancel", "取消")}
            </Button>
            <Button
              className="ripple"
              onClick={(e) => { makeRipple(e); void handlePasteImport(); }}
              disabled={pasteBusy || !pasteText.trim()}
            >
              {pasteBusy ? t("mcp.importing", "导入中…") : t("mcp.import", "导入")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* 删除确认 AlertDialog（破坏性，禁 native confirm） */}
      <AlertDialog
        open={deleteTarget !== null}
        onOpenChange={(next) => { if (!next) setDeleteTarget(null); }}
      >
        <AlertDialogContent className="glass-elevated">
          <AlertDialogHeader>
            <AlertDialogTitle style={{ margin: 0, fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
              {t("mcp.deleteTitle", "删除 MCP")}
            </AlertDialogTitle>
            <AlertDialogDescription style={{ margin: "12px 0", fontSize: 13, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
              {deleteTarget && t("mcp.deleteConfirm", {
                name: deleteTarget.name,
                defaultValue: `将从 DB + 所有 enabled agent 配置移除「${deleteTarget.name}」，不可恢复`,
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>
              {t("common.cancel", "取消")}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => { makeRipple(e); void handleDelete(); }}
              className="ripple bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {t("common.delete", "删除")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 编辑/添加 Dialog（editTarget=null = 添加模式） */}
      <Dialog
        open={editOpen}
        onOpenChange={(next) => { if (!next && busyKey === null) { setEditTarget(null); setEditOpen(false); } }}
      >
        <DialogContent className="glass-elevated" style={{ maxWidth: "min(560px, 90vw)", maxHeight: "80vh", gap: 12 }}>
          <DialogHeader>
            <DialogTitle style={{ margin: "0 0 12px", fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>
              {editTarget === null
                ? t("mcp.addTitle", "添加 MCP")
                : t("mcp.editTitle", "编辑 MCP")}
            </DialogTitle>
          </DialogHeader>
          <div style={{ display: "flex", flexDirection: "column", gap: 10, overflow: "auto", paddingRight: 4 }}>
            {/* 添加模式双 tab（票 05 方案 A）：推荐（默认）/ 手动配置；编辑模式无 tab */}
            {editTarget === null && (
              <div style={{ display: "flex", gap: 2, borderBottom: "1px solid var(--border)" }} role="tablist">
                {(["rec", "manual"] as const).map((tab) => (
                  <button
                    key={tab}
                    role="tab"
                    aria-selected={addTab === tab}
                    onClick={() => setAddTab(tab)}
                    style={{
                      padding: "8px 14px",
                      fontSize: 13,
                      cursor: "pointer",
                      background: "transparent",
                      border: "none",
                      borderBottom: `2px solid ${addTab === tab ? "var(--accent)" : "transparent"}`,
                      color: addTab === tab ? "var(--text-primary)" : "var(--text-tertiary)",
                      fontWeight: addTab === tab ? 600 : 400,
                      marginBottom: -1,
                    }}
                  >
                    {tab === "rec"
                      ? t("mcp.recTab", "推荐")
                      : t("mcp.manualTab", "手动配置")}
                  </button>
                ))}
              </div>
            )}
            {editTarget !== null || addTab === "manual" ? (
              <>
            <div style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
              <span>{t("mcp.field.name", "名称")}</span>
              <Input
                value={editForm.name}
                onChange={(e) => setEditForm((f) => ({ ...f, name: e.target.value }))}
              />
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
              <span>{t("mcp.field.transport", "传输")}</span>
              <Select
                value={editForm.transport}
                onValueChange={(v) => setEditForm((f) => ({ ...f, transport: v as McpTransport }))}
              >
                <SelectTrigger style={{ fontSize: 13 }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="stdio">stdio</SelectItem>
                  <SelectItem value="http">http</SelectItem>
                  <SelectItem value="sse">sse</SelectItem>
                </SelectContent>
              </Select>
            </div>
            {editForm.transport === "stdio" ? (
              <>
                <div style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
                  <span>{t("mcp.field.command", "命令")}</span>
                  <Input
                    value={editForm.command}
                    placeholder="npx"
                    onChange={(e) => setEditForm((f) => ({ ...f, command: e.target.value }))}
                  />
                </div>
                <div style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
                  <span>{t("mcp.field.args", "参数（每行一个）")}</span>
                  <Textarea
                    style={{ minHeight: 64, fontFamily: "var(--font-mono)" }}
                    value={editForm.argsText}
                    onChange={(e) => setEditForm((f) => ({ ...f, argsText: e.target.value }))}
                  />
                </div>
                <KVRows
                  label={t("mcp.field.env", "环境变量")}
                  rows={editForm.envRows}
                  onChange={(envRows) => setEditForm((f) => ({ ...f, envRows }))}
                />
              </>
            ) : (
              <>
                <div style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12, color: "var(--text-secondary)" }}>
                  <span>{t("mcp.field.url", "URL")}</span>
                  <Input
                    value={editForm.url}
                    placeholder="https://..."
                    onChange={(e) => setEditForm((f) => ({ ...f, url: e.target.value }))}
                  />
                </div>
                <KVRows
                  label={t("mcp.field.headers", "请求头")}
                  rows={editForm.headersRows}
                  onChange={(headersRows) => setEditForm((f) => ({ ...f, headersRows }))}
                />
              </>
            )}
                {/* 推荐预填：必填 env 提示（票 09；键已预填、值留空） */}
                {prefillMeta && prefillMeta.requiredEnvKeys.length > 0 && (
                  <div style={{ fontSize: 11, color: "var(--warning)", lineHeight: 1.6 }}>
                    {t("mcp.requiredKeyHint", {
                      keys: prefillMeta.requiredEnvKeys.join(", "),
                      defaultValue: "{{keys}} 必填，申请地址：",
                    })}
                    {prefillMeta.docsUrl && (
                      <a
                        href={prefillMeta.docsUrl}
                        target="_blank"
                        rel="noreferrer"
                        style={{ color: "var(--warning)", wordBreak: "break-all" }}
                      >
                        {prefillMeta.docsUrl}
                      </a>
                    )}
                  </div>
                )}
                {/* 非 stdio 传输：Codex 同步边界提示（票 06） */}
                {editForm.transport !== "stdio" && (
                  <div style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
                    {t("mcp.codexStdioOnlyHint", "Codex 仅支持 stdio，此项不会同步到 Codex")}
                  </div>
                )}
              </>
            ) : (
              <RecommendedPane d={d} />
            )}
          </div>
          {(editTarget !== null || addTab === "manual") && (
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
            <Button variant="outline" onClick={() => { setEditTarget(null); setEditOpen(false); }} disabled={busyKey !== null}>
              {t("common.cancel", "取消")}
            </Button>
            <Button
              className="ripple"
              onClick={(e) => { makeRipple(e); void handleEditSave(); }}
              disabled={busyKey !== null}
            >
              {busyKey !== null ? t("mcp.saving", "保存中…") : t("mcp.save", "保存")}
            </Button>
          </div>
          )}
        </DialogContent>
      </Dialog>

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

// ─── 推荐 tab（票 09）───

/** simpleicons 图标（`https://cdn.simpleicons.org/<slug>`，品牌色）；slug 空串/加载失败 → 首字母。 */
function RecIcon({ slug, name }: { slug: string; name: string }) {
  const [failed, setFailed] = useState(false);
  if (!slug || failed) {
    return (
      <span style={{ fontSize: 14, fontWeight: 700, color: "var(--text-secondary)" }}>
        {name.charAt(0).toUpperCase()}
      </span>
    );
  }
  return (
    <img
      src={`https://cdn.simpleicons.org/${slug}`}
      alt={name}
      onError={() => setFailed(true)}
      style={{ width: 18, height: 18, objectFit: "contain" }}
    />
  );
}

/** 推荐 tab 主体：按 category 分组卡片；已装（对齐 mcp_list name）置灰点不动（防同名覆盖）。 */
function RecommendedPane({ d }: { d: McpData }) {
  const { t, i18n } = useTranslation();
  const installed = new Set(d.servers.map((s) => s.name));
  const entries = d.recommended ?? [];
  const lang = i18n.resolvedLanguage ?? i18n.language ?? "en-US";

  if (d.recLoading) {
    return (
      <div style={{ padding: 24, textAlign: "center", color: "var(--text-tertiary)", fontSize: 13 }}>
        {t("common.loading", "加载中…")}
      </div>
    );
  }
  if (entries.length === 0) {
    return (
      <div style={{ padding: 24, textAlign: "center", color: "var(--text-tertiary)", fontSize: 13 }}>
        {t("mcp.recEmpty", "暂无推荐")}
      </div>
    );
  }

  // 分组：已知 category 按固定序，未知按首现顺序排末尾
  const order: string[] = [...CATEGORY_ORDER];
  for (const e of entries) if (!order.includes(e.category)) order.push(e.category);

  return (
    <div style={{ maxHeight: "46vh", overflow: "auto" }}>
      {order.map((cat) => {
        const items = entries.filter((e) => e.category === cat);
        if (items.length === 0) return null;
        return (
          <div key={cat}>
            <div style={{ margin: "10px 0 6px", fontSize: 12, fontWeight: 600, color: "var(--text-tertiary)" }}>
              {t(`mcp.category.${cat}`, cat)}
            </div>
            {items.map((e) => {
              const inst = installed.has(e.name);
              return (
                <div
                  key={e.name}
                  data-recommended={e.name}
                  onClick={inst ? undefined : () => d.pickRecommended(e)}
                  style={{
                    display: "flex",
                    gap: 10,
                    alignItems: "flex-start",
                    padding: "10px 12px",
                    border: `1px solid var(--border)`,
                    borderRadius: 10,
                    marginBottom: 6,
                    cursor: inst ? "default" : "pointer",
                    opacity: inst ? 0.55 : 1,
                    background: inst ? "var(--bg-elevated)" : "transparent",
                  }}
                >
                  <div
                    style={{
                      width: 30,
                      height: 30,
                      borderRadius: 8,
                      flexShrink: 0,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      background: "var(--bg-elevated)",
                    }}
                  >
                    <RecIcon slug={e.icon} name={e.display_name || e.name} />
                  </div>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
                      {e.display_name || e.name}
                      {inst && (
                        <span style={{ fontSize: 10, fontWeight: 500, color: "var(--success)" }}>
                          {t("mcp.installed", "已安装")}
                        </span>
                      )}
                    </div>
                    <div style={{ fontSize: 12, color: "var(--text-tertiary)", marginTop: 2 }}>
                      {descFor(e, lang)}
                    </div>
                    <div
                      style={{
                        fontSize: 11,
                        color: "var(--text-tertiary)",
                        fontFamily: "var(--font-mono, monospace)",
                        marginTop: 2,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {cmdSummary(e)}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        );
      })}
    </div>
  );
}
