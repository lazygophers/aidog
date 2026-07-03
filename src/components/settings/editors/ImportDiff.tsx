// ─── Import Diff (diff tree + modal) ───────────────────────
// Extracted verbatim from editors.tsx (arch-redesign phase 3).

import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { getManagedPaths } from "../../../services/api";
import { F, S } from "./tokens";
import { Toggle } from "./_shared";

/**
 * One node in the import diff tree. `path` is a dot-path (`env.FOO`, `permissions.allow`).
 * Top-level keys whose value is a plain object expand one level into `children`
 * (MVP: depth 1 — deeper nesting stays as a single leaf, see TODO below).
 */
export interface DiffNode {
  path: string;
  label: string;       // display label (last path segment)
  current: any;
  incoming: any;
  children?: DiffNode[];
}

// ponytail: D7 刻意不合 — 此处是松版（any 入参 + 仅 typeof object && !null && !Array），
// editors/diff 渲染走表单配置，结构来自 JSON.parse 不会出现 Map/Date/类实例，宽松判定即可。
// 另有一份严版在 utils/deepMerge.ts（unknown 入参 + toString 锁 [object Object]），
// 语义不同，禁合并。见 arch-redesign D7 决策。
export const isPlainObject = (v: any): v is Record<string, any> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

/** Collect all leaf paths under a node (the node itself if it has no children). */
function collectLeafPaths(node: DiffNode, out: string[]): void {
  if (node.children && node.children.length > 0) {
    node.children.forEach(c => collectLeafPaths(c, out));
  } else {
    out.push(node.path);
  }
}

/**
 * Read the aidog-managed dot-path set from the aidog internal DB
 * (command `get_managed_paths` → `setting` table scope=`claude_default_group`/
 * key=`managed_paths`, written by the Rust side `write_default_claude_settings`).
 * These are the exact leaf paths aidog's default group injected into
 * ~/.claude/settings.json (env routing vars, statusLine, hooks, aidog's own
 * enabledPlugins/mcpServers entries, …). The import diff excludes these so only
 * user-added fields surface.
 *
 * Missing / malformed (no default group, or pre-marker legacy) → empty set →
 * diff degrades to its prior behavior (zero regression).
 *
 * Marker used to live in settings.json `_aidog_managed` key; moved to DB to keep
 * the user's settings.json clean (CC ignored it but it polluted their file).
 */
export async function readManagedPaths(): Promise<Set<string>> {
  try {
    const paths = await getManagedPaths();
    return new Set(paths);
  } catch {
    return new Set();
  }
}

/**
 * Mirror of the Rust `collect_leaf_paths`: walk a value and push the dot-path of
 * every leaf (scalar / array / null), skipping `_aidog_` keys. Used to compare a
 * subtree's full leaf set against the managed set so one-level diff nodes
 * (`hooks.Stop`, `extraKnownMarketplaces.x`) whose *entire* subtree is
 * aidog-managed can be excluded even though the frontend only expands depth 1.
 */
function collectValueLeafPaths(value: any, prefix: string, out: string[]): void {
  if (isPlainObject(value)) {
    for (const k of Object.keys(value)) {
      if (k.startsWith("_aidog_")) continue;
      const path = prefix === "" ? k : `${prefix}.${k}`;
      collectValueLeafPaths(value[k], path, out);
    }
  } else if (prefix !== "") {
    out.push(prefix);
  }
}

/**
 * A one-level diff child at `path` is fully managed iff its incoming subtree is
 * non-empty and every leaf path under it is in `managed`. Returns false when the
 * subtree contains any user-added (non-managed) leaf, so such nodes stay in the
 * diff. Exactly matches the Rust leaf granularity (`env.X`, `hooks.Stop.0...`).
 */
function isFullyManaged(incomingValue: any, path: string, managed: Set<string>): boolean {
  if (managed.size === 0) return false;
  const leaves: string[] = [];
  collectValueLeafPaths(incomingValue, path, leaves);
  if (leaves.length === 0) return false;
  return leaves.every((p) => managed.has(p));
}

/**
 * Claude Code self-written runtime preference fields. aidog never manages these
 * (its write side only injects `env.ANTHROPIC_BASE_URL` / `env.ANTHROPIC_AUTH_TOKEN`),
 * so they can't ride the `_aidog_managed` marker — the import diff filters them
 * here instead. Users don't care about them; surfacing them is pure noise.
 *
 * Exact top-level keys + dot-path prefixes (matching the diff's path conventions:
 * top-level scalar path=key; object child path=`${key}.${childKey}`).
 */
const CC_RUNTIME_IGNORE = {
  exact: new Set([
    "model",
    "effortLevel",
    "ultracode",
    "maxSkillDescriptionChars",
    "skipWebFetchPreflight",
    "workflowKeywordTriggerEnabled",
  ]),
  prefixes: ["fileSuggestion.", "env.ANTHROPIC_DEFAULT_"],
};

function isCcRuntimeIgnored(path: string): boolean {
  if (CC_RUNTIME_IGNORE.exact.has(path)) return true;
  return CC_RUNTIME_IGNORE.prefixes.some((p) => path.startsWith(p));
}

/**
 * Build the diff tree between `current` config and `incoming` source.
 * Skips internal `_aidog_` keys. Object top-level keys expand to child entries.
 *
 * `managed` = aidog-managed leaf dot-paths (from `readManagedPaths`); any leaf
 * whose dot-path is in the set is excluded so the diff lists only user-added
 * (non-managed) fields. Exclusion is precise to the sub-key: `env.ANTHROPIC_BASE_URL`
 * is dropped while a user's `env.FOO` is kept. A parent object node is dropped
 * only when all its diffing children are managed (none remain).
 *
 * TODO: only one level of nesting is expanded (covers permissions/env/hooks);
 * deeper objects are diffed as a single leaf.
 */
export function buildImportDiffTree(
  current: Record<string, any>,
  incoming: Record<string, any>,
  managed: Set<string>,
): DiffNode[] {
  const nodes: DiffNode[] = [];
  const keys = new Set([...Object.keys(current), ...Object.keys(incoming)]);
  for (const key of keys) {
    if (key.startsWith("_aidog_")) continue;
    const cur = current[key];
    const inc = incoming[key];
    if (JSON.stringify(cur) === JSON.stringify(inc)) continue;

    // Expand plain-object top-level keys one level into children.
    if (isPlainObject(cur) || isPlainObject(inc)) {
      const curObj = isPlainObject(cur) ? cur : {};
      const incObj = isPlainObject(inc) ? inc : {};
      const childKeys = new Set([...Object.keys(curObj), ...Object.keys(incObj)]);
      const children: DiffNode[] = [];
      for (const ck of childKeys) {
        const childPath = `${key}.${ck}`;
        // Exclude aidog-managed sub-keys. Precise to the child path: a leaf or
        // array child (`env.ANTHROPIC_BASE_URL`, `hooks.Stop`) is matched
        // directly; a deeper-object child whose entire subtree is managed
        // (`extraKnownMarketplaces.x`) is matched via its full incoming leaf set
        // — keeping any user-added sibling/leaf inside that subtree.
        if (
          managed.has(childPath) ||
          isCcRuntimeIgnored(childPath) ||
          isFullyManaged(incObj[ck], childPath, managed)
        )
          continue;
        if (JSON.stringify(curObj[ck]) === JSON.stringify(incObj[ck])) continue;
        children.push({
          path: childPath,
          label: ck,
          current: curObj[ck],
          incoming: incObj[ck],
        });
      }
      // All diffing children managed → parent fully managed → drop the node.
      if (children.length > 0) {
        nodes.push({ path: key, label: key, current: cur, incoming: inc, children });
      }
      continue;
    }
    // Scalar / array top-level leaf: exclude when the whole key is managed
    // or is a CC self-written runtime preference.
    if (managed.has(key) || isCcRuntimeIgnored(key)) continue;
    nodes.push({ path: key, label: key, current: cur, incoming: inc });
  }
  return nodes;
}

export function ImportDiffModal({
  diff,
  onApply,
  onClose,
}: {
  diff: DiffNode[];
  onApply: (selectedPaths: Set<string>) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  // All leaf paths (the actual selectable units).
  const allLeafPaths = useMemo(() => {
    const out: string[] = [];
    diff.forEach(n => collectLeafPaths(n, out));
    return out;
  }, [diff]);

  const [selected, setSelected] = useState<Set<string>>(() => new Set(allLeafPaths));

  const toggleLeaf = (path: string) => {
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path); else next.add(path);
      return next;
    });
  };

  // Toggle a parent: select/deselect all its leaves at once.
  const toggleNode = (node: DiffNode) => {
    const leaves: string[] = [];
    collectLeafPaths(node, leaves);
    const allOn = leaves.every(p => selected.has(p));
    setSelected(prev => {
      const next = new Set(prev);
      leaves.forEach(p => { if (allOn) next.delete(p); else next.add(p); });
      return next;
    });
  };

  // Parent checkbox state: full / none / partial.
  const nodeState = (node: DiffNode): "on" | "off" | "partial" => {
    const leaves: string[] = [];
    collectLeafPaths(node, leaves);
    const on = leaves.filter(p => selected.has(p)).length;
    if (on === 0) return "off";
    if (on === leaves.length) return "on";
    return "partial";
  };

  const toggleAll = () => {
    if (selected.size === allLeafPaths.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(allLeafPaths));
    }
  };

  const formatValue = (v: any): string => {
    if (v === undefined) return t("settings.editor.none", "(无)");
    if (typeof v === "object") return JSON.stringify(v, null, 2);
    return String(v);
  };

  const getChangeType = (d: { current: any; incoming: any }) => {
    if (d.current === undefined) return "added";
    if (d.incoming === undefined) return "removed";
    return "changed";
  };

  // Render one leaf row (selectable unit with value diff).
  const renderLeaf = (d: DiffNode, nested: boolean) => {
    const changeType = getChangeType(d);
    const isSelected = selected.has(d.path);
    const bgColor = changeType === "added" ? "color-mix(in srgb, var(--color-success) 6%, transparent)"
      : changeType === "removed" ? "color-mix(in srgb, var(--color-danger) 6%, transparent)"
      : "var(--bg-glass)";
    const labelColor = changeType === "added" ? "var(--color-success)"
      : changeType === "removed" ? "var(--color-danger)"
      : "var(--accent)";
    const label = changeType === "added" ? t("settings.editor.diffAdded", "新增") : changeType === "removed" ? t("settings.editor.diffRemoved", "删除") : t("settings.editor.diffChanged", "变更");
    return (
      <div key={d.path} style={{
        margin: nested ? "4px 0 4px 28px" : "4px 12px",
        padding: "8px 12px",
        background: isSelected ? bgColor : "var(--bg-surface)",
        border: `1px solid ${isSelected ? "var(--border)" : "transparent"}`,
        borderRadius: "var(--radius-sm)",
        opacity: isSelected ? 1 : 0.5,
        transition: "all 150ms",
      }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}
          onClick={() => toggleLeaf(d.path)}>
          {/* 阻止冒泡：否则点开关会触发 Toggle.onChange + 父 div.onClick 双次 toggle 互相抵消 → 看似无效 */}
          <span onClick={(e) => e.stopPropagation()} style={{ display: "inline-flex" }}>
            <Toggle active={isSelected} onChange={() => toggleLeaf(d.path)} />
          </span>
          <span style={{
            fontSize: F.body, fontWeight: 600, color: "var(--text-primary)",
            fontFamily: '"SF Mono", "Fira Code", monospace',
          }}>{d.label}</span>
          <span style={{
            fontSize: F.hint, fontWeight: 600, color: labelColor,
            padding: "1px 6px", background: `${labelColor}18`, borderRadius: "var(--radius-sm)",
          }}>{label}</span>
        </div>
        {isSelected && (
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, marginTop: 8, marginLeft: 36 }}>
            <div>
              <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 2 }}>{t("settings.editor.diffCurrent", "当前")}</div>
              <pre style={{
                fontFamily: '"SF Mono", "Fira Code", monospace',
                fontSize: F.hint, lineHeight: 1.5,
                background: "var(--bg-surface)", borderRadius: "var(--radius-sm)",
                padding: 8, overflow: "auto", whiteSpace: "pre-wrap", wordBreak: "break-all",
                color: d.current === undefined ? "var(--text-tertiary)" : "var(--text-primary)",
                margin: 0, maxHeight: 120,
              }}>{formatValue(d.current)}</pre>
            </div>
            <div>
              <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 2 }}>{t("settings.editor.diffIncoming", "导入")}</div>
              <pre style={{
                fontFamily: '"SF Mono", "Fira Code", monospace',
                fontSize: F.hint, lineHeight: 1.5,
                background: "var(--bg-surface)", borderRadius: "var(--radius-sm)",
                padding: 8, overflow: "auto", whiteSpace: "pre-wrap", wordBreak: "break-all",
                color: d.incoming === undefined ? "var(--text-tertiary)" : "var(--text-primary)",
                margin: 0, maxHeight: 120,
              }}>{formatValue(d.incoming)}</pre>
            </div>
          </div>
        )}
      </div>
    );
  };

  return (
    <div style={{
      position: "fixed", inset: 0, zIndex: 1000,
      display: "flex", alignItems: "center", justifyContent: "center",
      background: "rgba(0,0,0,0.5)", animation: "fadeIn 150ms ease both",
    }} onClick={onClose}>
      <div className="glass-elevated"
        style={{
          width: 680, maxHeight: "85vh", display: "flex", flexDirection: "column",
          padding: 0, borderRadius: "var(--radius-lg)",
          animation: "fadeIn 200ms ease both",
        }}
        onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div style={{
          padding: "16px 20px", borderBottom: "1px solid var(--border)",
          display: "flex", justifyContent: "space-between", alignItems: "center",
        }}>
          <div style={{ fontSize: F.title, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("settings.editor.importTitle", "从 Claude Code 导入配置")}
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <button className="btn btn-ghost" style={{ fontSize: F.hint, padding: "4px 10px" }}
              onClick={toggleAll}>
              {selected.size === allLeafPaths.length ? t("settings.editor.deselectAll", "取消全选") : t("settings.editor.selectAll", "全选")}
            </button>
            <button type="button" className="btn btn-ghost btn-icon"
              style={{ width: 28, height: 28, fontSize: F.body }}
              onClick={onClose}>×</button>
          </div>
        </div>

        {/* Diff list */}
        <div style={{ flex: 1, overflowY: "auto", padding: "8px 0" }}>
          {diff.map(node => {
            // Leaf node (no children) — render directly as a selectable row.
            if (!node.children || node.children.length === 0) {
              return renderLeaf(node, false);
            }
            // Parent node — header with tri-state toggle + nested children.
            const state = nodeState(node);
            return (
              <div key={node.path} style={{
                margin: "4px 12px", padding: "10px 14px",
                background: "var(--bg-glass)",
                border: "1px solid var(--border)",
                borderRadius: "var(--radius-sm)",
                opacity: state === "off" ? 0.6 : 1,
                transition: "all 150ms",
              }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}
                  onClick={() => toggleNode(node)}>
                  {/* 同 leaf：阻止冒泡避免 Toggle + 父 div 双次 toggle 抵消 */}
                  <span onClick={(e) => e.stopPropagation()} style={{ display: "inline-flex" }}>
                    <Toggle active={state !== "off"} onChange={() => toggleNode(node)} />
                  </span>
                  <span style={{
                    fontSize: F.body, fontWeight: 600, color: "var(--text-primary)",
                    fontFamily: '"SF Mono", "Fira Code", monospace',
                  }}>{node.label}</span>
                  <span style={{
                    fontSize: F.hint, fontWeight: 600,
                    color: state === "partial" ? "var(--color-warning)" : "var(--accent)",
                    padding: "1px 6px",
                    background: state === "partial" ? "color-mix(in srgb, var(--color-warning) 12%, transparent)" : "var(--accent-subtle)",
                    borderRadius: "var(--radius-sm)",
                  }}>{state === "partial" ? t("settings.editor.diffPartial", "部分") : t("settings.editor.diffObject", "对象")}</span>
                </div>
                <div style={{ marginTop: 6 }}>
                  {node.children.map(child => renderLeaf(child, true))}
                </div>
              </div>
            );
          })}
        </div>

        {/* Footer */}
        <div style={{
          padding: "12px 20px", borderTop: "1px solid var(--border)",
          display: "flex", justifyContent: "space-between", alignItems: "center",
        }}>
          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>
            {t("settings.editor.selectedPrefix", "已选")} {selected.size}/{allLeafPaths.length} {t("settings.editor.selectedSuffix", "项")}
          </span>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn btn-ghost" style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={onClose}>{t("action.cancel", "取消")}</button>
            <button className="btn btn-primary" style={{ fontSize: F.body, padding: S.btnPad }}
              disabled={selected.size === 0}
              onClick={() => onApply(selected)}>
              {t("settings.editor.importSelected", "导入选中")} ({selected.size})
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
