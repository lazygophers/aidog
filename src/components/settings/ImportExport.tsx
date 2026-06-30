// ─── 导入导出子系统 UI ────────────────────────────────────
// AppSettings「导入导出」tab。导出勾选范围 → 加密单文件 .aidogx；
// 导入选文件 → 冲突预览 → 逐项决策 → 应用。消费 services/api.ts importExportApi 契约。
//
// 视觉重设（06-14-import-export-redesign）：scope 卡片化 + 拖放式导入入口 +
// 冲突分段控件 + report 语义色卡片。功能/数据流 100% 不变，纯展示层。
// 全部样式走主题令牌（--radius-*/--shadow-*/--transition/--accent*/--border*/--bg-*/--text-*/--color-*），
// 随 9 style × 12 palette 自适应；无硬编码主题色、无 emoji。

import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { save, open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  importExportApi,
  backupApi,
  type BackupSettings,
  type BackupResult,
  type ImportExportScope,
  type ConflictItem,
  type ConflictDecision,
  type ImportDecision,
  type ImportItem,
  type ImportPreview,
  type ImportReport,
} from "../../services/api";
import { useApp } from "../../context/AppContext";
import { SectionIcon } from "./editors";
import { IconCheck } from "../icons";
import { CcSwitchImportSection } from "./CcSwitchImport";
import { Sub2ApiImportSection } from "./Sub2ApiImport";
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
  { id: "model_price", labelKey: "importExport.scope.modelPrice", defaultLabel: "模型价格", icon: "pricing", descKey: "importExport.scopeDesc.modelPrice", defaultDesc: "自定义模型定价" },
  { id: "mcp", labelKey: "importExport.scope.mcp", defaultLabel: "MCP 服务器", icon: "mcp", descKey: "importExport.scopeDesc.mcp", defaultDesc: "MCP 服务器配置 + 启用状态" },
  { id: "middleware", labelKey: "importExport.scope.middleware", defaultLabel: "中间件规则", icon: "rules", descKey: "importExport.scopeDesc.middleware", defaultDesc: "请求/响应中间件规则" },
  { id: "skills", labelKey: "importExport.scope.skills", defaultLabel: "Skills", icon: "plugins", descKey: "importExport.scopeDesc.skills", defaultDesc: "npx 安装 + 启用状态" },
];

const SCOPE_ICON: Record<string, string> = Object.fromEntries(ALL_SCOPES.map((s) => [s.id, s.icon]));

function scopeLabel(t: TFunction, scope: string): string {
  const entry = ALL_SCOPES.find((s) => s.id === scope);
  if (!entry) return scope;
  return t(entry.labelKey, entry.defaultLabel);
}

// ── IA 菜单分组（按侧栏菜单组织）──
// 导出/导入条目不再按裸 scope 平铺，而是按菜单组聚合呈现：
//   - 平台模块合并 platform + group + group_platform 三 scope 为一组
//   - setting scope 的条目按其 key 前缀（settingScope）二次归类到对应菜单组
//     （key 形如 `<settingScope>:<settingKey>`，见后端 build_items setting 约定）
type MenuGroupId = "platform" | "extension" | "rules" | "scheduling" | "uiPref" | "system";

const MENU_GROUPS: { id: MenuGroupId; labelKey: string; defaultLabel: string; icon: string }[] = [
  { id: "platform", labelKey: "importExport.menuGroup.platform", defaultLabel: "平台", icon: "network" },
  { id: "extension", labelKey: "importExport.menuGroup.extension", defaultLabel: "扩展", icon: "plugins" },
  { id: "rules", labelKey: "importExport.menuGroup.rules", defaultLabel: "规则", icon: "rules" },
  { id: "scheduling", labelKey: "importExport.menuGroup.scheduling", defaultLabel: "调度", icon: "worktree" },
  { id: "uiPref", labelKey: "importExport.menuGroup.uiPref", defaultLabel: "界面偏好", icon: "bolt" },
  { id: "system", labelKey: "importExport.menuGroup.system", defaultLabel: "系统", icon: "settings" },
];

// scope → 菜单组（setting 例外：按 settingScope 子归类，见 SETTING_SCOPE_GROUP）。
const SCOPE_MENU_GROUP: Record<string, MenuGroupId> = {
  platform: "platform",
  group: "platform",
  group_platform: "platform",
  skills: "extension",
  mcp: "extension",
  middleware: "rules",
  codex: "system",
  claude_code: "system",
  model_price: "system",
  setting: "system", // 兜底；细分见 SETTING_SCOPE_GROUP
};

// setting item 的 settingScope（key 前缀）→ 菜单组。未列出走 system（全局设置）。
const SETTING_SCOPE_GROUP: Record<string, MenuGroupId> = {
  scheduling: "scheduling",
  tray: "uiPref",
  popover: "uiPref",
  notification: "uiPref",
};

// setting item 的 `<scope>:<key>` → i18n label key。
// 后端 build_items 对 setting scope 存裸 key（`app:theme` 等稳定标识），前端展示层做本地化映射；
// 未命中（新增/未知 setting key）→ 回退裸 key（不崩不空）。
// 全集来源：grep src-tauri set_setting/get_setting 各 (scope,key) 调用点。
const SETTING_KEY_LABEL: Record<string, string> = {
  "app:theme": "importExport.settingLabel.app_theme",
  "app:locale": "importExport.settingLabel.app_locale",
  "app:logging": "importExport.settingLabel.app_logging",
  "app:script_executor": "importExport.settingLabel.app_script_executor",
  "proxy:settings": "importExport.settingLabel.proxy_settings",
  "proxy:proxy_client": "importExport.settingLabel.proxy_client",
  "proxy:timeout": "importExport.settingLabel.proxy_timeout",
  "proxy:logging": "importExport.settingLabel.proxy_logging",
  "notification:settings": "importExport.settingLabel.notification_settings",
  "middleware:settings": "importExport.settingLabel.middleware_settings",
  "scheduling:settings": "importExport.settingLabel.scheduling_settings",
  "stats:settings": "importExport.settingLabel.stats_settings",
  "stats:agg_rebuild_v1": "importExport.settingLabel.stats_agg_rebuild_v1",
  "stats:agg_count_tokens_excluded_v1": "importExport.settingLabel.stats_agg_count_tokens_excluded_v1",
  "pricing:sync": "importExport.settingLabel.pricing_sync",
  "tray:config": "importExport.settingLabel.tray_config",
  "popover:config": "importExport.settingLabel.popover_config",
  "global:claude_code": "importExport.settingLabel.global_claude_code",
  "global:coding_tools_settings": "importExport.settingLabel.global_coding_tools_settings",
  "backup:settings": "importExport.settingLabel.backup_settings",
  // ponytail: 内部迁移标记，迁移完成后必落库（db/maintenance.rs），导出高频可见，故补 label 而非裸 key 兜底。
  "db:compact_migrated_v1": "importExport.settingLabel.db_compact_migrated_v1",
};

// 取某 setting item 的本地化 label；非 setting scope 或未命中 → 返回 null（调用方回退原 label）。
function settingLabelKey(item: ImportItem): string | null {
  if (item.scope !== "setting") return null;
  return SETTING_KEY_LABEL[item.key] ?? null;
}

// 取某 item 的菜单组 id。setting scope 按 key 前缀细分。
function menuGroupOf(item: ImportItem): MenuGroupId {
  if (item.scope === "setting") {
    const settingScope = item.key.split(":")[0];
    return SETTING_SCOPE_GROUP[settingScope] ?? "system";
  }
  return SCOPE_MENU_GROUP[item.scope] ?? "system";
}

function menuGroupLabel(t: TFunction, id: string): string {
  const g = MENU_GROUPS.find((x) => x.id === id);
  if (!g) return id;
  return t(g.labelKey, g.defaultLabel);
}

const MENU_GROUP_ICON: Record<string, string> = Object.fromEntries(MENU_GROUPS.map((g) => [g.id, g.icon]));

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

  // 步骤1：勾 scope 后 debounce(~300ms) 自动拉全量条目预览，默认全选（skills 例外，需手动勾选）。
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
      setExportSelected(
        new Set(
          prev.items
            .filter((it) => it.scope !== "skills")
            .map((it) => itemKey(it.scope, it.key)),
        ),
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
      // 逐项默认全选（**排除 skills scope**：skills 导入可能触发 npx 操作，强制用户显式勾选
      // skills 才导入，防导入 .aidogx 默认全选误触 — 见 F2 导入误删修复）。
      setSelectedItems(
        new Set(
          prev.items
            .filter((it) => it.scope !== "skills")
            .map((it) => itemKey(it.scope, it.key)),
        ),
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

  // 按菜单组聚合 scope 卡片（platform 三 scope 合并为「平台」一张卡）。
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

        {/* scope 卡片网格：按菜单组聚合（平台三 scope 合并为一张卡）。 */}
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

        {/* 逐项预览：scope 选定后 debounce 自动拉全量条目勾选（默认全选，skills 需手动）。 */}
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

        <button
          onClick={handleExport}
          disabled={exporting || previewing || selectedCount === 0 || (exportPreview !== null && exportSelCount === 0) || (exportPreview === null && scopes.size > 0)}
          className="btn btn-primary"
          style={{ alignSelf: "flex-end" }}
        >
          {exporting
            ? t("importExport.exporting", "导出中…")
            : previewing
              ? t("importExport.loadingPreview", "加载中…")
              : exportPreview
                ? t("importExport.exportN", "导出 {{n}} 项", { n: exportSelCount })
                : t("importExport.exportBtn", "导出")}
        </button>

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

            <button
              onClick={handleApply}
              disabled={importing || selectedItems.size === 0}
              className="btn btn-primary"
              style={{ alignSelf: "flex-end" }}
            >
              {importing
                ? t("importExport.applying", "导入中…")
                : t("importExport.applyN", "应用导入（{{n}} 项）", { n: selectedItems.size })}
            </button>
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
  indeterminate,
  onToggle,
}: {
  icon: string;
  label: string;
  desc: string;
  selected: boolean;
  indeterminate?: boolean;
  onToggle: () => void;
}) {
  const [hover, setHover] = useState(false);
  const on = selected || indeterminate;
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
        border: `1px solid ${on ? "var(--accent)" : "var(--border)"}`,
        background: on ? "var(--accent-subtle)" : "transparent",
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
          border: `1px solid ${on ? "var(--accent)" : "var(--border)"}`,
          background: on ? "var(--accent)" : "transparent",
          transition: "var(--transition)",
        }}
      >
        {selected && !indeterminate && <IconCheck size={12} color="#fff" strokeWidth={2.5} />}
        {indeterminate && <span style={{ width: 8, height: 2, background: "#fff", borderRadius: 1 }} />}
      </span>

      <SectionIcon name={icon} size={20} style={{ color: on ? "var(--accent)" : "var(--text-secondary)" }} />
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

/** 导入入口（虚线 glass 区）：点击触发 open；原生拖入时 active=true 高亮。 */
function DropZone({ onClick, active, title, hint }: { onClick: () => void; active: boolean; title: string; hint: string }) {
  const [hover, setHover] = useState(false);
  const lit = hover || active;
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
        border: `1.5px dashed ${lit ? "var(--accent)" : "var(--border)"}`,
        background: lit ? "var(--accent-subtle)" : "var(--bg-glass)",
        cursor: "pointer",
        transition: "var(--transition)",
        transform: active ? "scale(1.01)" : "none",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 8,
        textAlign: "center",
      }}
    >
      <SectionIcon name="file" size={28} style={{ color: lit ? "var(--accent)" : "var(--text-secondary)" }} />
      <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>{title}</div>
      <div style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{hint}</div>
    </div>
  );
}

/** 小复选框（受控 √ 方块，accent 选中态）。 */
function CheckBox({ checked, indeterminate }: { checked: boolean; indeterminate?: boolean }) {
  const on = checked || indeterminate;
  return (
    <span
      style={{
        width: 16,
        height: 16,
        flexShrink: 0,
        borderRadius: "var(--radius-sm)",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        border: `1px solid ${on ? "var(--accent)" : "var(--border)"}`,
        background: on ? "var(--accent)" : "transparent",
        transition: "var(--transition)",
      }}
    >
      {checked && !indeterminate && <IconCheck size={11} color="#fff" strokeWidth={3} />}
      {indeterminate && <span style={{ width: 8, height: 2, background: "#fff", borderRadius: 1 }} />}
    </span>
  );
}

/** 折叠箭头（▸ 旋转，open 时 90°）。 */
function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      width={12}
      height={12}
      viewBox="0 0 24 24"
      fill="none"
      stroke="var(--text-tertiary)"
      strokeWidth={2.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ transform: open ? "rotate(90deg)" : "none", transition: "var(--transition)", flexShrink: 0 }}
    >
      <polyline points="9 18 15 12 9 6" />
    </svg>
  );
}

/**
 * 逐项勾选器：全局头（全选/反选 + 计数）+ 按菜单组分组可折叠，每组内逐项 checkbox。
 * 导出/导入共用。分组逻辑由 groupOf 注入（默认按菜单组聚合，平台三 scope 合并、setting 子归类）。
 */
function ItemSelector({
  items,
  selected,
  onToggle,
  onGroupSet,
  onAllSet,
  itemKey,
  groupOf,
  groupLabel,
  groupIcon,
  t,
}: {
  items: ImportItem[];
  selected: Set<string>;
  onToggle: (it: ImportItem) => void;
  /** 组级全选/反选：传入组内全部 items + select 方向。 */
  onGroupSet: (groupItems: ImportItem[], select: boolean) => void;
  onAllSet: (select: boolean) => void;
  itemKey: (scope: string, key: string) => string;
  /** item → 分组 id（默认菜单组）。 */
  groupOf: (it: ImportItem) => string;
  groupLabel: (groupId: string) => string;
  groupIcon: (groupId: string) => string;
  t: TFunction;
}) {
  // 按菜单组分组（保持出现顺序）。
  const groups: { gid: string; items: ImportItem[] }[] = [];
  for (const it of items) {
    const gid = groupOf(it);
    let g = groups.find((x) => x.gid === gid);
    if (!g) {
      g = { gid, items: [] };
      groups.push(g);
    }
    g.items.push(it);
  }
  // 默认展开（条目多时用户可手动折叠）。
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const toggleCollapse = (scope: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(scope)) next.delete(scope);
      else next.add(scope);
      return next;
    });

  const total = items.length;
  const selCount = items.filter((it) => selected.has(itemKey(it.scope, it.key))).length;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
        <strong style={{ fontSize: 14, color: "var(--text-primary)" }}>
          {t("importExport.selectItems", "选择导入项")}
        </strong>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <TextButton onClick={() => onAllSet(true)}>{t("importExport.selectAll", "全选")}</TextButton>
          <TextButton onClick={() => onAllSet(false)}>{t("importExport.deselectAll", "反选")}</TextButton>
          <StatChip value={`${selCount} / ${total}`} label={t("importExport.selectedLabel", "已选")} level={(selCount > 0 ? "success" : "neutral") as ColorLevel} />
        </div>
      </div>

      {groups.map((g) => {
        const open = !collapsed.has(g.gid);
        const gSel = g.items.filter((it) => selected.has(itemKey(it.scope, it.key))).length;
        const allOn = gSel === g.items.length;
        const someOn = gSel > 0 && !allOn;
        // skills 条目落在「扩展」组；只要本组含 skills 且全未选则提示。
        const hasSkills = g.items.some((it) => it.scope === "skills");
        const skillsSel = g.items.filter((it) => it.scope === "skills" && selected.has(itemKey(it.scope, it.key))).length;
        return (
          <div
            key={g.gid}
            className="glass-surface"
            style={{ borderRadius: "var(--radius-md)", border: "1px solid var(--border)", overflow: "hidden" }}
          >
            {/* 组头：折叠箭头 + 组复选框（全选/反选本组）+ 标题 + 计数 */}
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                padding: "10px 12px",
                cursor: "pointer",
                background: "var(--bg-glass)",
              }}
              onClick={() => toggleCollapse(g.gid)}
            >
              <Chevron open={open} />
              <span
                onClick={(e) => {
                  e.stopPropagation();
                  onGroupSet(g.items, !allOn);
                }}
                style={{ display: "inline-flex" }}
              >
                <CheckBox checked={allOn} indeterminate={someOn} />
              </span>
              <SectionIcon name={groupIcon(g.gid)} size={14} style={{ color: "var(--text-secondary)" }} />
              <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>{groupLabel(g.gid)}</span>
              {/* F2: skills 条目默认不勾选（防导入误删），显眼提示告知用户需手动勾选 */}
              {hasSkills && skillsSel === 0 && (
                <span
                  style={{
                    fontSize: 11,
                    color: "var(--color-warning)",
                    background: "var(--color-warning-bg)",
                    padding: "2px 6px",
                    borderRadius: "var(--radius-sm)",
                    marginLeft: 4,
                  }}
                >
                  {t("importExport.skillsScopeHint", "Skills 默认不导入，需手动勾选")}
                </span>
              )}
              <span style={{ fontSize: 12, color: "var(--text-tertiary)", marginLeft: "auto" }}>
                {gSel} / {g.items.length}
              </span>
            </div>

            {open && (
              <div style={{ display: "flex", flexDirection: "column" }}>
                {g.items.map((it) => {
                  const k = itemKey(it.scope, it.key);
                  const on = selected.has(k);
                  // setting scope 的 label 后端存裸 key（`app:theme` 等稳定标识），前端按 (scope:key) 映射本地化；
                  // 未命中映射（新增/未知 setting key）→ 回退裸 key（不崩不空）。
                  const settingLk = settingLabelKey(it);
                  const displayLabel = settingLk ? t(settingLk, it.label) : it.label;
                  return (
                    <div
                      key={k}
                      onClick={() => onToggle(it)}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 10,
                        padding: "8px 12px 8px 34px",
                        cursor: "pointer",
                        borderTop: "1px solid var(--border)",
                        transition: "var(--transition)",
                      }}
                    >
                      <CheckBox checked={on} />
                      <SectionIcon name={SCOPE_ICON[it.scope] ?? "folder"} size={12} style={{ color: "var(--text-tertiary)", flexShrink: 0 }} />
                      <span style={{ fontSize: 13, color: "var(--text-primary)", wordBreak: "break-all", flex: 1 }}>{displayLabel}</span>
                      {it.conflict && (
                        <StatChip value={t("importExport.conflictTag", "冲突")} label="" level={"warning" as ColorLevel} />
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      })}
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

// ─── 定时备份 section ──────────────────────────────────────
// 消费 services/api.ts backupApi：开关 / 间隔(自由小时+快捷预设) / 保留天数 /
// 状态展示(上次/下次/错误) / 立即备份一次。复用 import_export 加密容器 (.aidogx)，
// 落盘 ~/.aidog/backups/，超期自动清理。后端常驻 loop 实时读 settings 即时生效。

const INTERVAL_PRESETS: { labelKey: string; defaultLabel: string; hours: number }[] = [
  { labelKey: "settings.backup.preset1h", defaultLabel: "1h", hours: 1 },
  { labelKey: "settings.backup.preset6h", defaultLabel: "6h", hours: 6 },
  { labelKey: "settings.backup.preset12h", defaultLabel: "12h", hours: 12 },
  { labelKey: "settings.backup.presetDaily", defaultLabel: "每天", hours: 24 },
  { labelKey: "settings.backup.presetWeekly", defaultLabel: "每周", hours: 168 },
];

function formatBackupTime(ms: number, t: TFunction): string {
  if (!ms) return t("settings.backup.never", "从未");
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

function ScheduledBackupSection() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<BackupSettings | null>(null);
  const [running, setRunning] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [lastResultPath, setLastResultPath] = useState<string | null>(null);

  // 初次加载。
  useEffect(() => {
    backupApi.get().then(setSettings).catch(() => setSettings(null));
  }, []);

  if (!settings) {
    return (
      <section className="glass" style={{ padding: 20 }}>
        <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>…</div>
      </section>
    );
  }

  const patch = async (next: Partial<BackupSettings>) => {
    const merged = { ...settings, ...next };
    setSettings(merged); // 乐观更新
    try {
      const saved = await backupApi.set(merged);
      setSettings(saved);
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
    }
  };

  const runNow = async () => {
    setRunning(true);
    setMsg(null);
    try {
      const r: BackupResult = await backupApi.runNow();
      if (r.ok) {
        setMsg({ ok: true, text: t("settings.backup.success", "备份成功") });
        setLastResultPath(r.path ?? null);
        // 刷新 last_backup_at。
        const fresh = await backupApi.get();
        setSettings(fresh);
      } else {
        setMsg({ ok: false, text: r.error ?? t("settings.backup.failed", "备份失败") });
      }
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
    } finally {
      setRunning(false);
    }
  };

  const nextAt = settings.enabled && settings.last_backup_at
    ? settings.last_backup_at + settings.interval_hours * 3600 * 1000
    : 0;

  return (
    <section className="glass" style={{ padding: 20, display: "flex", flexDirection: "column", gap: 16 }}>
      <SectionHeader
        icon="backup"
        title={t("settings.backup.title", "定时备份")}
        desc={t("settings.backup.desc", "按设定间隔自动导出全部数据为加密 .aidogx，落盘 ~/.aidog/backups/，超期自动清理。")}
      />

      {/* 总开关 */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <span style={{ fontSize: 13, color: "var(--text-primary)", fontWeight: 500 }}>
          {t("settings.backup.enable", "启用定时备份")}
        </span>
        <label className="toggle-wrap" style={{ cursor: "pointer", display: "flex", alignItems: "center" }}>
          <input
            type="checkbox"
            checked={settings.enabled}
            onChange={(e) => patch({ enabled: e.target.checked })}
            style={{ display: "none" }}
          />
          <span className={`toggle ${settings.enabled ? "active" : ""}`} />
        </label>
      </div>

      {settings.enabled && (
        <>
          {/* 间隔 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {t("settings.backup.interval", "备份间隔（小时）")}
            </span>
            <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
              <input
                type="number"
                min={1}
                value={settings.interval_hours}
                onChange={(e) => {
                  const v = Math.max(1, Math.floor(Number(e.target.value) || 1));
                  patch({ interval_hours: v });
                }}
                style={{
                  width: 90, padding: "6px 10px", fontSize: 13,
                  borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
                  background: "var(--bg-input)", color: "var(--text-primary)",
                }}
              />
              <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{t("settings.backup.hours", "小时")}</span>
              {INTERVAL_PRESETS.map((p) => (
                <button
                  key={p.hours}
                  onClick={() => patch({ interval_hours: p.hours })}
                  className="ie-chip"
                  style={{
                    padding: "4px 10px", fontSize: 12, cursor: "pointer",
                    borderRadius: "var(--radius-md)",
                    border: settings.interval_hours === p.hours
                      ? "1px solid var(--accent)"
                      : "1px solid var(--border-default)",
                    background: settings.interval_hours === p.hours
                      ? "var(--accent-subtle)"
                      : "transparent",
                    color: "var(--text-primary)",
                  }}
                >
                  {t(p.labelKey, p.defaultLabel)}
                </button>
              ))}
            </div>
          </div>

          {/* 保留天数 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {t("settings.backup.retention", "保留天数")}
            </span>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <input
                type="number"
                min={1}
                max={90}
                value={settings.retention_days}
                onChange={(e) => {
                  const v = Math.min(90, Math.max(1, Math.floor(Number(e.target.value) || 7)));
                  patch({ retention_days: v });
                }}
                style={{
                  width: 90, padding: "6px 10px", fontSize: 13,
                  borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
                  background: "var(--bg-input)", color: "var(--text-primary)",
                }}
              />
              <span style={{ fontSize: 12, color: "var(--text-tertiary)" }}>{t("settings.backup.days", "天（1-90）")}</span>
            </div>
          </div>

          {/* 状态 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4, padding: "10px 12px", borderRadius: "var(--radius-md)", background: "var(--bg-subtle)", fontSize: 12, color: "var(--text-secondary)" }}>
            <div>{t("settings.backup.lastBackup", "上次备份")}: <span style={{ color: "var(--text-primary)" }}>{formatBackupTime(settings.last_backup_at, t)}</span></div>
            {nextAt > 0 && (
              <div>{t("settings.backup.nextBackup", "下次预计")}: <span style={{ color: "var(--text-primary)" }}>{formatBackupTime(nextAt, t)}</span></div>
            )}
            <div>{t("settings.backup.location", "备份位置")}: <code style={{ fontSize: 11, color: "var(--text-tertiary)" }}>~/.aidog/backups/</code></div>
            {settings.last_backup_error && (
              <div style={{ color: "var(--color-danger)" }}>
                {t("settings.backup.lastError", "上次错误")}: {settings.last_backup_error}
              </div>
            )}
          </div>

          {/* 立即备份 */}
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <button
              onClick={runNow}
              disabled={running}
              style={{
                padding: "7px 16px", fontSize: 13, cursor: running ? "not-allowed" : "pointer",
                borderRadius: "var(--radius-md)", border: "1px solid var(--accent)",
                background: "var(--accent)", color: "var(--text-on-accent, #fff)",
                opacity: running ? 0.6 : 1,
              }}
            >
              {running ? t("settings.backup.running", "备份中…") : t("settings.backup.runNow", "立即备份一次")}
            </button>
            {lastResultPath && (
              <button
                onClick={() => { revealItemInDir(lastResultPath).catch(() => {}); }}
                style={{
                  padding: "7px 14px", fontSize: 12, cursor: "pointer",
                  borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
                  background: "transparent", color: "var(--text-secondary)",
                }}
              >
                {t("settings.backup.reveal", "在文件夹显示")}
              </button>
            )}
          </div>
        </>
      )}

      {msg && (
        <div style={{
          padding: "8px 12px", fontSize: 12, borderRadius: "var(--radius-md)",
          color: msg.ok ? "var(--color-success)" : "var(--color-danger)",
          background: msg.ok ? "var(--color-success-bg)" : "var(--color-danger-bg)",
        }}>
          {msg.text}
        </div>
      )}
    </section>
  );
}
