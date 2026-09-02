// ─── CliProxy 管理页 (cpa-standalone-module s5, c10 拆分) ───────────────
// 独立于 platform 表的 CLI 代理上游 provider 管理。后端 command 见 `commands_cli_proxy` crate。
// - 列表：name / wire / base_url / status + 操作（测试余额 / 建 platform 行 / 编辑 / 删除）
// - 编辑/新增：inline form（toggle 显示）
// - 导入：modal（源路径 + OAuth 凭据目录 + 分组），调 cli_proxy_import 批量入库
// 删除/导入 modal 均用 shadcn Dialog/AlertDialog（Radix Portal，满足 createPortal(document.body) 居中规则，
// 见 memory modal-window-center-rule）。
// 批量操作三 modal（删除/覆盖 models/设置 quota）已拆到同目录 Batch*Dialog.tsx；
// 选择态（selectMode/selectedIds/3 个 modal 开关等 7 项）收敛进 useCliProxySelection；
// quotaTypeOf 纯函数拆到 quotaTypeOf.ts（可独立测试）。

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { pickPath } from "../../services/pathPicker";
import {
  cliProxyApi,
  groupApi,
  type CliProxyProvider,
  type CreateCliProxyProvider,
  type Group,
  type CliProxyImportResult,
  type BatchReport,
} from "@/services/api";
import { makeRipple } from "@/utils/motion";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import { useCliProxySelection } from "./useCliProxySelection";
import { ProviderRow } from "./ProviderRow";
import { ProviderFormPanel } from "./ProviderFormPanel";
import { ImportDialog } from "./ImportDialog";
import { BatchDeleteDialog } from "./BatchDeleteDialog";
import { BatchModelsDialog } from "./BatchModelsDialog";
import { BatchQuotaDialog } from "./BatchQuotaDialog";

const EMPTY_FORM: CreateCliProxyProvider = {
  name: "", wire_protocol: "anthropic", base_url: "", api_key: "",
  models: [], extra: "", quota: "{}", status: "active", group_id: null,
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

  // 批量操作：selectMode/selectedIds/3 个 batch modal 开关+payload 全收进 useCliProxySelection。
  const sel = useCliProxySelection(providers.map(p => p.id));
  const enterSelect = () => { sel.enter(); setMsg(null); };
  const exitSelect = () => sel.exit();

  // silent=true：写操作后的刷新，不置 loading —— 否则整张列表被「加载中」占位替换，
  // 用户点一个开关看到的是整页重来一遍。首屏加载才需要 loading。
  const reload = useCallback(async (silent = false) => {
    if (!silent) setLoading(true);
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
      await reload(true);
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
      await reload(true);
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
    const ids = [...sel.selectedIds];
    if (ids.length === 0) return;
    setBusyKey("batch-del");
    setMsg(null);
    try {
      const r = await cliProxyApi.batchDelete(ids);
      reportToast(r, "cliProxy.batchDeleted");
      sel.closeBatchDelete();
      exitSelect();
      await reload(true);
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  const handleBatchOverrideModels = async () => {
    const ids = [...sel.selectedIds];
    if (ids.length === 0) return;
    const models = sel.batchModelsText.split("\n").map(s => s.trim()).filter(Boolean);
    setBusyKey("batch-models");
    setMsg(null);
    try {
      const r = await cliProxyApi.batchOverrideModels(ids, models);
      reportToast(r, "cliProxy.batchModelsUpdated");
      sel.closeBatchModels();
      exitSelect();
      await reload(true);
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  const handleBatchSetQuota = async () => {
    const ids = [...sel.selectedIds];
    if (ids.length === 0) return;
    const quota = JSON.stringify({ type: sel.batchQuotaType });
    setBusyKey("batch-quota");
    setMsg(null);
    try {
      const r = await cliProxyApi.batchSetQuota(ids, quota);
      reportToast(r, "cliProxy.batchQuotaUpdated");
      sel.closeBatchQuota();
      exitSelect();
      await reload(true);
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  const pickFile = async (setter: (v: string) => void) => {
    const picked = await pickPath();
    if (picked) setter(picked);
  };
  const pickDir = async (setter: (v: string) => void) => {
    const picked = await pickPath({ directory: true });
    if (picked) setter(picked);
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
      await reload(true);
    } catch (e) {
      setMsg({ kind: "err", text: String(e) });
    } finally {
      setBusyKey(null);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, width: "100%" }}>
      {/* 顶栏 */}
      <div className="section-header" style={{ justifyContent: "space-between" }}>
        <div>
          <div className="section-title">{t("cliProxy.title")}</div>
          <div className="section-desc">
            {t("cliProxy.subtitle", { count: providers.length })}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <Button
            variant="ghost"
            className="ripple"
            onClick={(e) => { makeRipple(e); openNew(); }}
            disabled={busyKey !== null}
          >
            {t("cliProxy.add")}
          </Button>
          <Button
            variant="default"
            className="ripple"
            onClick={(e) => { makeRipple(e); setImportOpen(true); setMsg(null); }}
            disabled={busyKey !== null}
          >
            {t("cliProxy.import")}
          </Button>
          <Button
            variant={sel.selectMode ? "destructive" : "ghost"}
            className="ripple"
            onClick={(e) => { makeRipple(e); sel.selectMode ? exitSelect() : enterSelect(); }}
            disabled={busyKey !== null}
          >
            {sel.selectMode ? t("cliProxy.exitSelect") : t("cliProxy.selectMode")}
          </Button>
        </div>
      </div>

      {/* 选择模式工具栏 */}
      {sel.selectMode && (
        <div className="glass-surface" style={{
          display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap",
          padding: "10px 14px",
        }}>
          <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 13, cursor: "pointer", color: "var(--text-primary)" }}>
            <Checkbox
              checked={sel.isAllSelected}
              onCheckedChange={sel.toggleAll}
            />
            {t("cliProxy.selectAll")}
          </label>
          <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
            {t("cliProxy.selectedCount", { count: sel.selectedCount })}
          </span>
          <div style={{ flex: 1 }} />
          <Button
            variant="destructive"
            className="ripple"
            onClick={(e) => { makeRipple(e); sel.openBatchDelete(); }}
            disabled={sel.selectedCount === 0 || busyKey !== null}
            style={{ opacity: sel.selectedCount === 0 ? 0.4 : 1 }}
          >
            {t("cliProxy.batchDelete")}
          </Button>
          <Button
            variant="ghost"
            className="ripple"
            onClick={(e) => { makeRipple(e); sel.openBatchModels(); }}
            disabled={sel.selectedCount === 0 || busyKey !== null}
            style={{ opacity: sel.selectedCount === 0 ? 0.4 : 1 }}
          >
            {t("cliProxy.batchOverrideModels")}
          </Button>
          <Button
            variant="ghost"
            className="ripple"
            onClick={(e) => { makeRipple(e); sel.openBatchQuota(); }}
            disabled={sel.selectedCount === 0 || busyKey !== null}
            style={{ opacity: sel.selectedCount === 0 ? 0.4 : 1 }}
          >
            {t("cliProxy.batchSetQuota")}
          </Button>
        </div>
      )}

      {/* 消息条 */}
      {msg && (
        <div className="glass-surface" style={{
          padding: "8px 12px",
          border: `1px solid ${msg.kind === "ok" ? "var(--color-success)" : "var(--color-danger)"}`,
          color: msg.kind === "ok" ? "var(--color-success)" : "var(--color-danger)",
          fontSize: 13,
        }}>
          {msg.text}
        </div>
      )}

      {/* 编辑/新增 inline form */}
      {editingId !== null && (
        <ProviderFormPanel
          editingId={editingId}
          form={form}
          setForm={setForm}
          modelsText={modelsText}
          setModelsText={setModelsText}
          groups={groups}
          busy={busyKey !== null}
          onCancel={cancelForm}
          onSave={() => void handleSave()}
        />
      )}

      {/* 列表 */}
      {loading ? (
        <div style={{ color: "var(--text-tertiary)", fontSize: 14 }}>
          {t("common.loading")}
        </div>
      ) : providers.length === 0 ? (
        <div className="glass-surface" style={{
          padding: 32, textAlign: "center", color: "var(--text-tertiary)", fontSize: 14,
        }}>
          {t("cliProxy.empty")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {providers.map((p, idx) => (
            <ProviderRow
              key={p.id}
              p={p}
              idx={idx}
              selectMode={sel.selectMode}
              selected={sel.isSelected(p.id)}
              busy={busyKey !== null}
              onToggle={() => sel.toggle(p.id)}
              onTest={() => void handleTest(p)}
              onCreatePlatform={() => void handleCreatePlatform(p)}
              onEdit={() => openEdit(p)}
              onDelete={() => setDeleteTarget(p)}
              t={t}
            />
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
              className="ripple"
              onClick={(e) => { makeRipple(e); void handleDelete(); }}
              disabled={busyKey !== null}
              style={{ background: "var(--color-danger)" }}
            >
              {t("cliProxy.delete")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* 批量删除确认 / 批量覆盖 models / 批量设置 quota：拆到同目录 Batch*Dialog（c10） */}
      <BatchDeleteDialog
        open={sel.batchDeleteOpen}
        onOpenChange={o => { if (!o) sel.closeBatchDelete(); }}
        providers={providers}
        selectedIds={sel.selectedIds}
        busy={busyKey !== null}
        onConfirm={() => void handleBatchDelete()}
      />
      <BatchModelsDialog
        open={sel.batchModelsOpen}
        onOpenChange={o => { if (!o) sel.closeBatchModels(); }}
        selectedCount={sel.selectedCount}
        value={sel.batchModelsText}
        onChange={sel.setBatchModelsText}
        busy={busyKey !== null}
        onSave={() => void handleBatchOverrideModels()}
      />
      <BatchQuotaDialog
        open={sel.batchQuotaOpen}
        onOpenChange={o => { if (!o) sel.closeBatchQuota(); }}
        selectedCount={sel.selectedCount}
        quotaType={sel.batchQuotaType}
        onQuotaTypeChange={sel.setBatchQuotaType}
        busy={busyKey !== null}
        onSave={() => void handleBatchSetQuota()}
      />

      {/* 导入 Dialog */}
      <ImportDialog
        open={importOpen}
        onOpenChange={setImportOpen}
        source={importSource}
        setSource={setImportSource}
        authDir={importAuthDir}
        setAuthDir={setImportAuthDir}
        groupId={importGroupId}
        setGroupId={setImportGroupId}
        groups={groups}
        busy={busyKey !== null}
        onPickFile={() => void pickFile(setImportSource)}
        onPickDir={() => void pickDir(setImportAuthDir)}
        onImport={() => void handleImport()}
      />
    </div>
  );
}
