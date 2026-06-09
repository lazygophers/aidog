import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { settingsApi } from "../services/api";
import {
  SECTIONS,
  RECOMMENDED_CONFIG,
  type SettingField,
} from "../services/claude-settings-schema";

const CONFIG_KEY = "claude_code";

// ─── Sub-components ────────────────────────────────────────

/** Toggle switch */
function Toggle({
  active,
  onChange,
}: {
  active: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div
      className={`toggle ${active ? "active" : ""}`}
      style={{ cursor: "pointer", flexShrink: 0 }}
      onClick={() => onChange(!active)}
    />
  );
}

/** Collapsible section */
function Section({
  title,
  defaultOpen = false,
  children,
}: {
  title: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div
      style={{
        borderTop: "1px solid var(--border)",
        paddingTop: 12,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          cursor: "pointer",
          userSelect: "none",
        }}
        onClick={() => setOpen(!open)}
      >
        <span
          style={{
            fontSize: 14,
            fontWeight: 600,
            color: "var(--text-primary)",
          }}
        >
          {title}
        </span>
        <span
          style={{
            fontSize: 12,
            color: "var(--text-tertiary)",
            transition: "transform 0.15s",
            transform: open ? "rotate(90deg)" : "rotate(0deg)",
          }}
        >
          ▶
        </span>
      </div>
      {open && (
        <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 12 }}>
          {children}
        </div>
      )}
    </div>
  );
}

/** Label with optional original-key hint */
function FieldLabel({ field, t }: { field: SettingField; t: ReturnType<typeof useTranslation>["t"] }) {
  const translated = t(`settings.f_${field.key}`, field.label);
  return (
    <label
      style={{
        display: "block",
        fontSize: 13,
        fontWeight: 500,
        color: "var(--text-secondary)",
        marginBottom: 4,
      }}
    >
      {translated}{" "}
      <span style={{ fontSize: 11, color: "var(--text-tertiary)", fontWeight: 400 }}>
        ({field.key})
      </span>
      {field.description && (
        <span style={{ fontWeight: 400, marginLeft: 6, fontSize: 11, color: "var(--text-tertiary)" }}>
          {field.description}
        </span>
      )}
    </label>
  );
}

/** JSON textarea for complex objects */
function JsonEditor({
  value,
  onChange,
  placeholder,
  rows = 6,
}: {
  value: any;
  onChange: (v: any) => void;
  placeholder?: string;
  rows?: number;
}) {
  const [text, setText] = useState(() => {
    if (value === undefined || value === null) return "";
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return String(value);
    }
  });
  const [error, setError] = useState("");

  useEffect(() => {
    if (value === undefined || value === null) {
      setText("");
      return;
    }
    try {
      setText(JSON.stringify(value, null, 2));
    } catch {
      setText(String(value));
    }
  }, [value]);

  return (
    <div>
      <textarea
        className="input"
        style={{
          fontFamily: '"SF Mono", "Fira Code", monospace',
          fontSize: 12,
          lineHeight: 1.5,
          minHeight: rows * 20,
          resize: "vertical",
          whiteSpace: "pre",
        }}
        value={text}
        placeholder={placeholder}
        spellCheck={false}
        onChange={(e) => {
          setText(e.target.value);
          setError("");
          try {
            if (e.target.value.trim()) {
              const parsed = JSON.parse(e.target.value);
              onChange(parsed);
            } else {
              onChange(undefined);
            }
          } catch {
            setError("Invalid JSON");
          }
        }}
      />
      {error && (
        <span style={{ fontSize: 11, color: "#ff453a" }}>{error}</span>
      )}
    </div>
  );
}

/** Key-value editor (for env) */
function KvEditor({
  items,
  onChange,
}: {
  items: Record<string, string>;
  onChange: (items: Record<string, string>) => void;
}) {
  const [newKey, setNewKey] = useState("");
  const [newVal, setNewVal] = useState("");
  const entries = Object.entries(items);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      {entries.map(([k, v]) => (
        <div key={k} style={{ display: "flex", gap: 4 }}>
          <input
            className="input"
            style={{ flex: 2, fontSize: 13 }}
            value={k}
            readOnly
          />
          <input
            className="input"
            style={{ flex: 3, fontSize: 13 }}
            value={v}
            onChange={(e) => onChange({ ...items, [k]: e.target.value })}
          />
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            style={{ width: 26, height: 26, minWidth: 26, fontSize: 13 }}
            onClick={() => {
              const next = { ...items };
              delete next[k];
              onChange(next);
            }}
          >
            ×
          </button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 4 }}>
        <input
          className="input"
          style={{ flex: 2, fontSize: 13 }}
          placeholder="KEY"
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
        />
        <input
          className="input"
          style={{ flex: 3, fontSize: 13 }}
          placeholder="VALUE"
          value={newVal}
          onChange={(e) => setNewVal(e.target.value)}
        />
        <button
          type="button"
          className="btn btn-ghost"
          style={{ fontSize: 13, padding: "4px 8px" }}
          onClick={() => {
            if (newKey.trim()) {
              onChange({ ...items, [newKey.trim()]: newVal });
              setNewKey("");
              setNewVal("");
            }
          }}
        >
          +
        </button>
      </div>
    </div>
  );
}

/** String list editor (for permissions allow/ask/deny) */
function StringListEditor({
  items,
  onChange,
  addLabel,
}: {
  items: string[];
  onChange: (items: string[]) => void;
  addLabel: string;
}) {
  const [draft, setDraft] = useState("");

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      {items.map((item, i) => (
        <div key={i} style={{ display: "flex", gap: 4 }}>
          <input
            className="input"
            style={{ flex: 1, fontSize: 13 }}
            value={item}
            onChange={(e) => {
              const next = [...items];
              next[i] = e.target.value;
              onChange(next);
            }}
          />
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            style={{ width: 26, height: 26, minWidth: 26, fontSize: 13 }}
            onClick={() => onChange(items.filter((_, j) => j !== i))}
          >
            ×
          </button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 4 }}>
        <input
          className="input"
          style={{ flex: 1, fontSize: 13 }}
          placeholder={addLabel}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && draft.trim()) {
              onChange([...items, draft.trim()]);
              setDraft("");
            }
          }}
        />
        <button
          type="button"
          className="btn btn-ghost"
          style={{ fontSize: 13, padding: "4px 8px" }}
          onClick={() => {
            if (draft.trim()) {
              onChange([...items, draft.trim()]);
              setDraft("");
            }
          }}
        >
          +
        </button>
      </div>
    </div>
  );
}

// ─── Field Renderer ────────────────────────────────────────

function FieldRenderer({
  field,
  value,
  onChange,
  t,
}: {
  field: SettingField;
  value: any;
  onChange: (v: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  switch (field.type) {
    case "boolean":
      return (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
          }}
        >
          <div>
            <span style={{ fontSize: 13, fontWeight: 500, color: "var(--text-secondary)" }}>
              {t(`settings.f_${field.key}`, field.label)}{" "}
              <span style={{ fontSize: 11, color: "var(--text-tertiary)", fontWeight: 400 }}>
                ({field.key})
              </span>
            </span>
          </div>
          <Toggle active={!!value} onChange={(v) => onChange(v || undefined)} />
        </div>
      );

    case "select":
      return (
        <div>
          <FieldLabel field={field} t={t} />
          <select
            className="input"
            value={value ?? ""}
            onChange={(e) => onChange(e.target.value || undefined)}
          >
            <option value="">—</option>
            {field.options?.map((opt) => (
              <option key={opt} value={opt}>
                {opt}
              </option>
            ))}
          </select>
        </div>
      );

    case "json":
      return (
        <div>
          <FieldLabel field={field} t={t} />
          <JsonEditor value={value} onChange={onChange} placeholder="{}" />
        </div>
      );

    case "string[]":
      return (
        <div>
          <FieldLabel field={field} t={t} />
          <StringListEditor
            items={Array.isArray(value) ? value : []}
            onChange={(list) => onChange(list.length > 0 ? list : undefined)}
            addLabel={t("settings.addRule")}
          />
        </div>
      );

    case "string":
    default:
      return (
        <div>
          <FieldLabel field={field} t={t} />
          <input
            className="input"
            placeholder={field.placeholder}
            value={value ?? ""}
            onChange={(e) => onChange(e.target.value || undefined)}
          />
        </div>
      );
  }
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

  useEffect(() => {
    const load = async () => {
      try {
        const result = await settingsApi.get("global", CONFIG_KEY);
        const stored = result as Record<string, any> | null | undefined;
        // 若从未存储过，默认填入推荐配置
        const data = stored && Object.keys(stored).length > 0 ? stored : { ...RECOMMENDED_CONFIG };
        setConfig(data);
        setEditJson(JSON.stringify(data, null, 2));
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

  const handleSave = async () => {
    setSaving(true);
    setSaveError("");
    try {
      const value =
        mode === "json" ? JSON.parse(editJson) : { ...config };
      await settingsApi.set("global", CONFIG_KEY, value);
      setConfig(value);
      setEditJson(JSON.stringify(value, null, 2));
      setToast(t("settings.saved"));
      setTimeout(() => setToast(""), 2000);
    } catch (e: any) {
      setSaveError(e.toString());
    }
    setSaving(false);
  };

  const handleLoadRecommended = () => {
    const merged = { ...RECOMMENDED_CONFIG, ...config };
    setConfig(merged);
    setEditJson(JSON.stringify(merged, null, 2));
    setToast(t("settings.loadedRecommended"));
    setTimeout(() => setToast(""), 2000);
  };

  // Permissions helpers for the special permissions sub-editor
  const perms = (config.permissions ?? {}) as Record<string, string[]>;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 20,
        maxWidth: 720,
        width: "100%",
      }}
    >
      {/* Header */}
      <div className="section-header">
        <div>
          <div className="section-title">{t("settings.title")}</div>
          <div className="section-desc">{t("settings.desc")}</div>
        </div>
      </div>

      <div
        className="glass-surface"
        style={{
          padding: 16,
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}
      >
        {/* Mode toggle + Load Recommended */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            borderBottom: "1px solid var(--border)",
            paddingBottom: 8,
          }}
        >
          <div style={{ display: "flex", gap: 4 }}>
            <button
              className={`btn ${mode === "gui" ? "btn-primary" : "btn-ghost"}`}
              style={{ fontSize: 13, padding: "5px 12px" }}
              onClick={() => setMode("gui")}
            >
              {t("settings.guiMode")}
            </button>
            <button
              className={`btn ${mode === "json" ? "btn-primary" : "btn-ghost"}`}
              style={{ fontSize: 13, padding: "5px 12px" }}
              onClick={() => {
                setEditJson(JSON.stringify(config, null, 2));
                setMode("json");
              }}
            >
              {t("settings.jsonMode")}
            </button>
          </div>
          <button
            className="btn btn-ghost"
            style={{ fontSize: 12, padding: "4px 10px" }}
            onClick={handleLoadRecommended}
          >
            ⚡ {t("settings.loadRecommended")}
          </button>
        </div>

        {/* JSON mode */}
        {mode === "json" && (
          <textarea
            className="input"
            style={{
              fontFamily: '"SF Mono", "Fira Code", monospace',
              fontSize: 13,
              lineHeight: 1.6,
              minHeight: 480,
              resize: "vertical",
              whiteSpace: "pre",
            }}
            value={editJson}
            onChange={(e) => setEditJson(e.target.value)}
            spellCheck={false}
          />
        )}

        {/* GUI mode — schema-driven sections */}
        {mode === "gui" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {SECTIONS.map((section) => {
              // Special handling for permissions section: use sub-editors
              if (section.id === "permissions") {
                return (
                  <Section
                    key={section.id}
                    title={t(section.labelKey)}
                    defaultOpen
                  >
                    {/* Default Mode */}
                    <div>
                      <label
                        style={{
                          display: "block",
                          fontSize: 13,
                          fontWeight: 500,
                          color: "var(--text-secondary)",
                          marginBottom: 4,
                        }}
                      >
                        {t("settings.permissionsDefaultMode")}{" "}
                        <span style={{ fontSize: 11, color: "var(--text-tertiary)", fontWeight: 400 }}>
                          (permissions.defaultMode)
                        </span>
                      </label>
                      <select
                        className="input"
                        value={perms.defaultMode ?? ""}
                        onChange={(e) => {
                          const next: Record<string, any> = {};
                          if (perms.allow?.length) next.allow = perms.allow;
                          if (perms.ask?.length) next.ask = perms.ask;
                          if (perms.deny?.length) next.deny = perms.deny;
                          if (e.target.value) next.defaultMode = e.target.value;
                          updateField(
                            "permissions",
                            Object.keys(next).length > 0 ? next : undefined,
                          );
                        }}
                      >
                        <option value="">—</option>
                        <option value="default">default</option>
                        <option value="plan">plan</option>
                        <option value="auto">auto</option>
                        <option value="acceptEdits">acceptEdits</option>
                        <option value="dontAsk">dontAsk</option>
                        <option value="bypassPermissions">bypassPermissions</option>
                      </select>
                    </div>

                    {/* Allow */}
                    <div>
                      <div style={{ fontSize: 12, fontWeight: 500, color: "var(--text-tertiary)", marginBottom: 4 }}>
                        {t("settings.permissionsAllow")} <span style={{ fontSize: 11 }}>(permissions.allow)</span>
                      </div>
                      <StringListEditor
                        items={perms.allow ?? []}
                        onChange={(list) => {
                          const next: Record<string, any> = {};
                          if (perms.ask?.length) next.ask = perms.ask;
                          if (perms.deny?.length) next.deny = perms.deny;
                          if (perms.defaultMode) next.defaultMode = perms.defaultMode;
                          if (list.length > 0) next.allow = list;
                          updateField(
                            "permissions",
                            Object.keys(next).length > 0 ? next : undefined,
                          );
                        }}
                        addLabel={t("settings.addRule")}
                      />
                    </div>

                    {/* Ask */}
                    <div>
                      <div style={{ fontSize: 12, fontWeight: 500, color: "var(--text-tertiary)", marginBottom: 4 }}>
                        {t("settings.permissionsAsk")} <span style={{ fontSize: 11 }}>(permissions.ask)</span>
                      </div>
                      <StringListEditor
                        items={perms.ask ?? []}
                        onChange={(list) => {
                          const next: Record<string, any> = {};
                          if (perms.allow?.length) next.allow = perms.allow;
                          if (perms.deny?.length) next.deny = perms.deny;
                          if (perms.defaultMode) next.defaultMode = perms.defaultMode;
                          if (list.length > 0) next.ask = list;
                          updateField(
                            "permissions",
                            Object.keys(next).length > 0 ? next : undefined,
                          );
                        }}
                        addLabel={t("settings.addRule")}
                      />
                    </div>

                    {/* Deny */}
                    <div>
                      <div style={{ fontSize: 12, fontWeight: 500, color: "var(--text-tertiary)", marginBottom: 4 }}>
                        {t("settings.permissionsDeny")} <span style={{ fontSize: 11 }}>(permissions.deny)</span>
                      </div>
                      <StringListEditor
                        items={perms.deny ?? []}
                        onChange={(list) => {
                          const next: Record<string, any> = {};
                          if (perms.allow?.length) next.allow = perms.allow;
                          if (perms.ask?.length) next.ask = perms.ask;
                          if (perms.defaultMode) next.defaultMode = perms.defaultMode;
                          if (list.length > 0) next.deny = list;
                          updateField(
                            "permissions",
                            Object.keys(next).length > 0 ? next : undefined,
                          );
                        }}
                        addLabel={t("settings.addRule")}
                      />
                    </div>
                  </Section>
                );
              }

              // Special handling for env: use KvEditor
              if (section.id === "env") {
                return (
                  <Section
                    key={section.id}
                    title={t(section.labelKey)}
                    defaultOpen
                  >
                    <KvEditor
                      items={(config.env ?? {}) as Record<string, string>}
                      onChange={(newEnv) =>
                        updateField(
                          "env",
                          Object.keys(newEnv).length > 0 ? newEnv : undefined,
                        )
                      }
                    />
                  </Section>
                );
              }

              // Default: render each field in section
              return (
                <Section
                  key={section.id}
                  title={t(section.labelKey)}
                  defaultOpen={section.id === "core"}
                >
                  {section.fields.map((field) => (
                    <FieldRenderer
                      key={field.key}
                      field={field}
                      value={config[field.key]}
                      onChange={(v) => updateField(field.key, v)}
                      t={t}
                    />
                  ))}
                </Section>
              );
            })}
          </div>
        )}

        {/* Error */}
        {saveError && (
          <div
            style={{
              fontSize: 13,
              wordBreak: "break-all",
              color: "#ff453a",
            }}
          >
            {saveError}
          </div>
        )}

        {/* Actions */}
        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            gap: 8,
            paddingTop: 8,
            borderTop: "1px solid var(--border)",
          }}
        >
          {toast && (
            <span
              style={{
                fontSize: 13,
                color: "#34c759",
                alignSelf: "center",
                marginRight: "auto",
              }}
            >
              {toast}
            </span>
          )}
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? t("status.loading") : t("action.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
