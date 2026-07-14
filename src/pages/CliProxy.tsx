// ─── CliProxy 管理页 (cpa-standalone-module s5) ───────────────
// 独立于 platform 表的 CLI 代理上游 provider 管理。后端 command 见 `commands_cli_proxy` crate。
// - 列表：name / wire / base_url / status + 操作（测试余额 / 建 platform 行 / 编辑 / 删除）
// - 编辑/新增：inline form（toggle 显示）
// - 导入：modal（源路径 + OAuth 凭据目录 + 分组），调 cli_proxy_import 批量入库
// 删除/导入 modal 均 createPortal(document.body)，脱离 liquid glass 祖先 transform（见 memory
// modal-window-center-rule）。ponytail: 单文件页，无子组件拆分（YAGNI，s5 范围）。

import { useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import {
  cliProxyApi,
  groupApi,
  type CliProxyProvider,
  type CreateCliProxyProvider,
  type Group,
  type CliProxyImportResult,
} from "../services/api";

const btnPrimary: React.CSSProperties = {
  padding: "7px 14px", borderRadius: 8, border: "1px solid var(--accent)",
  background: "var(--accent)", color: "#fff", fontSize: 13, fontWeight: 600, cursor: "pointer",
};
const btnGhost: React.CSSProperties = {
  padding: "7px 14px", borderRadius: 8, border: "1px solid var(--border)",
  background: "transparent", color: "var(--text-primary)", fontSize: 13, cursor: "pointer",
};
const btnDanger: React.CSSProperties = {
  ...btnPrimary, border: "1px solid var(--danger)", background: "var(--danger)",
};
const inputStyle: React.CSSProperties = {
  padding: "6px 10px", borderRadius: 8, border: "1px solid var(--border)",
  background: "var(--bg)", color: "var(--text-primary)", fontSize: 13, outline: "none",
  width: "100%",
};
const fieldLabel: React.CSSProperties = {
  display: "flex", flexDirection: "column", gap: 4,
  fontSize: 12, color: "var(--text-secondary)",
};
const modalOverlay: React.CSSProperties = {
  position: "fixed", inset: 0, background: "rgba(0,0,0,0.5)",
  display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000,
};
const modalBody: React.CSSProperties = {
  background: "var(--bg-floating)", border: "1px solid var(--border)", borderRadius: 12,
  padding: 20, width: "min(520px, 90vw)", maxHeight: "80vh",
  display: "flex", flexDirection: "column", gap: 12, boxShadow: "0 8px 32px rgba(0,0,0,0.3)",
};

const EMPTY_FORM: CreateCliProxyProvider = {
  name: "", wire_protocol: "anthropic", base_url: "", api_key: "",
  models: [], extra: "", status: "active", group_id: null,
};

type Msg = { kind: "ok" | "err"; text: string } | null;

export function CliProxy() {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<CliProxyProvider[]>([]);
  const [groups, setGroups] = useState<Group[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [msg, setMsg] = useState<Msg>(null);

  // 编辑/新增表单：editingId=null=新增，number=编辑；formOpen 控制 inline form 显隐。
  const [editingId, setEditingId] = useState<number | "new" | null>(null);
  const [form, setForm] = useState<CreateCliProxyProvider>(EMPTY_FORM);
  // models 文本域：一行一 model（UI 友好；保存时 split）。
  const [modelsText, setModelsText] = useState("");

  // 导入 modal。
  const [importOpen, setImportOpen] = useState(false);
  const [importSource, setImportSource] = useState("");
  const [importAuthDir, setImportAuthDir] = useState("");
  const [importGroupId, setImportGroupId] = useState<number | "">("");

  // 删除确认。
  const [deleteTarget, setDeleteTarget] = useState<CliProxyProvider | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      const [list, gs] = await Promise.all([cliProxyApi.list(), groupApi.list()]);
      setProviders(list);
      setGroups(gs);
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void reload(); }, [reload]);

  const openNew = () => {
    setEditingId("new");
    setForm(EMPTY_FORM);
    setModelsText("");
    setMsg(null);
  };
  const openEdit = (p: CliProxyProvider) => {
    setEditingId(p.id);
    setForm({
      name: p.name, wire_protocol: p.wire_protocol, base_url: p.base_url,
      api_key: p.api_key, models: p.models, extra: p.extra,
      status: p.status, group_id: p.group_id ?? null,
    });
    setModelsText(p.models.join("\n"));
    setMsg(null);
  };
  const cancelForm = () => { setEditingId(null); setForm(EMPTY_FORM); setModelsText(""); };

  const handleSave = async () => {
    if (!form.name.trim()) { setMsg({ kind: "err", text: t("cliProxy.nameRequired") }); return; }
    if (!form.base_url.trim()) { setMsg({ kind: "err", text: t("cliProxy.baseUrlRequired") }); return; }
    const input: CreateCliProxyProvider = {
      ...form,
      models: modelsText.split("\n").map(s => s.trim()).filter(Boolean),
    };
    setBusyKey("save");
    try {
      if (editingId === "new") {
        await cliProxyApi.create(input);
      } else if (typeof editingId === "number") {
        await cliProxyApi.update(editingId, input);
      }
      setMsg({ kind: "ok", text: t("cliProxy.saved") });
      cancelForm();
      await reload();
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    setBusyKey(`del-${deleteTarget.id}`);
    try {
      await cliProxyApi.delete(deleteTarget.id);
      setMsg({ kind: "ok", text: t("cliProxy.deleted") });
      setDeleteTarget(null);
      await reload();
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  const handleTest = async (p: CliProxyProvider) => {
    setBusyKey(`test-${p.id}`);
    setMsg(null);
    try {
      const q = await cliProxyApi.test(p.id);
      if (q.success) {
        const bal = q.balance;
        setMsg({
          kind: "ok",
          text: bal
            ? `${t("cliProxy.testOk")}: ${bal.remaining} / ${bal.total ?? "?"} ${bal.currency}`.trim()
            : t("cliProxy.testOk"),
        });
      } else {
        setMsg({ kind: "err", text: `${t("cliProxy.testFail")}: ${q.error ?? ""}`.trim() });
      }
    } catch (e) {
      setMsg({ kind: "err", text: `${t("cliProxy.testFail")}: ${e}` });
    } finally {
      setBusyKey(null);
    }
  };

  const handleCreatePlatform = async (p: CliProxyProvider) => {
    setBusyKey(`plat-${p.id}`);
    setMsg(null);
    try {
      await cliProxyApi.createPlatform(p.id);
      setMsg({ kind: "ok", text: t("cliProxy.platformCreated") });
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  const pickFile = async (setter: (v: string) => void) => {
    const picked = await open({ multiple: false });
    if (picked && typeof picked === "string") setter(picked);
  };
  const pickDir = async (setter: (v: string) => void) => {
    const picked = await open({ directory: true, multiple: false });
    if (picked && typeof picked === "string") setter(picked);
  };

  const handleImport = async () => {
    if (!importSource.trim()) { setMsg({ kind: "err", text: t("cliProxy.importSource") + " required" }); return; }
    setBusyKey("import");
    setMsg(null);
    try {
      const r: CliProxyImportResult = await cliProxyApi.import(
        importSource,
        importAuthDir || undefined,
        importGroupId === "" ? null : importGroupId,
      );
      setMsg({
        kind: r.failed.length > 0 ? "err" : "ok",
        text: t("cliProxy.imported", {
          created: r.created.length, failed: r.failed.length, skipped: r.skipped.length,
        }),
      });
      setImportOpen(false);
      setImportSource(""); setImportAuthDir(""); setImportGroupId("");
      await reload();
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* 顶栏 */}
      <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <h1 style={{ fontSize: 22, fontWeight: 700, margin: 0, color: "var(--text-primary)" }}>
          {t("cliProxy.title")}
        </h1>
        <span style={{ color: "var(--text-tertiary)", fontSize: 13 }}>
          {t("cliProxy.subtitle", { count: providers.length })}
        </span>
        <div style={{ flex: 1 }} />
        <button onClick={openNew} disabled={busyKey !== null} style={btnGhost}>
          {t("cliProxy.add")}
        </button>
        <button
          onClick={() => { setImportOpen(true); setMsg(null); }}
          disabled={busyKey !== null}
          style={btnPrimary}
        >
          {t("cliProxy.import")}
        </button>
      </div>

      {/* 消息条 */}
      {msg && (
        <div style={{
          padding: "8px 12px", borderRadius: 8,
          border: `1px solid ${msg.kind === "ok" ? "var(--success)" : "var(--danger)"}`,
          background: "var(--bg-elevated)",
          color: msg.kind === "ok" ? "var(--success)" : "var(--danger)",
          fontSize: 13,
        }}>
          {msg.text}
        </div>
      )}

      {/* 编辑/新增 inline form */}
      {editingId !== null && (
        <div style={{
          padding: 16, border: "1px solid var(--border)", borderRadius: 12,
          background: "var(--bg-elevated)",
          display: "flex", flexDirection: "column", gap: 12,
        }}>
          <div style={{ fontSize: 15, fontWeight: 600, color: "var(--text-primary)" }}>
            {editingId === "new" ? t("cliProxy.add") : t("cliProxy.edit")}
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
            <label style={fieldLabel}>
              {t("cliProxy.name")}
              <input
                style={inputStyle}
                value={form.name}
                onChange={e => setForm({ ...form, name: e.target.value })}
                placeholder="claude-official"
              />
            </label>
            <label style={fieldLabel}>
              {t("cliProxy.wireProtocol")}
              <select
                style={inputStyle}
                value={form.wire_protocol}
                onChange={e => setForm({ ...form, wire_protocol: e.target.value })}
              >
                {["anthropic", "openai", "openai_responses", "openai_completions", "gemini", "glm_coding"].map(v => (
                  <option key={v} value={v}>{v}</option>
                ))}
              </select>
            </label>
            <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
              {t("cliProxy.baseUrl")}
              <input
                style={inputStyle}
                value={form.base_url}
                onChange={e => setForm({ ...form, base_url: e.target.value })}
                placeholder="https://api.anthropic.com/v1"
              />
            </label>
            <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
              {t("cliProxy.apiKey")}
              <input
                style={inputStyle}
                type="password"
                value={form.api_key ?? ""}
                onChange={e => setForm({ ...form, api_key: e.target.value })}
                placeholder="sk-..."
              />
            </label>
            <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
              {t("cliProxy.models")}
              <textarea
                style={{ ...inputStyle, minHeight: 80, resize: "vertical" }}
                value={modelsText}
                onChange={e => setModelsText(e.target.value)}
                placeholder={t("cliProxy.modelsHint")}
              />
            </label>
            <label style={fieldLabel}>
              {t("cliProxy.status")}
              <select
                style={inputStyle}
                value={form.status ?? "active"}
                onChange={e => setForm({ ...form, status: e.target.value })}
              >
                <option value="active">{t("cliProxy.statusActive")}</option>
                <option value="disabled">{t("cliProxy.statusDisabled")}</option>
              </select>
            </label>
            <label style={fieldLabel}>
              {t("cliProxy.groupId")}
              <select
                style={inputStyle}
                value={form.group_id ?? ""}
                onChange={e => setForm({ ...form, group_id: e.target.value === "" ? null : Number(e.target.value) })}
              >
                <option value="">—</option>
                {groups.map(g => (
                  <option key={g.id} value={g.id}>{g.name}</option>
                ))}
              </select>
            </label>
            <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
              {t("cliProxy.extra")}
              <input
                style={inputStyle}
                value={form.extra ?? ""}
                onChange={e => setForm({ ...form, extra: e.target.value })}
                placeholder="{}"
              />
            </label>
          </div>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button onClick={cancelForm} disabled={busyKey !== null} style={btnGhost}>
              {t("cliProxy.cancel")}
            </button>
            <button onClick={handleSave} disabled={busyKey !== null} style={btnPrimary}>
              {t("cliProxy.save")}
            </button>
          </div>
        </div>
      )}

      {/* 列表 */}
      {loading ? (
        <div style={{ color: "var(--text-tertiary)", fontSize: 14 }}>
          {t("common.loading")}
        </div>
      ) : providers.length === 0 ? (
        <div style={{
          padding: 32, textAlign: "center", color: "var(--text-tertiary)", fontSize: 14,
          border: "1px dashed var(--border)", borderRadius: 12,
        }}>
          {t("cliProxy.empty")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {providers.map(p => (
            <div
              key={p.id}
              style={{
                padding: "12px 14px", borderRadius: 10,
                border: "1px solid var(--border)", background: "var(--bg-elevated)",
                display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap",
              }}
            >
              <div style={{ minWidth: 160 }}>
                <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>{p.name}</div>
                <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{p.wire_protocol}</div>
              </div>
              <div style={{ flex: 1, minWidth: 200, fontSize: 12, color: "var(--text-secondary)", wordBreak: "break-all" }}>
                {p.base_url}
              </div>
              <span style={{
                padding: "2px 8px", borderRadius: 6, fontSize: 11,
                border: `1px solid ${p.status === "active" ? "var(--success)" : "var(--text-tertiary)"}`,
                color: p.status === "active" ? "var(--success)" : "var(--text-tertiary)",
              }}>
                {p.status === "active" ? t("cliProxy.statusActive") : t("cliProxy.statusDisabled")}
              </span>
              <div style={{ display: "flex", gap: 6 }}>
                <button
                  onClick={() => void handleTest(p)}
                  disabled={busyKey !== null}
                  style={btnGhost}
                  title={t("cliProxy.test")}
                >
                  {t("cliProxy.test")}
                </button>
                <button
                  onClick={() => void handleCreatePlatform(p)}
                  disabled={busyKey !== null}
                  style={btnGhost}
                  title={t("cliProxy.createPlatform")}
                >
                  {t("cliProxy.createPlatform")}
                </button>
                <button
                  onClick={() => openEdit(p)}
                  disabled={busyKey !== null}
                  style={btnGhost}
                >
                  {t("cliProxy.edit")}
                </button>
                <button
                  onClick={() => setDeleteTarget(p)}
                  disabled={busyKey !== null}
                  style={btnDanger}
                >
                  {t("cliProxy.delete")}
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* 删除确认 modal */}
      {deleteTarget && createPortal(
        <div style={modalOverlay} onClick={() => setDeleteTarget(null)}>
          <div style={modalBody} onClick={e => e.stopPropagation()}>
            <div style={{ fontSize: 15, fontWeight: 600, color: "var(--text-primary)" }}>
              {t("cliProxy.confirmDelete")}
            </div>
            <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>
              {deleteTarget.name} ({deleteTarget.wire_protocol})
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button onClick={() => setDeleteTarget(null)} disabled={busyKey !== null} style={btnGhost}>
                {t("cliProxy.cancel")}
              </button>
              <button onClick={handleDelete} disabled={busyKey !== null} style={btnDanger}>
                {t("cliProxy.delete")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}

      {/* 导入 modal */}
      {importOpen && createPortal(
        <div style={modalOverlay} onClick={() => setImportOpen(false)}>
          <div style={modalBody} onClick={e => e.stopPropagation()}>
            <div style={{ fontSize: 15, fontWeight: 600, color: "var(--text-primary)" }}>
              {t("cliProxy.import")}
            </div>
            <label style={fieldLabel}>
              {t("cliProxy.importSource")}
              <div style={{ display: "flex", gap: 8 }}>
                <input
                  style={inputStyle}
                  value={importSource}
                  onChange={e => setImportSource(e.target.value)}
                  placeholder="config.yaml / .zip / .tgz / dir"
                />
                <button onClick={() => void pickFile(setImportSource)} style={btnGhost}>
                  {t("cliProxy.importPickFile")}
                </button>
              </div>
            </label>
            <label style={fieldLabel}>
              {t("cliProxy.importAuthDir")}
              <div style={{ display: "flex", gap: 8 }}>
                <input
                  style={inputStyle}
                  value={importAuthDir}
                  onChange={e => setImportAuthDir(e.target.value)}
                  placeholder="~/.claude/auth.json dir (optional)"
                />
                <button onClick={() => void pickDir(setImportAuthDir)} style={btnGhost}>
                  {t("cliProxy.importPickDir")}
                </button>
              </div>
            </label>
            <label style={fieldLabel}>
              {t("cliProxy.groupId")}
              <select
                style={inputStyle}
                value={importGroupId}
                onChange={e => setImportGroupId(e.target.value === "" ? "" : Number(e.target.value))}
              >
                <option value="">—</option>
                {groups.map(g => (
                  <option key={g.id} value={g.id}>{g.name}</option>
                ))}
              </select>
            </label>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button onClick={() => setImportOpen(false)} disabled={busyKey !== null} style={btnGhost}>
                {t("cliProxy.cancel")}
              </button>
              <button onClick={handleImport} disabled={busyKey !== null} style={btnPrimary}>
                {t("cliProxy.import")}
              </button>
            </div>
          </div>
        </div>,
        document.body,
      )}
    </div>
  );
}
