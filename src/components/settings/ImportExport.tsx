// ─── 导入导出子系统 UI ────────────────────────────────────
// AppSettings「导入导出」tab。导出勾选范围 → 加密单文件 .aidogx；
// 导入选文件 → 冲突预览 → 逐项决策 → 应用。消费 services/api.ts importExportApi 契约。

import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { save, open } from "@tauri-apps/plugin-dialog";
import {
  importExportApi,
  type ImportExportScope,
  type ConflictItem,
  type ConflictDecision,
  type ImportDecision,
  type ImportPreview,
  type ImportReport,
} from "../../services/api";

const ALL_SCOPES: { id: ImportExportScope; labelKey: string; defaultLabel: string }[] = [
  { id: "platform", labelKey: "importExport.scope.platform", defaultLabel: "平台" },
  { id: "group", labelKey: "importExport.scope.group", defaultLabel: "分组" },
  { id: "group_platform", labelKey: "importExport.scope.groupPlatform", defaultLabel: "分组↔平台关联" },
  { id: "setting", labelKey: "importExport.scope.setting", defaultLabel: "代理全局设置" },
  { id: "codex", labelKey: "importExport.scope.codex", defaultLabel: "Codex 设置" },
  { id: "claude_code", labelKey: "importExport.scope.claudeCode", defaultLabel: "Claude Code 设置" },
  { id: "model_price", labelKey: "importExport.scope.modelPrice", defaultLabel: "模型价格表" },
  { id: "skills", labelKey: "importExport.scope.skills", defaultLabel: "Skills（npx 安装 + 启用状态）" },
];

function scopeLabel(t: TFunction, scope: string): string {
  const entry = ALL_SCOPES.find((s) => s.id === scope);
  if (!entry) return scope;
  return t(entry.labelKey, entry.defaultLabel);
}

export function ImportExportTab() {
  const { t } = useTranslation();
  const [scopes, setScopes] = useState<Set<ImportExportScope>>(
    new Set<ImportExportScope>(["platform", "group", "group_platform", "setting"]),
  );
  const [exporting, setExporting] = useState(false);
  const [exportMsg, setExportMsg] = useState("");

  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [decisions, setDecisions] = useState<Map<string, ImportDecision>>(new Map());
  const [importPath, setImportPath] = useState("");
  const [importing, setImporting] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [error, setError] = useState("");

  const toggleScope = (id: ImportExportScope) => {
    setScopes((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleExport = async () => {
    setError("");
    setExportMsg("");
    if (scopes.size === 0) {
      setError(t("importExport.error.noScope", "请至少勾选一项导出范围"));
      return;
    }
    try {
      const path = await save({
        defaultPath: `aidog-export-${new Date().toISOString().slice(0, 10)}.aidogx`,
        filters: [{ name: "AiDog Export", extensions: ["aidogx"] }],
      });
      if (!path) return;
      setExporting(true);
      await importExportApi.exportToFile(Array.from(scopes), path);
      setExportMsg(t("importExport.exportDone", "导出成功：{{path}}", { path }));
    } catch (e) {
      setError(String(e));
    } finally {
      setExporting(false);
    }
  };

  const handlePickFile = async () => {
    setError("");
    setReport(null);
    setPreview(null);
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "AiDog Export", extensions: ["aidogx"] }],
      });
      if (!selected || typeof selected !== "string") return;
      const p = selected as string;
      const prev = await importExportApi.readPreview(p);
      setImportPath(p);
      setPreview(prev);
      // 默认全部 overwrite。
      const map = new Map<string, ImportDecision>();
      for (const c of prev.conflicts) {
        map.set(decisionKey(c.scope, c.key), { kind: "overwrite" });
      }
      setDecisions(map);
    } catch (e) {
      setError(String(e));
    }
  };

  const decisionKey = (scope: string, key: string) => `${scope}::${key}`;

  const setDecision = (c: ConflictItem, d: ImportDecision) => {
    setDecisions((prev) => {
      const next = new Map(prev);
      next.set(decisionKey(c.scope, c.key), d);
      return next;
    });
  };

  const handleApply = async () => {
    if (!importPath) return;
    setError("");
    setImporting(true);
    try {
      const ds: ConflictDecision[] = Array.from(decisions.entries()).map(([k, d]) => {
        const [scope, key] = k.split("::");
        return { scope, key, decision: d };
      });
      const r = await importExportApi.apply(importPath, ds);
      setReport(r);
      setPreview(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 24, maxWidth: 760 }}>
      {/* 导出区 */}
      <section className="glass" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 14 }}>
        <h3 style={{ margin: 0 }}>{t("importExport.exportTitle", "导出")}</h3>
        <p style={{ margin: 0, opacity: 0.7, fontSize: 13 }}>
          {t("importExport.exportDesc", "勾选要导出的内容，加密为单文件 .aidogx（密钥隐藏在文件内，人眼无法识别）。")}
        </p>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
          {ALL_SCOPES.map((s) => (
            <label key={s.id} style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}>
              <input
                type="checkbox"
                checked={scopes.has(s.id)}
                onChange={() => toggleScope(s.id)}
              />
              <span>{scopeLabel(t, s.id)}</span>
            </label>
          ))}
        </div>
        <button onClick={handleExport} disabled={exporting} className="btn-primary">
          {exporting ? t("importExport.exporting", "导出中…") : t("importExport.exportBtn", "选择路径并导出")}
        </button>
        {exportMsg && <div className="toast" style={{ background: "var(--bg-floating)", borderColor: "var(--success)", color: "var(--success)" }}>{exportMsg}</div>}
      </section>

      {/* 导入区 */}
      <section className="glass" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 14 }}>
        <h3 style={{ margin: 0 }}>{t("importExport.importTitle", "导入")}</h3>
        <p style={{ margin: 0, opacity: 0.7, fontSize: 13 }}>
          {t("importExport.importDesc", "选择 .aidogx 文件，程序自动解密。冲突项逐条决策；Skill 自动安装并恢复原启用状态。")}
        </p>
        <button onClick={handlePickFile} className="btn-primary">
          {t("importExport.pickFile", "选择 .aidogx 文件")}
        </button>

        {preview && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <div style={{ fontSize: 13, opacity: 0.8 }}>
              {t("importExport.sourceMachine", "来源机器")}: {preview.manifest.source_machine}
              {" · "}
              {t("importExport.createdAt", "导出时间")}: {preview.manifest.created_at}
            </div>
            <div style={{ fontSize: 13 }}>
              {Object.entries(preview.counts).map(([k, v]) => (
                <span key={k} style={{ marginRight: 12 }}>
                  {scopeLabel(t, k)}: {v}
                </span>
              ))}
            </div>

            {preview.conflicts.length > 0 && (
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <strong>{t("importExport.conflicts", "冲突（{{n}} 项）", { n: preview.conflicts.length })}</strong>
                {preview.conflicts.map((c) => {
                  const dk = decisionKey(c.scope, c.key);
                  const cur = decisions.get(dk) || { kind: "overwrite" };
                  return (
                    <ConflictRow
                      key={dk}
                      item={c}
                      scopeLabel={scopeLabel(t, c.scope)}
                      current={cur}
                      onChange={(d) => setDecision(c, d)}
                      t={t}
                    />
                  );
                })}
              </div>
            )}

            <button onClick={handleApply} disabled={importing} className="btn-primary">
              {importing
                ? t("importExport.applying", "导入中…")
                : t("importExport.applyBtn", "应用导入")}
            </button>
          </div>
        )}

        {report && <ReportView report={report} t={t} scopeLabel={(s: string) => scopeLabel(t, s)} />}
      </section>

      {error && <div className="toast" style={{ background: "var(--bg-floating)", borderColor: "var(--danger)", color: "var(--danger)" }}>{error}</div>}
    </div>
  );
}

function ConflictRow({
  item,
  scopeLabel,
  current,
  onChange,
  t,
}: {
  item: ConflictItem;
  scopeLabel: string;
  current: ImportDecision;
  onChange: (d: ImportDecision) => void;
  t: TFunction;
}) {
  const isRename = current.kind === "rename";
  return (
    <div style={{ padding: 10, border: "1px solid var(--border)", borderRadius: 8, display: "flex", flexDirection: "column", gap: 6 }}>
      <div style={{ fontWeight: 600 }}>
        [{scopeLabel}] {item.key}
      </div>
      <div style={{ fontSize: 12, opacity: 0.75 }}>{item.existing_summary}</div>
      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        {(["overwrite", "skip", "rename"] as const).map((kind) => (
          <label key={kind} style={{ display: "flex", alignItems: "center", gap: 4, cursor: "pointer" }}>
            <input
              type="radio"
              name={item.key + item.scope}
              checked={current.kind === kind}
              onChange={() => {
                if (kind === "rename") onChange({ kind: "rename", new_key: item.key + "-imported" });
                else onChange({ kind });
              }}
            />
            {kind === "overwrite"
              ? t("importExport.overwrite", "覆盖")
              : kind === "skip"
              ? t("importExport.skip", "跳过")
              : t("importExport.rename", "重命名")}
          </label>
        ))}
        {isRename && (
          <input
            type="text"
            value={(current as { kind: "rename"; new_key: string }).new_key}
            onChange={(e) => onChange({ kind: "rename", new_key: e.target.value })}
            style={{ width: 200 }}
          />
        )}
      </div>
    </div>
  );
}

function ReportView({
  report,
  t,
  scopeLabel,
}: {
  report: ImportReport;
  t: TFunction;
  scopeLabel: (s: string) => string;
}) {
  return (
    <div className="glass" style={{ padding: 14, display: "flex", flexDirection: "column", gap: 6 }}>
      <strong>{t("importExport.reportTitle", "导入结果")}</strong>
      {Object.entries(report.applied).map(([k, v]) => (
        <div key={"a" + k} style={{ fontSize: 13 }}>
          ✓ {scopeLabel(k)}: {v}
        </div>
      ))}
      {Object.entries(report.skipped).map(([k, v]) => (
        <div key={"s" + k} style={{ fontSize: 13, opacity: 0.7 }}>
          ⊘ {scopeLabel(k)}: {v}
        </div>
      ))}
      {report.errors.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 2, marginTop: 6 }}>
          <strong style={{ color: "var(--danger)" }}>
            {t("importExport.errors", "错误（{{n}}）", { n: report.errors.length })}
          </strong>
          {report.errors.map((e, i) => (
            <div key={i} style={{ fontSize: 12, color: "var(--danger)" }}>
              {e}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
