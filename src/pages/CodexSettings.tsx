import { useState, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { codexApi } from "../services/api";
import {
  F,
  S,
  SectionIcon,
  FieldRenderer,
} from "../components/settings/editors";
import {
  CODEX_SECTIONS,
  CODEX_RECOMMENDED_CONFIG,
} from "../services/codex-settings-schema";
import { deepMerge } from "../utils/deepMerge";

// Order-insensitive serialization for dirty tracking (mirrors Settings.tsx).
function stableStringify(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  const obj = value as Record<string, unknown>;
  const keys = Object.keys(obj).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stableStringify(obj[k])}`).join(",")}}`;
}

type CodexConfig = Record<string, unknown>;

// ─── Codex Global Settings Page ────────────────────────────

export function CodexSettings() {
  const { t } = useTranslation();
  const [mode, setMode] = useState<"gui" | "json">("gui");
  const [config, setConfig] = useState<CodexConfig>({});
  const [editJson, setEditJson] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [toast, setToast] = useState("");
  const [baseline, setBaseline] = useState("");

  const currentSig = useMemo(() => {
    if (mode === "json") {
      try {
        return stableStringify(JSON.parse(editJson));
      } catch {
        return `__invalid__${editJson}`;
      }
    }
    return stableStringify(config);
  }, [mode, editJson, config]);

  const dirty = baseline !== "" && currentSig !== baseline;

  useEffect(() => {
    const load = async () => {
      try {
        const stored = await codexApi.read();
        // 从未配置过 → 默认填入推荐配置，便于用户一键起步。
        const data =
          stored && Object.keys(stored).length > 0
            ? stored
            : { ...CODEX_RECOMMENDED_CONFIG };
        setConfig(data);
        setEditJson(JSON.stringify(data, null, 2));
        setBaseline(stableStringify(data));
      } catch (e) {
        console.error("codex_config_read:", e);
      }
    };
    load();
  }, []);

  const updateField = useCallback((field: string, value: unknown) => {
    setConfig((prev) => {
      const next: CodexConfig = {};
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
      const value: CodexConfig = mode === "json" ? JSON.parse(editJson) : { ...config };
      await codexApi.write(value);
      setConfig(value);
      setEditJson(JSON.stringify(value, null, 2));
      setBaseline(stableStringify(value));
      setToast(t("settings.saved", "已保存"));
      setTimeout(() => setToast(""), 2000);
      setSaving(false);
      return true;
    } catch (e) {
      setSaveError(String(e));
      setSaving(false);
      return false;
    }
  }, [mode, editJson, config, t]);

  const handleLoadRecommended = () => {
    const merged = deepMerge(config, CODEX_RECOMMENDED_CONFIG);
    setConfig(merged);
    setEditJson(JSON.stringify(merged, null, 2));
    setToast(t("settings.loadedRecommended", "已加载推荐配置"));
    setTimeout(() => setToast(""), 2000);
  };

  const renderSection = (section: (typeof CODEX_SECTIONS)[number]) => (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {section.fields.map((field) => {
        const hasDefault = Object.prototype.hasOwnProperty.call(CODEX_RECOMMENDED_CONFIG, field.key);
        const defaultValue = hasDefault ? CODEX_RECOMMENDED_CONFIG[field.key] : undefined;
        return (
          <FieldRenderer
            key={field.key}
            field={field}
            value={config[field.key]}
            onChange={(v) => updateField(field.key, v)}
            t={t}
            defaultValue={defaultValue}
            onReset={hasDefault ? () => updateField(field.key, defaultValue) : undefined}
          />
        );
      })}
    </div>
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "calc(100vh - 48px)", width: "100%" }}>
      {/* Sticky header */}
      <div
        className="settings-sticky-bar"
        style={{
          position: "sticky",
          top: 0,
          zIndex: 30,
          display: "flex",
          alignItems: "center",
          gap: 8,
          flexWrap: "wrap",
          padding: "12px 4px",
          background: "var(--bg-glass)",
          backdropFilter: "blur(20px)",
          WebkitBackdropFilter: "blur(20px)",
        }}
      >
        <div style={{ fontSize: F.title, fontWeight: 600, color: "var(--text-primary)", display: "flex", alignItems: "center", gap: 8, marginRight: 8 }}>
          <SectionIcon name="bolt" size={18} />
          {t("codex.title", "Codex 配置")}
        </div>

        <button
          className={`btn ${mode === "gui" ? "btn-primary" : "btn-ghost"}`}
          style={{ fontSize: F.body, padding: S.btnPad }}
          onClick={() => setMode("gui")}
        >
          {t("settings.guiMode", "图形")}
        </button>
        <button
          className={`btn ${mode === "json" ? "btn-primary" : "btn-ghost"}`}
          style={{ fontSize: F.body, padding: S.btnPad }}
          onClick={() => {
            setEditJson(JSON.stringify(config, null, 2));
            setMode("json");
          }}
        >
          {t("settings.jsonMode", "JSON")}
        </button>

        <div style={{ flex: 1 }} />

        <button
          className="btn btn-ghost"
          style={{ fontSize: F.hint, padding: "6px 14px" }}
          onClick={handleLoadRecommended}
        >
          <SectionIcon name="bolt" size={14} /> {t("settings.loadRecommended", "加载推荐配置")}
        </button>

        {toast && <span style={{ fontSize: F.body, color: "var(--color-success)" }}>{toast}</span>}
        {!toast && (
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 6,
              fontSize: F.hint,
              color: dirty ? "var(--color-warning)" : "var(--text-tertiary)",
            }}
          >
            <span
              style={{
                width: 7,
                height: 7,
                borderRadius: "50%",
                background: dirty ? "var(--color-warning)" : "var(--text-tertiary)",
                opacity: dirty ? 1 : 0.5,
                flexShrink: 0,
              }}
            />
            {dirty ? t("settings.unsavedChanges", "未保存更改") : t("settings.allSaved", "已保存")}
          </span>
        )}

        <button
          className={`btn ${dirty ? "btn-primary" : "btn-ghost"}`}
          style={{ fontSize: F.body, padding: S.btnPad, minWidth: 80 }}
          onClick={handleSave}
          disabled={saving || !dirty}
        >
          {saving ? t("status.loading", "保存中…") : t("action.save", "保存")}
        </button>
      </div>

      {mode === "json" ? (
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
            <div style={{ fontSize: F.body, color: "var(--color-danger)", marginTop: 12, wordBreak: "break-all" }}>
              {saveError}
            </div>
          )}
        </div>
      ) : (
        <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: S.sectionGap, padding: "20px 4px 80px" }}>
            {CODEX_SECTIONS.map((section) => (
              <div
                key={section.id}
                className="glass-surface glass-highlight settings-section-card"
                style={{ padding: S.pad, borderRadius: "var(--radius-lg)" }}
              >
                <div style={{ marginBottom: S.gap + 4 }}>
                  <div style={{ fontSize: F.title, fontWeight: 600, color: "var(--text-primary)", letterSpacing: "-0.01em", display: "flex", alignItems: "center", gap: 8 }}>
                    <SectionIcon name={section.id} size={20} />
                    {t(section.labelKey)}
                  </div>
                </div>
                {renderSection(section)}
              </div>
            ))}
            {saveError && (
              <div style={{ fontSize: F.body, color: "var(--color-danger)", wordBreak: "break-all", padding: "0 4px" }}>
                {saveError}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
