// ─── CliProxy 管理页 (cpa-standalone-module s5) ───────────────
// 独立于 platform 表的 CLI 代理上游 provider 管理。后端 command 见 `commands_cli_proxy` crate。
// - 列表：name / wire / base_url / status + 操作（测试余额 / 建 platform 行 / 编辑 / 删除）
// - 编辑/新增：inline form（toggle 显示）
// - 导入：modal（源路径 + OAuth 凭据目录 + 分组），调 cli_proxy_import 批量入库
// 删除/导入 modal 均用 shadcn Dialog/AlertDialog（Radix Portal，满足 createPortal(document.body) 居中规则，
// 见 memory modal-window-center-rule）。ponytail: 单文件页，无子组件拆分（YAGNI，s5 范围）。

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import {
  cliProxyApi,
  groupApi,
  type CliProxyProvider,
  type CreateCliProxyProvider,
  type Group,
  type CliProxyImportResult,
  type BatchReport,
} from "../services/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Checkbox } from "@/components/ui/checkbox";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog";
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

// radix Select 空值哨兵：`<SelectItem value="">` 会抛，用 __none__ 映射回 ""/null。
const NONE = "__none__";

const fieldLabel: React.CSSProperties = {
  display: "flex", flexDirection: "column", gap: 4,
  fontSize: 12, color: "var(--text-secondary)",
};

const EMPTY_FORM: CreateCliProxyProvider = {
  name: "", wire_protocol: "anthropic", base_url: "", api_key: "",
  models: [], extra: "", quota: "{}", status: "active", group_id: null,
};

/** 解析 quota JSON → type 值（none/newapi），异常/缺省回落 none。 */
function quotaTypeOf(q: string | undefined): string {
  try { return (JSON.parse(q || "{}").type) || "none"; } catch { return "none"; }
}

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

  // 批量操作：selectMode 切换 + 选中集合 + 3 modal 各自 payload。
  const [selectMode, setSelectMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [batchDeleteOpen, setBatchDeleteOpen] = useState(false);
  const [batchModelsOpen, setBatchModelsOpen] = useState(false);
  const [batchModelsText, setBatchModelsText] = useState("");
  const [batchQuotaOpen, setBatchQuotaOpen] = useState(false);
  const [batchQuotaType, setBatchQuotaType] = useState<"none" | "newapi">("none");

  const enterSelect = () => {
    setSelectMode(true);
    setSelectedIds(new Set());
    setMsg(null);
  };
  const exitSelect = () => {
    setSelectMode(false);
    setSelectedIds(new Set());
    setBatchDeleteOpen(false);
    setBatchModelsOpen(false);
    setBatchQuotaOpen(false);
  };
  const toggleSelect = (id: number) => {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };
  const toggleSelectAll = () => {
    setSelectedIds(prev => {
      if (prev.size === providers.length) return new Set();
      return new Set(providers.map(p => p.id));
    });
  };

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
      quota: p.quota ?? "{}",
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

  // ─── 批量提交 ───────────────────────────────────────────
  // ponytail: 3 handler 共用 reporter，骨架同（setBusyKey → invoke → toast → reload → close），
  // 不抽公共 fn：每 handler 的 close + payload reset 各异，抽了反而加 indirection。
  const reportToast = (r: BatchReport, okKey: string) => {
    if (r.skipped.length > 0) {
      setMsg({
        kind: "err",
        text: `${t(okKey, { count: r.applied })} (skipped ${r.skipped.length})`,
      });
    } else {
      setMsg({ kind: "ok", text: t(okKey, { count: r.applied }) });
    }
  };

  const handleBatchDelete = async () => {
    const ids = [...selectedIds];
    if (ids.length === 0) return;
    setBusyKey("batch-del");
    setMsg(null);
    try {
      const r = await cliProxyApi.batchDelete(ids);
      reportToast(r, "cliProxy.batchDeleted");
      setBatchDeleteOpen(false);
      exitSelect();
      await reload();
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  const handleBatchOverrideModels = async () => {
    const ids = [...selectedIds];
    if (ids.length === 0) return;
    const models = batchModelsText.split("\n").map(s => s.trim()).filter(Boolean);
    setBusyKey("batch-models");
    setMsg(null);
    try {
      const r = await cliProxyApi.batchOverrideModels(ids, models);
      reportToast(r, "cliProxy.batchModelsUpdated");
      setBatchModelsOpen(false);
      setBatchModelsText("");
      exitSelect();
      await reload();
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  const handleBatchSetQuota = async () => {
    const ids = [...selectedIds];
    if (ids.length === 0) return;
    const quota = JSON.stringify({ type: batchQuotaType });
    setBusyKey("batch-quota");
    setMsg(null);
    try {
      const r = await cliProxyApi.batchSetQuota(ids, quota);
      reportToast(r, "cliProxy.batchQuotaUpdated");
      setBatchQuotaOpen(false);
      exitSelect();
      await reload();
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
        <Button variant="ghost" onClick={openNew} disabled={busyKey !== null}>
          {t("cliProxy.add")}
        </Button>
        <Button
          variant="default"
          onClick={() => { setImportOpen(true); setMsg(null); }}
          disabled={busyKey !== null}
        >
          {t("cliProxy.import")}
        </Button>
        <Button
          variant={selectMode ? "destructive" : "ghost"}
          onClick={selectMode ? exitSelect : enterSelect}
          disabled={busyKey !== null}
        >
          {selectMode ? t("cliProxy.exitSelect") : t("cliProxy.selectMode")}
        </Button>
      </div>

      {/* 选择模式工具栏 */}
      {selectMode && (
        <div style={{
          display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap",
          padding: "10px 14px", border: "1px solid var(--border)", borderRadius: 10,
          background: "var(--bg-elevated)",
        }}>
          <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 13, cursor: "pointer", color: "var(--text-primary)" }}>
            <Checkbox
              checked={providers.length > 0 && selectedIds.size === providers.length}
              onCheckedChange={toggleSelectAll}
            />
            {t("cliProxy.selectAll")}
          </label>
          <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
            {t("cliProxy.selectedCount", { count: selectedIds.size })}
          </span>
          <div style={{ flex: 1 }} />
          <Button
            variant="destructive"
            onClick={() => setBatchDeleteOpen(true)}
            disabled={selectedIds.size === 0 || busyKey !== null}
            style={{ opacity: selectedIds.size === 0 ? 0.4 : 1 }}
          >
            {t("cliProxy.batchDelete")}
          </Button>
          <Button
            variant="ghost"
            onClick={() => { setBatchModelsText(""); setBatchModelsOpen(true); }}
            disabled={selectedIds.size === 0 || busyKey !== null}
            style={{ opacity: selectedIds.size === 0 ? 0.4 : 1 }}
          >
            {t("cliProxy.batchOverrideModels")}
          </Button>
          <Button
            variant="ghost"
            onClick={() => { setBatchQuotaType("none"); setBatchQuotaOpen(true); }}
            disabled={selectedIds.size === 0 || busyKey !== null}
            style={{ opacity: selectedIds.size === 0 ? 0.4 : 1 }}
          >
            {t("cliProxy.batchSetQuota")}
          </Button>
        </div>
      )}

      {/* 消息条 */}
      {msg && (
        <div style={{
          padding: "8px 12px", borderRadius: 8,
          border: `1px solid ${msg.kind === "ok" ? "var(--color-success)" : "var(--color-danger)"}`,
          background: "var(--bg-elevated)",
          color: msg.kind === "ok" ? "var(--color-success)" : "var(--color-danger)",
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
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: 12 }}>
            <label style={fieldLabel}>
              {t("cliProxy.name")}
              <Input
                value={form.name}
                onChange={e => setForm({ ...form, name: e.target.value })}
                placeholder="claude-official"
              />
            </label>
            <label style={fieldLabel}>
              {t("cliProxy.wireProtocol")}
              <Select
                value={form.wire_protocol}
                onValueChange={v => setForm({ ...form, wire_protocol: v })}
              >
                <SelectTrigger style={{ width: "100%" }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {["anthropic", "openai", "openai_responses", "openai_completions", "gemini", "glm_coding"].map(v => (
                    <SelectItem key={v} value={v}>{v}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
            <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
              {t("cliProxy.baseUrl")}
              <Input
                value={form.base_url}
                onChange={e => setForm({ ...form, base_url: e.target.value })}
                placeholder="https://api.anthropic.com/v1"
              />
            </label>
            <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
              {t("cliProxy.apiKey")}
              <Input
                type="password"
                value={form.api_key ?? ""}
                onChange={e => setForm({ ...form, api_key: e.target.value })}
                placeholder="sk-..."
              />
            </label>
            <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
              {t("cliProxy.models")}
              <Textarea
                style={{ minHeight: 80, resize: "vertical" }}
                value={modelsText}
                onChange={e => setModelsText(e.target.value)}
                placeholder={t("cliProxy.modelsHint")}
              />
            </label>
            <label style={fieldLabel}>
              {t("cliProxy.status")}
              <Select
                value={form.status ?? "active"}
                onValueChange={v => setForm({ ...form, status: v })}
              >
                <SelectTrigger style={{ width: "100%" }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="active">{t("cliProxy.statusActive")}</SelectItem>
                  <SelectItem value="disabled">{t("cliProxy.statusDisabled")}</SelectItem>
                </SelectContent>
              </Select>
            </label>
            <label style={fieldLabel}>
              {t("cliProxy.groupId")}
              <Select
                value={form.group_id == null ? NONE : String(form.group_id)}
                onValueChange={v => setForm({ ...form, group_id: v === NONE ? null : Number(v) })}
              >
                <SelectTrigger style={{ width: "100%" }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={NONE}>—</SelectItem>
                  {groups.map(g => (
                    <SelectItem key={g.id} value={String(g.id)}>{g.name}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>
            <label style={{ ...fieldLabel, gridColumn: "1 / -1" }}>
              {t("cliProxy.extra")}
              <Input
                value={form.extra ?? ""}
                onChange={e => setForm({ ...form, extra: e.target.value })}
                placeholder="{}"
              />
            </label>
            <label style={fieldLabel}>
              {t("cliProxy.quotaType")}
              <Select
                value={quotaTypeOf(form.quota)}
                onValueChange={v => setForm({ ...form, quota: JSON.stringify({ type: v }) })}
              >
                <SelectTrigger style={{ width: "100%" }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">{t("cliProxy.quotaTypeNone")}</SelectItem>
                  <SelectItem value="newapi">{t("cliProxy.quotaTypeNewapi")}</SelectItem>
                </SelectContent>
              </Select>
            </label>
          </div>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <Button variant="ghost" onClick={cancelForm} disabled={busyKey !== null}>
              {t("cliProxy.cancel")}
            </Button>
            <Button variant="default" onClick={handleSave} disabled={busyKey !== null}>
              {t("cliProxy.save")}
            </Button>
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
              {selectMode && (
                <Checkbox
                  checked={selectedIds.has(p.id)}
                  onCheckedChange={() => toggleSelect(p.id)}
                  style={{ flexShrink: 0 }}
                />
              )}
              <div style={{ minWidth: 160 }}>
                <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>{p.name}</div>
                <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{p.wire_protocol}</div>
              </div>
              <div style={{ flex: 1, minWidth: 200, fontSize: 12, color: "var(--text-secondary)", wordBreak: "break-all" }}>
                {p.base_url}
              </div>
              <span style={{
                padding: "2px 8px", borderRadius: 6, fontSize: 11,
                border: `1px solid ${p.status === "active" ? "var(--color-success)" : "var(--text-tertiary)"}`,
                color: p.status === "active" ? "var(--color-success)" : "var(--text-tertiary)",
              }}>
                {p.status === "active" ? t("cliProxy.statusActive") : t("cliProxy.statusDisabled")}
              </span>
              {quotaTypeOf(p.quota) === "newapi" && (
                <span style={{
                  padding: "2px 8px", borderRadius: 6, fontSize: 11,
                  border: "1px solid var(--accent)", color: "var(--accent)",
                }}>
                  {t("cliProxy.quotaTypeNewapi")}
                </span>
              )}
              <div style={{ display: "flex", gap: 6 }}>
                <Button
                  variant="ghost"
                  onClick={() => void handleTest(p)}
                  disabled={busyKey !== null}
                  title={t("cliProxy.test")}
                  style={{ height: "auto", padding: "4px 10px", fontSize: 12 }}
                >
                  {t("cliProxy.test")}
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => void handleCreatePlatform(p)}
                  disabled={busyKey !== null}
                  title={t("cliProxy.createPlatform")}
                  style={{ height: "auto", padding: "4px 10px", fontSize: 12 }}
                >
                  {t("cliProxy.createPlatform")}
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => openEdit(p)}
                  disabled={busyKey !== null}
                  style={{ height: "auto", padding: "4px 10px", fontSize: 12 }}
                >
                  {t("cliProxy.edit")}
                </Button>
                <Button
                  variant="destructive"
                  onClick={() => setDeleteTarget(p)}
                  disabled={busyKey !== null}
                  style={{ height: "auto", padding: "4px 10px", fontSize: 12 }}
                >
                  {t("cliProxy.delete")}
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* 删除确认 AlertDialog */}
      <AlertDialog open={deleteTarget !== null} onOpenChange={o => { if (!o) setDeleteTarget(null); }}>
        <AlertDialogContent className="glass-elevated" style={{ maxWidth: 420, padding: 20 }}>
          <AlertDialogHeader>
            <AlertDialogTitle style={{ fontSize: 15, fontWeight: 600 }}>
              {t("cliProxy.confirmDelete")}
            </AlertDialogTitle>
            <AlertDialogDescription style={{ fontSize: 13, color: "var(--text-secondary)" }}>
              {deleteTarget?.name} ({deleteTarget?.wire_protocol})
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busyKey !== null}>{t("cliProxy.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              onClick={handleDelete}
              disabled={busyKey !== null}
              style={{ background: "var(--color-danger)" }}
            >
              {t("cliProxy.delete")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 批量删除确认 AlertDialog */}
      <AlertDialog open={batchDeleteOpen} onOpenChange={setBatchDeleteOpen}>
        <AlertDialogContent className="glass-elevated" style={{ maxWidth: 420, padding: 20 }}>
          <AlertDialogHeader>
            <AlertDialogTitle style={{ fontSize: 15, fontWeight: 600 }}>
              {t("cliProxy.batchDeleteTitle")}
            </AlertDialogTitle>
            <AlertDialogDescription style={{ fontSize: 13, color: "var(--text-secondary)" }}>
              {selectedIds.size <= 5
                ? providers
                    .filter(p => selectedIds.has(p.id))
                    .map(p => `${p.name} (${p.wire_protocol})`)
                    .join("、")
                : t("cliProxy.batchDeleteConfirm", { count: selectedIds.size })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
            {t("cliProxy.batchDeleteConfirm", { count: selectedIds.size })}
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busyKey !== null}>{t("cliProxy.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              onClick={handleBatchDelete}
              disabled={busyKey !== null}
              style={{ background: "var(--color-danger)" }}
            >
              {t("cliProxy.batchDelete")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 批量覆盖 models Dialog */}
      <Dialog open={batchModelsOpen} onOpenChange={setBatchModelsOpen}>
        <DialogContent className="glass-elevated" style={{ maxWidth: 520, padding: 20 }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 15, fontWeight: 600 }}>
              {t("cliProxy.batchModelsTitle")}
            </DialogTitle>
            <DialogDescription style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
              {t("cliProxy.selectedCount", { count: selectedIds.size })}
            </DialogDescription>
          </DialogHeader>
          <label style={fieldLabel}>
            <Textarea
              style={{ minHeight: 120, resize: "vertical" }}
              value={batchModelsText}
              onChange={e => setBatchModelsText(e.target.value)}
              placeholder={t("cliProxy.batchModelsPlaceholder")}
            />
          </label>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setBatchModelsOpen(false)} disabled={busyKey !== null}>
              {t("cliProxy.cancel")}
            </Button>
            <Button variant="default" onClick={handleBatchOverrideModels} disabled={busyKey !== null}>
              {t("cliProxy.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 批量设置 quota Dialog */}
      <Dialog open={batchQuotaOpen} onOpenChange={setBatchQuotaOpen}>
        <DialogContent className="glass-elevated" style={{ maxWidth: 420, padding: 20 }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 15, fontWeight: 600 }}>
              {t("cliProxy.batchQuotaTitle")}
            </DialogTitle>
            <DialogDescription style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
              {t("cliProxy.selectedCount", { count: selectedIds.size })}
            </DialogDescription>
          </DialogHeader>
          <label style={fieldLabel}>
            {t("cliProxy.quotaType")}
            <Select
              value={batchQuotaType}
              onValueChange={v => setBatchQuotaType(v as "none" | "newapi")}
            >
              <SelectTrigger style={{ width: "100%" }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="none">{t("cliProxy.quotaTypeNone")}</SelectItem>
                <SelectItem value="newapi">{t("cliProxy.quotaTypeNewapi")}</SelectItem>
              </SelectContent>
            </Select>
          </label>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setBatchQuotaOpen(false)} disabled={busyKey !== null}>
              {t("cliProxy.cancel")}
            </Button>
            <Button variant="default" onClick={handleBatchSetQuota} disabled={busyKey !== null}>
              {t("cliProxy.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 导入 Dialog */}
      <Dialog open={importOpen} onOpenChange={setImportOpen}>
        <DialogContent className="glass-elevated" style={{ maxWidth: 520, padding: 20 }}>
          <DialogHeader>
            <DialogTitle style={{ fontSize: 15, fontWeight: 600 }}>
              {t("cliProxy.import")}
            </DialogTitle>
          </DialogHeader>
          <label style={fieldLabel}>
            {t("cliProxy.importSource")}
            <div style={{ display: "flex", gap: 8 }}>
              <Input
                value={importSource}
                onChange={e => setImportSource(e.target.value)}
                placeholder="config.yaml / .zip / .tgz / dir"
              />
              <Button variant="ghost" onClick={() => void pickFile(setImportSource)} style={{ flexShrink: 0 }}>
                {t("cliProxy.importPickFile")}
              </Button>
            </div>
          </label>
          <label style={fieldLabel}>
            {t("cliProxy.importAuthDir")}
            <div style={{ display: "flex", gap: 8 }}>
              <Input
                value={importAuthDir}
                onChange={e => setImportAuthDir(e.target.value)}
                placeholder="~/.claude/auth.json dir (optional)"
              />
              <Button variant="ghost" onClick={() => void pickDir(setImportAuthDir)} style={{ flexShrink: 0 }}>
                {t("cliProxy.importPickDir")}
              </Button>
            </div>
          </label>
          <label style={fieldLabel}>
            {t("cliProxy.groupId")}
            <Select
              value={importGroupId === "" ? NONE : String(importGroupId)}
              onValueChange={v => setImportGroupId(v === NONE ? "" : Number(v))}
            >
              <SelectTrigger style={{ width: "100%" }}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={NONE}>—</SelectItem>
                {groups.map(g => (
                  <SelectItem key={g.id} value={String(g.id)}>{g.name}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </label>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setImportOpen(false)} disabled={busyKey !== null}>
              {t("cliProxy.cancel")}
            </Button>
            <Button variant="default" onClick={handleImport} disabled={busyKey !== null}>
              {t("cliProxy.import")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
