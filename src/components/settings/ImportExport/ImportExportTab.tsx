// 导入导出子系统主组件（AppSettings「导入导出」tab）。
// 导出勾选范围 → 加密单文件 .aidogx；导入选文件 → 冲突预览 → 逐项决策 → 应用。
// 消费 services/api.ts importExportApi 契约。
//
// 视觉重设（06-14-import-export-redesign）：scope 卡片化 + 拖放式导入入口 +
// 冲突分段控件 + report 语义色卡片。功能/数据流 100% 不变，纯展示层。
// 全部样式走主题令牌（--radius-*/--shadow-*/--transition/--accent*/--border*/--bg-*/--text-*/--color-*），
// 随 9 style × 12 palette 自适应；无硬编码主题色、无 emoji。
//
// 抽自原 ImportExport.tsx L164-696。state / handlers / effects / JSX 全部保留原貌（行为零变更）。

import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { save, open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  importExportApi,
  type ImportExportScope,
  type ConflictItem,
  type ConflictDecision,
  type ImportDecision,
  type ImportItem,
  type ImportPreview,
  type ImportReport,
} from "../../../services/api";
import { useApp } from "../../../context/AppContext";
import { SectionIcon } from "../editors";
import { CcSwitchImportSection } from "../CcSwitchImport";
import { Sub2ApiImportSection } from "../Sub2ApiImport";
import { StatChip } from "../../shared/StatChip";
import type { ColorLevel } from "../../shared/colorScale";
import {
  ALL_SCOPES,
  SCOPE_ICON,
  SCOPE_MENU_GROUP,
  MENU_GROUP_ICON,
  scopeLabel,
  menuGroupOf,
  menuGroupLabel,
  type MenuGroupId,
} from "./meta";
import {
  SectionHeader,
  TextButton,
  ScopeCard,
  SuccessPathCard,
  DropZone,
  MetaRow,
} from "./primitives";
import { ItemSelector } from "./ItemSelector";
import { ConflictRow } from "./ConflictRow";
import { ReportView } from "./ReportView";
import { ScheduledBackupSection } from "./ScheduledBackupSection";
import { Button } from "@/components/ui/button";

export function ImportExportTab() {
  const { t } = useTranslation();
  const { reloadFromDB } = useApp();
  const [scopes, setScopes] = useState<Set<ImportExportScope>>(
    new Set<ImportExportScope>(["platform", "group", "group_platform", "setting"]),
  );
  const [exporting, setExporting] = useState(false);
  const [exportMsg, setExportMsg] = useState("");
  // 导出预览（逐项勾选）：调 export_preview 拉全量条目，用户增删后只导出勾中项。
  const [exportPreview, setExportPreview] = useState<ImportPreview | null>(null);
  const [exportSelected, setExportSelected] = useState<Set<string>>(new Set());
  const [previewing, setPreviewing] = useState(false);

  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [decisions, setDecisions] = useState<Map<string, ImportDecision>>(new Map());
  // 逐项勾选白名单（key = `${scope}::${key}`）。默认全选；未勾选项不导入。
  const [selectedItems, setSelectedItems] = useState<Set<string>>(new Set());
  const [importPath, setImportPath] = useState("");
  const [importing, setImporting] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [error, setError] = useState("");
  // 原生文件拖入高亮态（Tauri onDragDropEvent；HTML5 DnD 在 macOS WKWebView 失效，故走原生事件）。
  const [dragActive, setDragActive] = useState(false);
  // loadPreview 最新引用，供拖入回调调用（避免 effect 依赖 loadPreview 反复重订阅）。
  const loadPreviewRef = useRef<(p: string) => Promise<void>>(async () => {});

  const selectAll = () => setScopes(new Set(ALL_SCOPES.map((s) => s.id)));
  const deselectAll = () => setScopes(new Set());

  // 步骤1：勾 scope 后 debounce(~300ms) 自动拉全量条目预览，默认全选。
  // 取代旧的「预览导出项」按钮 — 勾选即展开条目，连续勾多个 scope 只拉一次（防抖）。
  const loadExportPreview = async (scopeList: ImportExportScope[]) => {
    if (scopeList.length === 0) {
      setExportPreview(null);
      setExportSelected(new Set());
      return;
    }
    setPreviewing(true);
    setError("");
    setExportMsg("");
    try {
      const prev = await importExportApi.exportPreview(scopeList);
      setExportPreview(prev);
      // 默认全选（含 skills：用户主动定方案 C 删 filter，npx 误删防御由后端守卫兜底）。
      setExportSelected(
        new Set(prev.items.map((it) => itemKey(it.scope, it.key))),
      );
    } catch (e) {
      setError(String(e));
      setExportPreview(null);
    } finally {
      setPreviewing(false);
    }
  };

  // scopes 变化 → debounce 自动拉预览。依赖 scope 集合的规范化字符串，任何增/删/换都触发。
  const scopesSig = ALL_SCOPES.map((s) => (scopes.has(s.id) ? "1" : "0")).join("");
  useEffect(() => {
    if (scopes.size === 0) {
      setExportPreview(null);
      setExportSelected(new Set());
      return;
    }
    const snapshot = Array.from(scopes) as ImportExportScope[];
    const handle = window.setTimeout(() => {
      void loadExportPreview(snapshot);
    }, 300);
    return () => window.clearTimeout(handle);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopesSig]);

  // 步骤2：导出勾选条目。全选时传 null（全量，省 selection payload，走向后兼容路径）。
  // 条目预览由 scopes 变化的 debounce effect 自动拉取，此处不再兜底触发。
  const handleExport = async () => {
    setError("");
    setExportMsg("");
    if (!exportPreview) return;
    try {
      const path = await save({
        defaultPath: `aidog-export-${new Date().toISOString().slice(0, 10)}.aidogx`,
        filters: [{ name: "AiDog Export", extensions: ["aidogx"] }],
      });
      if (!path) return;
      setExporting(true);
      const allSelected = exportSelected.size === exportPreview.items.length;
      const selection: [string, string][] | null = allSelected
        ? null
        : exportPreview.items
            .filter((it) => exportSelected.has(itemKey(it.scope, it.key)))
            .map((it) => [it.scope, it.key]);
      await importExportApi.exportToFile(Array.from(scopes), path, selection);
      setExportMsg(t("importExport.exportDone", "导出成功：{{path}}", { path }));
    } catch (e) {
      setError(String(e));
    } finally {
      setExporting(false);
    }
  };

  // 导出逐项勾选操作（与导入侧对称，复用 itemKey）。
  const toggleExportItem = (it: ImportItem) => {
    const k = itemKey(it.scope, it.key);
    setExportSelected((prev) => {
      const next = new Set(prev);
      if (next.has(k)) next.delete(k);
      else next.add(k);
      return next;
    });
  };
  const setExportGroupItems = (groupItems: ImportItem[], select: boolean) => {
    setExportSelected((prev) => {
      const next = new Set(prev);
      for (const it of groupItems) {
        const k = itemKey(it.scope, it.key);
        if (select) next.add(k);
        else next.delete(k);
      }
      return next;
    });
  };
  const setAllExportItems = (select: boolean) => {
    if (!exportPreview) return;
    setExportSelected(select ? new Set(exportPreview.items.map((it) => itemKey(it.scope, it.key))) : new Set());
  };

  // 读文件 → 预览 → 初始化决策(全 overwrite) + 逐项全选。点击与拖入共享。
  const loadPreview = async (p: string) => {
    setError("");
    setReport(null);
    setPreview(null);
    try {
      const prev = await importExportApi.readPreview(p);
      setImportPath(p);
      setPreview(prev);
      // 默认全部 overwrite。
      const map = new Map<string, ImportDecision>();
      for (const c of prev.conflicts) {
        map.set(decisionKey(c.scope, c.key), { kind: "overwrite" });
      }
      setDecisions(map);
      // 逐项默认全选（含 skills scope：用户主动定方案 C 删 filter，npx 误删防御由后端守卫兜底）。
      setSelectedItems(
        new Set(prev.items.map((it) => itemKey(it.scope, it.key))),
      );
    } catch (e) {
      setError(String(e));
    }
  };

  const handlePickFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "AiDog Export", extensions: ["aidogx"] }],
      });
      if (!selected || typeof selected !== "string") return;
      await loadPreview(selected as string);
    } catch (e) {
      setError(String(e));
    }
  };

  const decisionKey = (scope: string, key: string) => `${scope}::${key}`;
  const itemKey = (scope: string, key: string) => `${scope}::${key}`;

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

  // ── 逐项勾选操作 ──
  const toggleItem = (it: ImportItem) => {
    const k = itemKey(it.scope, it.key);
    setSelectedItems((prev) => {
      const next = new Set(prev);
      if (next.has(k)) next.delete(k);
      else next.add(k);
      return next;
    });
  };

  // 组级全选 / 反选（传入该组全部条目 + 方向）。
  const setGroupItems = (groupItems: ImportItem[], select: boolean) => {
    setSelectedItems((prev) => {
      const next = new Set(prev);
      for (const it of groupItems) {
        const k = itemKey(it.scope, it.key);
        if (select) next.add(k);
        else next.delete(k);
      }
      return next;
    });
  };

  // 全局全选 / 反选。
  const setAllItems = (select: boolean) => {
    if (!preview) return;
    setSelectedItems(select ? new Set(preview.items.map((it) => itemKey(it.scope, it.key))) : new Set());
  };

  // loadPreview 引用同步（拖入回调读 ref，effect 只订阅一次）。
  loadPreviewRef.current = loadPreview;

  // 原生文件拖入：Tauri onDragDropEvent（HTML5 onDrop/onDragOver 在 macOS WKWebView drop 不触发）。
  // enter/over 高亮；drop 取首个 .aidogx 路径走 loadPreview；leave/cancel 清高亮。卸载 unlisten。
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const { type } = event.payload;
        if (type === "enter" || type === "over") {
          const paths = (event.payload as { paths?: string[] }).paths ?? [];
          // 仅当拖入含 .aidogx 时高亮（拖其它文件不误导）。
          if (type === "enter") setDragActive(paths.some((p) => p.toLowerCase().endsWith(".aidogx")));
        } else if (type === "drop") {
          setDragActive(false);
          const paths = (event.payload as { paths?: string[] }).paths ?? [];
          const target = paths.find((p) => p.toLowerCase().endsWith(".aidogx"));
          if (target) {
            void loadPreviewRef.current(target);
          } else if (paths.length > 0) {
            setError(t("importExport.error.notAidogx", "请拖入 .aidogx 备份文件"));
          }
        } else {
          // leave / cancel
          setDragActive(false);
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [t]);

  const handleApply = async () => {
    if (!importPath) return;
    setError("");
    setImporting(true);
    try {
      const ds: ConflictDecision[] = Array.from(decisions.entries()).map(([k, d]) => {
        const [scope, key] = k.split("::");
        return { scope, key, decision: d };
      });
      // 选中条目白名单：从 preview.items 重建 (scope, key) 对（避免 split "::" 在 g::p 上歧义）。
      const selection: [string, string][] = (preview?.items ?? [])
        .filter((it) => selectedItems.has(itemKey(it.scope, it.key)))
        .map((it) => [it.scope, it.key]);
      const r = await importExportApi.apply(importPath, ds, selection);
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
  const exportSelCount = exportSelected.size;

  // 按菜单组聚合 scope 卡片（platform/group/group_platform 三 scope 各自独立一张卡）。
  const scopeCardGroups: { gid: MenuGroupId; scopeIds: ImportExportScope[] }[] = [];
  for (const s of ALL_SCOPES) {
    const gid = SCOPE_MENU_GROUP[s.id] ?? "system";
    let g = scopeCardGroups.find((x) => x.gid === gid);
    if (!g) {
      g = { gid, scopeIds: [] };
      scopeCardGroups.push(g);
    }
    g.scopeIds.push(s.id);
  }
  // 切换某菜单组的全部 scope（任一未选则全开，否则全关）。
  const toggleGroupScopes = (scopeIds: ImportExportScope[]) => {
    setExportPreview(null);
    setScopes((prev) => {
      const next = new Set(prev);
      const allOn = scopeIds.every((id) => next.has(id));
      for (const id of scopeIds) {
        if (allOn) next.delete(id);
        else next.add(id);
      }
      return next;
    });
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 24, width: "100%" }}>
      {/* ── 导出区 ── */}
      <section className="glass" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 16 }}>
        <SectionHeader icon="folder" title={t("importExport.exportTitle", "导出")} desc={t("importExport.exportDesc", "勾选要导出的内容，加密为单文件 .aidogx（密钥隐藏在文件内，人眼无法识别）。")} />

        {/* scope 区头：标题 + 全选/反选 + 选中计数 */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>
              {t("importExport.scopeHeader", "导出范围")}
            </span>
            <TextButton onClick={() => { selectAll(); setExportPreview(null); }}>{t("importExport.selectAll", "全选")}</TextButton>
            <TextButton onClick={() => { deselectAll(); setExportPreview(null); }}>{t("importExport.deselectAll", "反选")}</TextButton>
          </div>
          <StatChip
            value={`${selectedCount} / ${ALL_SCOPES.length}`}
            label={t("importExport.selectedLabel", "已选")}
            level={(selectedCount > 0 ? "success" : "neutral") as ColorLevel}
          />
        </div>

        {/* scope 卡片网格：按菜单组聚合（platform/group/group_platform 各自一张卡）。 */}
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))", gap: 10 }}>
          {scopeCardGroups.map((g) => {
            // 单 scope 组沿用该 scope 的 label/desc/icon；多 scope 组用菜单组标题 + 子 scope 列表描述。
            const multi = g.scopeIds.length > 1;
            const allOn = g.scopeIds.every((id) => scopes.has(id));
            const someOn = g.scopeIds.some((id) => scopes.has(id)) && !allOn;
            if (!multi) {
              const s = ALL_SCOPES.find((x) => x.id === g.scopeIds[0])!;
              return (
                <ScopeCard
                  key={g.gid}
                  icon={s.icon}
                  label={t(s.labelKey, s.defaultLabel)}
                  desc={t(s.descKey, s.defaultDesc)}
                  selected={allOn}
                  onToggle={() => toggleGroupScopes(g.scopeIds)}
                />
              );
            }
            const subLabels = g.scopeIds.map((id) => scopeLabel(t, id)).join(" · ");
            return (
              <ScopeCard
                key={g.gid}
                icon={MENU_GROUP_ICON[g.gid] ?? "folder"}
                label={menuGroupLabel(t, g.gid)}
                desc={subLabels}
                selected={allOn}
                indeterminate={someOn}
                onToggle={() => toggleGroupScopes(g.scopeIds)}
              />
            );
          })}
        </div>

        {/* 逐项预览：scope 选定后 debounce 自动拉全量条目勾选（默认全选）。 */}
        {previewing && (
          <div style={{ fontSize: 13, color: "var(--text-tertiary)", display: "flex", alignItems: "center", gap: 8 }}>
            <span
              style={{
                width: 12,
                height: 12,
                borderRadius: "50%",
                border: "2px solid var(--border)",
                borderTopColor: "var(--accent)",
                animation: "spin 0.9s linear infinite",
                display: "inline-block",
              }}
            />
            {t("importExport.loadingPreview", "加载中…")}
          </div>
        )}
        {!previewing && exportPreview && exportPreview.items.length > 0 && (
          <ItemSelector
            items={exportPreview.items}
            selected={exportSelected}
            onToggle={toggleExportItem}
            onGroupSet={setExportGroupItems}
            onAllSet={setAllExportItems}
            itemKey={itemKey}
            groupOf={menuGroupOf}
            groupLabel={(g) => menuGroupLabel(t, g)}
            groupIcon={(g) => MENU_GROUP_ICON[g] ?? "folder"}
            t={t}
          />
        )}
        {!previewing && exportPreview && exportPreview.items.length === 0 && (
          <div style={{ fontSize: 13, color: "var(--text-tertiary)" }}>
            {t("importExport.exportEmpty", "所选范围暂无可导出条目")}
          </div>
        )}

        <Button variant="default"
          onClick={handleExport}
          disabled={exporting || previewing || selectedCount === 0 || (exportPreview !== null && exportSelCount === 0) || (exportPreview === null && scopes.size > 0)}
          
          style={{ alignSelf: "flex-end" }}
        >
          {exporting
            ? t("importExport.exporting", "导出中…")
            : previewing
              ? t("importExport.loadingPreview", "加载中…")
              : exportPreview
                ? t("importExport.exportN", "导出 {{n}} 项", { n: exportSelCount })
                : t("importExport.exportBtn", "导出")}
        </Button>

        {exportMsg && <SuccessPathCard message={exportMsg} />}
      </section>

      {/* ── 导入区 ── */}
      <section className="glass" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 16 }}>
        <SectionHeader icon="worktree" title={t("importExport.importTitle", "导入")} desc={t("importExport.importDesc", "选择 .aidogx 文件，程序自动解密。冲突项逐条决策；Skill 自动安装并恢复原启用状态。")} />

        {/* 导入入口：点击选文件 或 原生拖入 .aidogx（dragActive 高亮）。 */}
        <DropZone
          onClick={handlePickFile}
          active={dragActive}
          title={t("importExport.pickFile", "选择 .aidogx 文件")}
          hint={t("importExport.dropHint", "点击选择，或将 .aidogx 拖到此处 · 自动解密 · Skill 自动安装")}
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

            {/* 逐项勾选：按 scope 分组、可折叠、默认全选；未勾项不导入。 */}
            {preview.items.length > 0 && (
              <ItemSelector
                items={preview.items}
                selected={selectedItems}
                onToggle={toggleItem}
                onGroupSet={setGroupItems}
                onAllSet={setAllItems}
                itemKey={itemKey}
                groupOf={menuGroupOf}
                groupLabel={(g) => menuGroupLabel(t, g)}
                groupIcon={(g) => MENU_GROUP_ICON[g] ?? "folder"}
                t={t}
              />
            )}

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

            <Button variant="default"
              onClick={handleApply}
              disabled={importing || selectedItems.size === 0}
              
              style={{ alignSelf: "flex-end" }}
            >
              {importing
                ? t("importExport.applying", "导入中…")
                : t("importExport.applyN", "应用导入（{{n}} 项）", { n: selectedItems.size })}
            </Button>
          </div>
        )}

        {report && <ReportView report={report} t={t} scopeLabel={(s: string) => scopeLabel(t, s)} />}
      </section>

      {/* ── 从 cc-switch 导入区（异源单向，仅 claude + codex provider）── */}
      <CcSwitchImportSection onReport={(r) => { setReport(r); reloadFromDB().catch(() => {}); }} />

      {/* ── 从 sub2api 导入区（异源单向，账号数据 JSON 双入口）── */}
      <Sub2ApiImportSection onReport={(r) => { setReport(r); reloadFromDB().catch(() => {}); }} />

      <ScheduledBackupSection />

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
