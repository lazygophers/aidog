import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { settingsApi, claudeSettingsImportApi, configApi, statuslineApi } from "../services/api";
import { registerNavGuard } from "../utils/navGuard";
import { UnsavedChangesModal } from "../components/settings/UnsavedChangesModal";
import { SECTIONS, RECOMMENDED_CONFIG } from "../services/claude-settings-schema";
import {
  F,
  S,
  SectionIcon,
  FieldRenderer,
  EnvEditor,
  PermissionsSectionInline,
  SandboxSectionInline,
  PluginsSectionInline,
  HooksSectionInline,
  StatusLineSection,
  materializeStatusline,
  ImportDiffModal,
  buildImportDiffTree,
  isPlainObject,
  type DiffNode,
  type HooksConfig,
} from "../components/settings/editors";
import { SettingsHeader } from "../components/settings/SettingsHeader";
import { SectionAnchorNav } from "../components/settings/SectionAnchorNav";

const CONFIG_KEY = "claude_code";

// Header + anchor-nav heights drive sticky offsets & scroll-spy margins.
const HEADER_H = 58;
const NAV_H = 50;

// Order-insensitive JSON serialization — used as the dirty-state signature
// so reordered object keys don't register as a change.
function stableStringify(value: any): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  const keys = Object.keys(value).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stableStringify(value[k])}`).join(",")}}`;
}

// Materialize the native `statusLine` / `subagentStatusLine` fields from their
// `_aidog_*` UI drafts at save time — the authoritative, race-free path that
// replaces the UI's debounce-effect draft writes. For each of main + subagent:
//   • disabled            → delete the native field
//   • enabled + custom    → field = { type:"command", command:<customCommand> }
//                           (empty command → delete the field)
//   • enabled + builtin   → generate the bash script (writes the .sh file via
//                           statuslineApi.generate) → field.command = <path>
// Returns a NEW config object; the source is never mutated. Best-effort: a
// generate() failure logs and leaves that field untouched, never blocks save.
async function materializeStatuslineFields(
  config: Record<string, any>,
): Promise<Record<string, any>> {
  const next = { ...config };
  const targets: { aidogKey: string; fieldName: string; scriptType: "statusline" | "subagent"; isMain: boolean }[] = [
    { aidogKey: "_aidog_statusline", fieldName: "statusLine", scriptType: "statusline", isMain: true },
    { aidogKey: "_aidog_subagent_statusline", fieldName: "subagentStatusLine", scriptType: "subagent", isMain: false },
  ];

  for (const { aidogKey, fieldName, scriptType, isMain } of targets) {
    const stored = next[aidogKey] as Record<string, any> | undefined;
    const m = materializeStatusline(stored, scriptType);

    // Disabled → drop the native field entirely.
    if (!m.enabled) {
      delete next[fieldName];
      continue;
    }

    // Custom → write the user command verbatim; empty → drop the field.
    if (m.mode === "custom") {
      const cmd = m.customCommand.trim();
      if (!cmd) {
        delete next[fieldName];
        continue;
      }
      const value: Record<string, any> = { type: "command", command: cmd };
      if (m.padding > 0) value.padding = m.padding;
      next[fieldName] = value;
      continue;
    }

    // Builtin → generate the script file, point the field at the returned path.
    if (m.scriptContent != null) {
      try {
        const path = await statuslineApi.generate(scriptType, m.scriptContent);
        const value: Record<string, any> = { type: "command", command: path };
        if (isMain && m.padding > 0) value.padding = m.padding;
        next[fieldName] = value;
      } catch (e) {
        // Never block the save — leave the existing field value as-is.
        console.error(`materialize ${fieldName}:`, e);
      }
    }
  }

  return next;
}

// ─── Main Settings Page ────────────────────────────────────

export function Settings() {
  const { t } = useTranslation();
  const [mode, setMode] = useState<"json" | "gui">("gui");
  const [config, setConfig] = useState<Record<string, any>>({});
  const [editJson, setEditJson] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [toast, setToast] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [activeSection, setActiveSection] = useState(SECTIONS[0]?.id ?? "core");
  const [importDiff, setImportDiff] = useState<{
    source: Record<string, any>;
    diff: DiffNode[];
  } | null>(null);

  // Last-saved baseline signature for dirty tracking.
  const [baseline, setBaseline] = useState<string>("");
  // Pending navigation awaiting unsaved-changes confirmation.
  const [pendingNav, setPendingNav] = useState<{ proceed: () => void } | null>(null);

  const scrollRef = useRef<HTMLDivElement>(null);
  const sectionRefs = useRef<Record<string, HTMLDivElement | null>>({});
  // 粘性顶栏 / 锚点栏的实际渲染高度（chip 可能换行成 2 行）。用 ResizeObserver 量测，
  // 替代写死的 HEADER_H/NAV_H，驱动 scroll-spy rootMargin + section scrollMarginTop + nav top。
  const headerElRef = useRef<HTMLDivElement | null>(null);
  const navElRef = useRef<HTMLDivElement | null>(null);
  const [headerH, setHeaderH] = useState(HEADER_H);
  const [navH, setNavH] = useState(NAV_H);
  const setHeaderEl = useCallback((el: HTMLDivElement | null) => { headerElRef.current = el; }, []);
  const setNavEl = useCallback((el: HTMLDivElement | null) => { navElRef.current = el; }, []);

  // Current draft signature: in JSON mode use the textarea (best-effort parse),
  // otherwise the structured config object.
  const currentSig = useMemo(() => {
    if (mode === "json") {
      try {
        return stableStringify(JSON.parse(editJson));
      } catch {
        // Invalid JSON while typing → treat as dirty.
        return `__invalid__${editJson}`;
      }
    }
    return stableStringify(config);
  }, [mode, editJson, config]);

  const dirty = baseline !== "" && currentSig !== baseline;

  useEffect(() => {
    const load = async () => {
      try {
        const result = await settingsApi.get("global", CONFIG_KEY);
        const stored = result as Record<string, any> | null | undefined;
        // 若从未存储过，默认填入推荐配置
        const data = stored && Object.keys(stored).length > 0 ? stored : { ...RECOMMENDED_CONFIG };
        setConfig(data);
        setEditJson(JSON.stringify(data, null, 2));
        // Establish the saved baseline so initial state is non-dirty.
        setBaseline(stableStringify(data));
      } catch (e) {
        console.error(e);
      }
    };
    load();
  }, []);

  const updateField = useCallback((field: string, value: any) => {
    setConfig((prev) => {
      const next: Record<string, any> = {};
      for (const [k, v] of Object.entries(prev)) {
        if (k !== field) next[k] = v;
      }
      if (value !== undefined && value !== null && value !== "") {
        next[field] = value;
      }
      return next;
    });
  }, []);

  const handleSave = useCallback(async (): Promise<boolean> => {
    setSaving(true);
    setSaveError("");
    try {
      const draft = mode === "json" ? JSON.parse(editJson) : { ...config };
      // Authoritative, race-free materialization of statusLine / subagentStatusLine
      // from their `_aidog_*` drafts — must run BEFORE persisting so the native
      // fields are always in sync with the toggle/mode the user just saved.
      const value = await materializeStatuslineFields(draft);
      await settingsApi.set("global", CONFIG_KEY, value);
      setConfig(value);
      setEditJson(JSON.stringify(value, null, 2));
      // Refresh the baseline → draft becomes non-dirty.
      setBaseline(stableStringify(value));
      // Re-sync per-group `settings.{group}.json` so changes that live in the
      // effective files (statusLine, etc.) actually take effect. Best-effort:
      // never block the save if syncing fails.
      try {
        await configApi.syncGroupSettings();
      } catch (e) {
        console.error("sync_group_settings:", e);
      }
      setToast(t("settings.saved"));
      setTimeout(() => setToast(""), 2000);
      setSaving(false);
      return true;
    } catch (e: any) {
      setSaveError(e.toString());
      setSaving(false);
      return false;
    }
  }, [mode, editJson, config, t]);

  const handleLoadRecommended = () => {
    const merged = { ...RECOMMENDED_CONFIG, ...config };
    setConfig(merged);
    setEditJson(JSON.stringify(merged, null, 2));
    setToast(t("settings.loadedRecommended"));
    setTimeout(() => setToast(""), 2000);
  };

  const handleImportFromClaudeCode = async () => {
    try {
      const source = await claudeSettingsImportApi.readDefault();
      // Build nested diff tree: top-level objects expand one level into children.
      const diff = buildImportDiffTree(config, source);
      if (diff.length === 0) {
        setToast(t("settings.noDiff", "无差异，无需导入"));
        setTimeout(() => setToast(""), 2000);
        return;
      }
      setImportDiff({ source, diff });
    } catch (e: any) {
      setToast(e?.toString?.() ?? "导入失败");
      setTimeout(() => setToast(""), 3000);
    }
  };

  const applyImport = (selectedPaths: Set<string>) => {
    if (!importDiff) return;
    // Deep-merge selected dot-paths from source into a clone of current config.
    // Unselected sub-keys keep their current value (object keys are cloned before merge).
    const next: Record<string, any> = JSON.parse(JSON.stringify(config));
    const { source } = importDiff;
    for (const path of selectedPaths) {
      const segs = path.split(".");
      // Resolve incoming value by walking source along the path.
      let incoming: any = source;
      let found = true;
      for (const s of segs) {
        if (incoming != null && typeof incoming === "object" && s in incoming) {
          incoming = incoming[s];
        } else {
          incoming = undefined;
          found = false;
          break;
        }
      }
      // Write into next at the path, creating intermediate objects as needed.
      let cursor = next;
      for (let i = 0; i < segs.length - 1; i++) {
        const s = segs[i];
        if (!isPlainObject(cursor[s])) cursor[s] = {};
        cursor = cursor[s];
      }
      const leaf = segs[segs.length - 1];
      if (found) {
        cursor[leaf] = incoming;
      } else {
        // Source lacks this key (a "removed" diff) → drop it.
        delete cursor[leaf];
      }
    }
    setConfig(next);
    setEditJson(JSON.stringify(next, null, 2));
    setImportDiff(null);
    setToast(t("settings.imported", "已导入"));
    setTimeout(() => setToast(""), 2000);
  };

  // Permissions helpers for the special permissions sub-editor
  const perms = (config.permissions ?? {}) as Record<string, any>;

  // ── R8: Global search — filter sections & highlight matching fields ──
  // A field matches when its translated label, raw key, or description contains
  // the query. A section is shown when its own label matches (→ all fields shown)
  // or at least one of its fields matches (→ only matched fields shown).
  const search = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return null; // No active filter.
    const matched = new Map<string, "section" | Set<string>>();
    for (const section of SECTIONS) {
      const sectionLabel = t(section.labelKey).toLowerCase();
      if (sectionLabel.includes(q)) {
        matched.set(section.id, "section"); // Whole section matches → show all fields.
        continue;
      }
      const hits = new Set<string>();
      for (const f of section.fields) {
        const label = t(`settings.f_${f.key}`, f.label).toLowerCase();
        const desc = (f.description ?? "").toLowerCase();
        if (label.includes(q) || f.key.toLowerCase().includes(q) || desc.includes(q)) {
          hits.add(f.key);
        }
      }
      if (hits.size > 0) matched.set(section.id, hits);
    }
    return { q, matched };
  }, [searchQuery, t]);

  const visibleSections = useMemo(
    () => (search ? SECTIONS.filter((s) => search.matched.has(s.id)) : SECTIONS),
    [search],
  );

  // ── 量测粘性顶栏 + 锚点栏实际高度（换行时 > 单行常量），驱动偏移 ──
  useEffect(() => {
    if (mode !== "gui") return;
    const hEl = headerElRef.current;
    const nEl = navElRef.current;
    const ro = new ResizeObserver(() => {
      if (hEl) setHeaderH(Math.round(hEl.getBoundingClientRect().height));
      if (nEl) setNavH(Math.round(nEl.getBoundingClientRect().height));
    });
    if (hEl) ro.observe(hEl);
    if (nEl) ro.observe(nEl);
    // 首帧立即量一次（observe 回调虽会触发，但确保初值准确）
    if (hEl) setHeaderH(Math.round(hEl.getBoundingClientRect().height));
    if (nEl) setNavH(Math.round(nEl.getBoundingClientRect().height));
    return () => ro.disconnect();
  }, [mode]);

  // ── Scroll-spy: highlight the section currently in view ──
  useEffect(() => {
    if (mode !== "gui") return;
    const root = scrollRef.current;
    if (!root) return;
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((e) => e.isIntersecting)
          .sort((a, b) => b.intersectionRatio - a.intersectionRatio);
        if (visible.length > 0) {
          const id = (visible[0].target as HTMLElement).dataset.sectionId;
          if (id) setActiveSection(id);
        }
      },
      { root, rootMargin: `-${headerH + navH}px 0px -55% 0px`, threshold: [0, 0.25, 0.5, 1] },
    );
    visibleSections.forEach((s) => {
      const el = sectionRefs.current[s.id];
      if (el) observer.observe(el);
    });
    return () => observer.disconnect();
  }, [mode, visibleSections, headerH, navH]);

  // ── R8: on search, scroll the first matching section into view ──
  useEffect(() => {
    if (mode !== "gui") return;
    if (!search) return; // Cleared → keep current position.
    const first = visibleSections[0];
    if (!first) return;
    const el = sectionRefs.current[first.id];
    el?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, [searchQuery, mode]); // Re-run when the query text changes.

  // ── Smooth-scroll to a section on chip click ──
  const jumpToSection = useCallback((id: string) => {
    setActiveSection(id);
    const el = sectionRefs.current[id];
    el?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, []);

  // ── Cmd/Ctrl+S → save (only when dirty) ──
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const isSaveCombo = (e.metaKey || e.ctrlKey) && (e.key === "s" || e.key === "S");
      if (!isSaveCombo) return;
      // Always swallow the browser "save page" shortcut on this screen.
      e.preventDefault();
      if (dirty && !saving) void handleSave();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [dirty, saving, handleSave]);

  // ── Navigation guard: intercept page/tab switches while dirty ──
  useEffect(() => {
    if (!dirty) return; // No guard when clean → navigation flows freely.
    const unregister = registerNavGuard((proceed) => {
      setPendingNav({ proceed });
    });
    return unregister;
  }, [dirty]);

  // ── Tauri window close-requested: warn on unsaved changes ──
  useEffect(() => {
    if (!dirty) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const win = getCurrentWindow();
        const stop = await win.onCloseRequested((event) => {
          // Block the close and surface the in-app confirm modal.
          event.preventDefault();
          setPendingNav({ proceed: () => void win.destroy() });
        });
        if (cancelled) stop();
        else unlisten = stop;
      } catch {
        // Non-Tauri context or API unavailable → silently skip.
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [dirty]);

  // ── Resolve a pending navigation from the unsaved-changes modal ──
  const handleSaveAndLeave = useCallback(async () => {
    const ok = await handleSave();
    if (!ok) return; // Save failed → stay; error shown inline.
    const proceed = pendingNav?.proceed;
    setPendingNav(null);
    proceed?.();
  }, [handleSave, pendingNav]);

  const handleDiscardAndLeave = useCallback(() => {
    const proceed = pendingNav?.proceed;
    setPendingNav(null);
    proceed?.();
  }, [pendingNav]);

  // Render a single section's inner content (inside its Liquid Glass card).
  const renderSectionContent = (section: typeof SECTIONS[number]) => {
    if (section.id === "permissions") {
      return <PermissionsSectionInline perms={perms} updateField={updateField} t={t} />;
    }
    if (section.id === "env") {
      return (
        <EnvEditor
          env={(config.env ?? {}) as Record<string, string>}
          onChange={(newEnv) =>
            updateField("env", Object.keys(newEnv).length > 0 ? newEnv : undefined)
          }
          t={t}
        />
      );
    }
    if (section.id === "sandbox") {
      return (
        <SandboxSectionInline
          sandboxValue={config.sandbox as Record<string, any> | undefined}
          updateField={updateField}
        />
      );
    }
    if (section.id === "plugins") {
      return <PluginsSectionInline config={config} updateField={updateField} />;
    }
    if (section.id === "hooks") {
      return (
        <HooksSectionInline
          hooksValue={config.hooks as HooksConfig | undefined}
          updateField={updateField}
          t={t}
        />
      );
    }
    if (section.id === "status") {
      return <StatusLineSection config={config} updateField={updateField} t={t} />;
    }

    // R8: when searching, narrow to matched fields unless the section label matched.
    const fieldFilter = search?.matched.get(section.id);
    const visibleFields = section.fields.filter((f) => {
      if (f.skipGui) return false;
      if (fieldFilter instanceof Set) return fieldFilter.has(f.key);
      return true;
    });
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
        {visibleFields.map((field) => {
          const hasDefault = Object.prototype.hasOwnProperty.call(RECOMMENDED_CONFIG, field.key);
          const defaultValue = hasDefault ? RECOMMENDED_CONFIG[field.key] : undefined;
          return (
            <FieldRenderer
              key={field.key}
              field={field}
              value={config[field.key]}
              onChange={(v) => updateField(field.key, v)}
              t={t}
              defaultValue={defaultValue}
              onReset={hasDefault ? () => updateField(field.key, defaultValue) : undefined}
              highlight={search?.q}
            />
          );
        })}
        {/* Attribution fixed editor (commit + pr only) — hidden when search narrows to specific fields */}
        {section.id === "advanced" && !(fieldFilter instanceof Set) && (() => {
          const attr = (config.attribution ?? {}) as Record<string, string>;
          const rowStyle: React.CSSProperties = { display: "flex", alignItems: "center", gap: 12 };
          return (
            <div style={{ display: "flex", flexDirection: "column", gap: S.row, borderTop: "1px solid var(--border)", paddingTop: S.gap }}>
              <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-secondary)" }}>
                {t("settings.f_attribution", "Attribution")}
              </div>
              {(["commit", "pr"] as const).map((field) => (
                <div key={field} style={rowStyle}>
                  <label style={{ flexShrink: 0, width: S.labelW, fontSize: F.label, fontWeight: 500, color: "var(--text-primary)", paddingTop: 10 }}>
                    {field === "commit" ? t("settings.attribution.commit", "Commit Author") : t("settings.attribution.pr", "PR Author")}
                    <span style={{ display: "block", fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginTop: 2 }}>{field}</span>
                  </label>
                  <input className="input" style={{ flex: 1, fontSize: F.body, padding: S.inputPad }}
                    placeholder="e.g. Your Name <you@example.com>"
                    value={attr[field] ?? ""}
                    onChange={(e) => {
                      const next = { ...attr, [field]: e.target.value };
                      updateField("attribution", Object.values(next).some(Boolean) ? next : undefined);
                    }} />
                </div>
              ))}
            </div>
          );
        })()}
      </div>
    );
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "calc(100vh - 48px)", width: "100%" }}>
      {mode === "json" ? (
        <>
          <SettingsHeader
            mode={mode}
            onModeChange={(m) => {
              if (m === "json") setEditJson(JSON.stringify(config, null, 2));
              setMode(m);
            }}
            search={searchQuery}
            onSearchChange={setSearchQuery}
            onLoadRecommended={handleLoadRecommended}
            onImport={handleImportFromClaudeCode}
            onSave={handleSave}
            saving={saving}
            toast={toast}
            dirty={dirty}
          />
          <div
            className="glass-surface"
            style={{ flex: 1, display: "flex", flexDirection: "column", padding: S.pad, borderRadius: "var(--radius-lg)", overflow: "hidden", marginTop: 12 }}
          >
            <textarea
              className="input"
              style={{
                fontFamily: '"SF Mono", "Fira Code", monospace',
                fontSize: F.body,
                lineHeight: 1.7,
                flex: 1,
                resize: "none",
                whiteSpace: "pre",
                padding: S.inputPad,
                minHeight: 0,
              }}
              value={editJson}
              onChange={(e) => setEditJson(e.target.value)}
              spellCheck={false}
            />
            {saveError && (
              <div style={{ fontSize: F.body, color: "#ff453a", marginTop: 12, wordBreak: "break-all" }}>
                {saveError}
              </div>
            )}
          </div>
        </>
      ) : (
        <div ref={scrollRef} style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
          <SettingsHeader
            mode={mode}
            onModeChange={(m) => {
              if (m === "json") setEditJson(JSON.stringify(config, null, 2));
              setMode(m);
            }}
            search={searchQuery}
            onSearchChange={setSearchQuery}
            onLoadRecommended={handleLoadRecommended}
            onImport={handleImportFromClaudeCode}
            onSave={handleSave}
            saving={saving}
            toast={toast}
            dirty={dirty}
            rootRef={setHeaderEl}
          />
          <SectionAnchorNav activeId={activeSection} onJump={jumpToSection} top={headerH} rootRef={setNavEl} />

          <div style={{ display: "flex", flexDirection: "column", gap: S.sectionGap, padding: "20px 4px 80px" }}>
            {visibleSections.length === 0 && (
              <div
                className="glass-surface"
                style={{ padding: S.pad, borderRadius: "var(--radius-lg)", textAlign: "center", color: "var(--text-tertiary)", fontSize: F.body }}
              >
                {t("settings.searchNoMatch")}
              </div>
            )}
            {visibleSections.map((section) => (
              <div
                key={section.id}
                data-section-id={section.id}
                ref={(el) => { sectionRefs.current[section.id] = el; }}
                className="glass-surface glass-highlight settings-section-card"
                style={{ padding: S.pad, borderRadius: "var(--radius-lg)", scrollMarginTop: headerH + navH + 12 }}
              >
                <div style={{ marginBottom: S.gap + 4 }}>
                  <div style={{ fontSize: F.title, fontWeight: 600, color: "var(--text-primary)", letterSpacing: "-0.01em", display: "flex", alignItems: "center", gap: 8 }}>
                    <SectionIcon name={section.id} size={20} />
                    {t(section.labelKey)}
                  </div>
                </div>
                {renderSectionContent(section)}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Import diff modal */}
      {importDiff && (
        <ImportDiffModal
          diff={importDiff.diff}
          onApply={applyImport}
          onClose={() => setImportDiff(null)}
        />
      )}

      {/* Unsaved-changes confirm (page/tab switch or window close) */}
      {pendingNav && (
        <UnsavedChangesModal
          saving={saving}
          onSave={handleSaveAndLeave}
          onDiscard={handleDiscardAndLeave}
          onCancel={() => setPendingNav(null)}
        />
      )}
    </div>
  );
}
