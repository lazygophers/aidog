// ─── 导入导出子系统 UI ────────────────────────────────────
// AppSettings「导入导出」tab。导出勾选范围 → 加密单文件 .aidogx；
// 导入选文件 → 冲突预览 → 逐项决策 → 应用。消费 services/api.ts importExportApi 契约。
//
// 视觉重设（06-14-import-export-redesign）：scope 卡片化 + 拖放式导入入口 +
// 冲突分段控件 + report 语义色卡片。功能/数据流 100% 不变，纯展示层。
// 全部样式走主题令牌（--radius-*/--shadow-*/--transition/--accent*/--border*/--bg-*/--text-*/--color-*），
// 随 9 style × 12 palette 自适应；无硬编码主题色、无 emoji。

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
import { useApp } from "../../context/AppContext";
import { SectionIcon } from "./editors";
import { IconCheck } from "../icons";
import { StatChip } from "../shared/StatChip";
import type { ColorLevel } from "../shared/colorScale";

// scope 元数据：id + i18n labelKey + 默认 label + 映射图标（PRD scope→icon）+ 一行描述。
const ALL_SCOPES: {
  id: ImportExportScope;
  labelKey: string;
  defaultLabel: string;
  icon: string;
  descKey: string;
  defaultDesc: string;
}[] = [
  { id: "platform", labelKey: "importExport.scope.platform", defaultLabel: "平台", icon: "network", descKey: "importExport.scopeDesc.platform", defaultDesc: "平台连接与凭据" },
  { id: "group", labelKey: "importExport.scope.group", defaultLabel: "分组", icon: "team", descKey: "importExport.scopeDesc.group", defaultDesc: "分组与调度策略" },
  { id: "group_platform", labelKey: "importExport.scope.groupPlatform", defaultLabel: "分组↔平台关联", icon: "worktree", descKey: "importExport.scopeDesc.groupPlatform", defaultDesc: "分组与平台的关联关系" },
  { id: "setting", labelKey: "importExport.scope.setting", defaultLabel: "全局设置", icon: "bolt", descKey: "importExport.scopeDesc.setting", defaultDesc: "主题 / 语言 / 代理 / 通知等" },
  { id: "codex", labelKey: "importExport.scope.codex", defaultLabel: "Codex 设置", icon: "file", descKey: "importExport.scopeDesc.codex", defaultDesc: "Codex 配置" },
  { id: "claude_code", labelKey: "importExport.scope.claudeCode", defaultLabel: "Claude Code 设置", icon: "memory", descKey: "importExport.scopeDesc.claudeCode", defaultDesc: "Claude Code 配置" },
  { id: "model_price", labelKey: "importExport.scope.modelPrice", defaultLabel: "模型价格表", icon: "advanced", descKey: "importExport.scopeDesc.modelPrice", defaultDesc: "模型定价数据" },
  { id: "skills", labelKey: "importExport.scope.skills", defaultLabel: "Skills", icon: "plugins", descKey: "importExport.scopeDesc.skills", defaultDesc: "npx 安装 + 启用状态" },
];

const SCOPE_ICON: Record<string, string> = Object.fromEntries(ALL_SCOPES.map((s) => [s.id, s.icon]));

function scopeLabel(t: TFunction, scope: string): string {
  const entry = ALL_SCOPES.find((s) => s.id === scope);
  if (!entry) return scope;
  return t(entry.labelKey, entry.defaultLabel);
}

export function ImportExportTab() {
  const { t } = useTranslation();
  const { reloadFromDB } = useApp();
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

  const selectAll = () => setScopes(new Set(ALL_SCOPES.map((s) => s.id)));
  const deselectAll = () => setScopes(new Set());

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

  // 批量：对全部冲突一次性设置覆盖 / 跳过。
  const bulkSet = (kind: "overwrite" | "skip") => {
    if (!preview) return;
    setDecisions(() => {
      const next = new Map<string, ImportDecision>();
      for (const c of preview.conflicts) next.set(decisionKey(c.scope, c.key), { kind });
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
      // 应用后从 DB 重读主题/语言偏好（导入 setting scope 含 theme/locale 时即时生效）
      await reloadFromDB().catch(() => {});
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  const selectedCount = scopes.size;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 24, maxWidth: 760 }}>
      {/* ── 导出区 ── */}
      <section className="glass" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 16 }}>
        <SectionHeader icon="folder" title={t("importExport.exportTitle", "导出")} desc={t("importExport.exportDesc", "勾选要导出的内容，加密为单文件 .aidogx（密钥隐藏在文件内，人眼无法识别）。")} />

        {/* scope 区头：标题 + 全选/反选 + 选中计数 */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>
              {t("importExport.scopeHeader", "导出范围")}
            </span>
            <TextButton onClick={selectAll}>{t("importExport.selectAll", "全选")}</TextButton>
            <TextButton onClick={deselectAll}>{t("importExport.deselectAll", "反选")}</TextButton>
          </div>
          <StatChip
            value={`${selectedCount} / ${ALL_SCOPES.length}`}
            label={t("importExport.selectedLabel", "已选")}
            level={(selectedCount > 0 ? "success" : "neutral") as ColorLevel}
          />
        </div>

        {/* scope 卡片网格 */}
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))", gap: 10 }}>
          {ALL_SCOPES.map((s) => (
            <ScopeCard
              key={s.id}
              icon={s.icon}
              label={t(s.labelKey, s.defaultLabel)}
              desc={t(s.descKey, s.defaultDesc)}
              selected={scopes.has(s.id)}
              onToggle={() => toggleScope(s.id)}
            />
          ))}
        </div>

        <button
          onClick={handleExport}
          disabled={exporting || selectedCount === 0}
          className="btn btn-primary"
          style={{ alignSelf: "flex-start" }}
        >
          {exporting
            ? t("importExport.exporting", "导出中…")
            : t("importExport.exportN", "导出 {{n}} 项", { n: selectedCount })}
        </button>

        {exportMsg && <SuccessPathCard message={exportMsg} />}
      </section>

      {/* ── 导入区 ── */}
      <section className="glass" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 16 }}>
        <SectionHeader icon="worktree" title={t("importExport.importTitle", "导入")} desc={t("importExport.importDesc", "选择 .aidogx 文件，程序自动解密。冲突项逐条决策；Skill 自动安装并恢复原启用状态。")} />

        {/* 拖放式入口（视觉拖放风，实际点击触发 Tauri open） */}
        <DropZone
          onClick={handlePickFile}
          title={t("importExport.pickFile", "选择 .aidogx 文件")}
          hint={t("importExport.dropHint", "自动解密 · Skill 自动安装")}
        />

        {preview && (
          <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
            {/* 概要卡：来源机器 + 导出时间 meta 行 + counts StatChip 行 */}
            <div className="glass-surface" style={{ padding: 14, borderRadius: "var(--radius-lg)", display: "flex", flexDirection: "column", gap: 12 }}>
              <MetaRow label={t("importExport.sourceMachine", "来源机器")} value={preview.manifest.source_machine} />
              <MetaRow label={t("importExport.createdAt", "导出时间")} value={preview.manifest.created_at} />
              <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
                {Object.entries(preview.counts).map(([k, v]) => (
                  <StatChip
                    key={k}
                    icon={<SectionIcon name={SCOPE_ICON[k] ?? "folder"} size={13} />}
                    value={String(v)}
                    label={scopeLabel(t, k)}
                  />
                ))}
              </div>
            </div>

            {preview.conflicts.length > 0 && (
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
                  <strong style={{ fontSize: 14, color: "var(--color-warning)" }}>
                    {t("importExport.conflicts", "冲突（{{n}} 项）", { n: preview.conflicts.length })}
                  </strong>
                  <div style={{ display: "flex", gap: 8 }}>
                    <TextButton onClick={() => bulkSet("overwrite")}>{t("importExport.bulkOverwrite", "全部覆盖")}</TextButton>
                    <TextButton onClick={() => bulkSet("skip")}>{t("importExport.bulkSkip", "全部跳过")}</TextButton>
                  </div>
                </div>
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

            <button onClick={handleApply} disabled={importing} className="btn btn-primary" style={{ alignSelf: "flex-start" }}>
              {importing
                ? t("importExport.applying", "导入中…")
                : t("importExport.applyBtn", "应用导入")}
            </button>
          </div>
        )}

        {report && <ReportView report={report} t={t} scopeLabel={(s: string) => scopeLabel(t, s)} />}
      </section>

      {error && (
        <div
          style={{
            padding: "10px 14px",
            borderRadius: "var(--radius-md)",
            background: "var(--color-danger-bg)",
            border: "1px solid var(--color-danger)",
            color: "var(--color-danger)",
            fontSize: 13,
          }}
        >
          {error}
        </div>
      )}
    </div>
  );
}

// ─── Sub-components ────────────────────────────────────────

/** section 头：图标 + 标题 + 描述。 */
function SectionHeader({ icon, title, desc }: { icon: string; title: string; desc: string }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <SectionIcon name={icon} size={18} style={{ color: "var(--accent)" }} />
        <h3 style={{ margin: 0, fontSize: 18, fontWeight: 600, color: "var(--text-primary)" }}>{title}</h3>
      </div>
      <p style={{ margin: 0, fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>{desc}</p>
    </div>
  );
}

/** 文字按钮（全选/反选/批量），accent 文字、无填充。 */
function TextButton({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      style={{
        background: "transparent",
        border: "none",
        color: "var(--accent)",
        fontSize: 13,
        fontWeight: 500,
        cursor: "pointer",
        padding: 0,
      }}
    >
      {children}
    </button>
  );
}

/** scope 选择卡：整卡可点 toggle，选中态 accent 边 + subtle 底 + 右上角 ✓。 */
function ScopeCard({
  icon,
  label,
  desc,
  selected,
  onToggle,
}: {
  icon: string;
  label: string;
  desc: string;
  selected: boolean;
  onToggle: () => void;
}) {
  const [hover, setHover] = useState(false);
  return (
    <div
      className="glass-surface"
      role="button"
      tabIndex={0}
      onClick={onToggle}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onToggle();
        }
      }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        position: "relative",
        padding: 14,
        borderRadius: "var(--radius-lg)",
        cursor: "pointer",
        border: `1px solid ${selected ? "var(--accent)" : "var(--border)"}`,
        background: selected ? "var(--accent-subtle)" : "transparent",
        boxShadow: hover ? "var(--shadow-md)" : "var(--shadow-sm)",
        transform: hover ? "translateY(-1px)" : "none",
        transition: "var(--transition)",
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      {/* 右上角选中指示 */}
      <span
        style={{
          position: "absolute",
          top: 10,
          right: 10,
          width: 18,
          height: 18,
          borderRadius: "50%",
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          border: `1px solid ${selected ? "var(--accent)" : "var(--border)"}`,
          background: selected ? "var(--accent)" : "transparent",
          transition: "var(--transition)",
        }}
      >
        {selected && <IconCheck size={12} color="#fff" strokeWidth={2.5} />}
      </span>

      <SectionIcon name={icon} size={20} style={{ color: selected ? "var(--accent)" : "var(--text-secondary)" }} />
      <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)", paddingRight: 24 }}>{label}</div>
      <div style={{ fontSize: 12, color: "var(--text-tertiary)", lineHeight: 1.4 }}>{desc}</div>
    </div>
  );
}

/** 导出成功消息卡（含文件路径，语义成功色）。 */
function SuccessPathCard({ message }: { message: string }) {
  return (
    <div
      className="glass-elevated"
      style={{
        padding: 12,
        borderRadius: "var(--radius-md)",
        border: "1px solid var(--color-success)",
        background: "var(--color-success-bg)",
        display: "flex",
        alignItems: "center",
        gap: 10,
      }}
    >
      <IconCheck size={16} color="var(--color-success)" strokeWidth={2.5} style={{ flexShrink: 0 }} />
      <span
        style={{
          fontSize: 13,
          color: "var(--text-primary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
        title={message}
      >
        {message}
      </span>
    </div>
  );
}

/** 拖放式导入入口（虚线 glass 区，点击触发 open）。 */
function DropZone({ onClick, title, hint }: { onClick: () => void; title: string; hint: string }) {
  const [hover, setHover] = useState(false);
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        padding: "28px 20px",
        borderRadius: "var(--radius-lg)",
        border: `1.5px dashed ${hover ? "var(--accent)" : "var(--border)"}`,
        background: hover ? "var(--accent-subtle)" : "var(--bg-glass)",
        cursor: "pointer",
        transition: "var(--transition)",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 8,
        textAlign: "center",
      }}
    >
      <SectionIcon name="file" size={28} style={{ color: hover ? "var(--accent)" : "var(--text-secondary)" }} />
      <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>{title}</div>
      <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{hint}</div>
    </div>
  );
}

/** meta 行：左 label（次级）右 value（主）。 */
function MetaRow({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: "flex", alignItems: "baseline", gap: 8, fontSize: 13 }}>
      <span style={{ color: "var(--text-tertiary)", minWidth: 72 }}>{label}</span>
      <span style={{ color: "var(--text-primary)", fontWeight: 500, wordBreak: "break-all" }}>{value}</span>
    </div>
  );
}

/** 3 段分段控件（覆盖/跳过/重命名）。 */
function Segmented({
  value,
  options,
  onSelect,
}: {
  value: string;
  options: { id: string; label: string }[];
  onSelect: (id: string) => void;
}) {
  return (
    <div
      style={{
        display: "inline-flex",
        borderRadius: "var(--radius-sm)",
        border: "1px solid var(--border)",
        overflow: "hidden",
      }}
    >
      {options.map((opt, i) => {
        const active = value === opt.id;
        return (
          <button
            key={opt.id}
            onClick={() => onSelect(opt.id)}
            style={{
              padding: "5px 12px",
              fontSize: 12,
              fontWeight: active ? 600 : 500,
              cursor: "pointer",
              border: "none",
              borderLeft: i > 0 ? "1px solid var(--border)" : "none",
              background: active ? "var(--accent-subtle)" : "transparent",
              color: active ? "var(--accent)" : "var(--text-secondary)",
              transition: "var(--transition)",
            }}
          >
            {opt.label}
          </button>
        );
      })}
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
    <div
      className="glass-surface"
      style={{
        padding: 12,
        borderRadius: "var(--radius-md)",
        border: "1px solid var(--border)",
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
        <StatChip
          icon={<SectionIcon name={SCOPE_ICON[item.scope] ?? "folder"} size={12} />}
          value={scopeLabel}
          label=""
        />
        <span style={{ fontWeight: 600, color: "var(--text-primary)", fontSize: 13, wordBreak: "break-all" }}>{item.key}</span>
      </div>
      <div style={{ fontSize: 12, color: "var(--text-tertiary)", lineHeight: 1.4 }}>{item.existing_summary}</div>
      <div style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap" }}>
        <Segmented
          value={current.kind}
          options={[
            { id: "overwrite", label: t("importExport.overwrite", "覆盖") },
            { id: "skip", label: t("importExport.skip", "跳过") },
            { id: "rename", label: t("importExport.rename", "重命名") },
          ]}
          onSelect={(id) => {
            if (id === "rename") onChange({ kind: "rename", new_key: item.key + "-imported" });
            else onChange({ kind: id as "overwrite" | "skip" });
          }}
        />
        {isRename && (
          <input
            className="input"
            type="text"
            value={(current as { kind: "rename"; new_key: string }).new_key}
            onChange={(e) => onChange({ kind: "rename", new_key: e.target.value })}
            style={{ width: 220 }}
          />
        )}
      </div>
    </div>
  );
}

/** 结果卡：applied(成功区) / skipped(中性区) / errors(危险区)。 */
function ReportView({
  report,
  t,
  scopeLabel,
}: {
  report: ImportReport;
  t: TFunction;
  scopeLabel: (s: string) => string;
}) {
  const applied = Object.entries(report.applied);
  const skipped = Object.entries(report.skipped);
  const appliedTotal = applied.reduce((a, [, v]) => a + v, 0);
  const skippedTotal = skipped.reduce((a, [, v]) => a + v, 0);

  return (
    <div className="glass-surface" style={{ padding: 14, borderRadius: "var(--radius-lg)", display: "flex", flexDirection: "column", gap: 12 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
        <strong style={{ fontSize: 14, color: "var(--text-primary)" }}>{t("importExport.reportTitle", "导入结果")}</strong>
        <StatChip value={String(appliedTotal)} label={t("importExport.applied", "已应用")} level="success" />
        <StatChip value={String(skippedTotal)} label={t("importExport.skipped", "已跳过")} level="neutral" />
        {report.errors.length > 0 && (
          <StatChip value={String(report.errors.length)} label={t("importExport.errorsLabel", "错误")} level="danger" />
        )}
      </div>

      {applied.length > 0 && (
        <ReportSection
          title={t("importExport.applied", "已应用")}
          color="var(--color-success)"
          bg="var(--color-success-bg)"
          icon={<IconCheck size={13} color="var(--color-success)" strokeWidth={2.5} />}
          rows={applied.map(([k, v]) => `${scopeLabel(k)}: ${v}`)}
        />
      )}
      {skipped.length > 0 && (
        <ReportSection
          title={t("importExport.skipped", "已跳过")}
          color="var(--text-tertiary)"
          bg="var(--color-neutral-bg)"
          icon={<SectionIcon name="status" size={13} style={{ color: "var(--text-tertiary)" }} />}
          rows={skipped.map(([k, v]) => `${scopeLabel(k)}: ${v}`)}
        />
      )}
      {report.errors.length > 0 && (
        <ReportSection
          title={t("importExport.errors", "错误（{{n}}）", { n: report.errors.length })}
          color="var(--color-danger)"
          bg="var(--color-danger-bg)"
          rows={report.errors}
        />
      )}
    </div>
  );
}

/** report 单语义区：小标题 + 行列表。 */
function ReportSection({
  title,
  color,
  bg,
  icon,
  rows,
}: {
  title: string;
  color: string;
  bg: string;
  icon?: React.ReactNode;
  rows: string[];
}) {
  return (
    <div
      style={{
        padding: 10,
        borderRadius: "var(--radius-md)",
        background: bg,
        border: `1px solid ${color}`,
        display: "flex",
        flexDirection: "column",
        gap: 4,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 13, fontWeight: 600, color }}>
        {icon}
        {title}
      </div>
      {rows.map((r, i) => (
        <div key={i} style={{ fontSize: 12, color: "var(--text-secondary)", paddingLeft: icon ? 19 : 0, wordBreak: "break-all" }}>
          {r}
        </div>
      ))}
    </div>
  );
}
