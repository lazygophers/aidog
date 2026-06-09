import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { settingsApi } from "../services/api";
import {
  SECTIONS,
  RECOMMENDED_CONFIG,
  type SettingField,
} from "../services/claude-settings-schema";

const CONFIG_KEY = "claude_code";

// ─── Design tokens (all derived from 16px base) ───

const F = {
  title: 18,        // section heading
  label: 16,        // field label
  body: 16,         // input / button / general text
  hint: 14,         // secondary / key-in-parens / description
  small: 13,        // arrow icon / error
} as const;

const S = {
  gap: 14,          // between fields
  row: 14,          // kv row gap
  pad: 20,          // surface padding
  inputPad: "8px 12px",
  btnPad: "6px 14px",
  btnIcon: 30,      // icon button size
} as const;

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
        paddingTop: S.gap,
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
            fontSize: F.title,
            fontWeight: 600,
            color: "var(--text-primary)",
          }}
        >
          {title}
        </span>
        <span
          style={{
            fontSize: F.small,
            color: "var(--text-tertiary)",
            transition: "transform 0.15s",
            transform: open ? "rotate(90deg)" : "rotate(0deg)",
          }}
        >
          ▶
        </span>
      </div>
      {open && (
        <div style={{ display: "flex", flexDirection: "column", gap: S.gap, marginTop: S.gap }}>
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
        fontSize: F.label,
        fontWeight: 500,
        color: "var(--text-secondary)",
        marginBottom: 6,
      }}
    >
      {translated}{" "}
      <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400 }}>
        ({field.key})
      </span>
      {field.description && (
        <span style={{ fontWeight: 400, marginLeft: 8, fontSize: F.hint, color: "var(--text-tertiary)" }}>
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
          fontSize: F.body,
          lineHeight: 1.6,
          minHeight: rows * 24,
          resize: "vertical",
          whiteSpace: "pre",
          padding: S.inputPad,
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
        <span style={{ fontSize: F.small, color: "#ff453a" }}>{error}</span>
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
    <div style={{ display: "flex", flexDirection: "column", gap: S.row }}>
      {entries.map(([k, v]) => (
        <div key={k} style={{ display: "flex", gap: 6 }}>
          <input
            className="input"
            style={{ flex: 2, fontSize: F.body, padding: S.inputPad }}
            value={k}
            readOnly
          />
          <input
            className="input"
            style={{ flex: 3, fontSize: F.body, padding: S.inputPad }}
            value={v}
            onChange={(e) => onChange({ ...items, [k]: e.target.value })}
          />
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
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
      <div style={{ display: "flex", gap: 6 }}>
        <input
          className="input"
          style={{ flex: 2, fontSize: F.body, padding: S.inputPad }}
          placeholder="KEY"
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
        />
        <input
          className="input"
          style={{ flex: 3, fontSize: F.body, padding: S.inputPad }}
          placeholder="VALUE"
          value={newVal}
          onChange={(e) => setNewVal(e.target.value)}
        />
        <button
          type="button"
          className="btn btn-ghost"
          style={{ fontSize: F.body, padding: S.btnPad }}
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
    <div style={{ display: "flex", flexDirection: "column", gap: S.row }}>
      {items.map((item, i) => (
        <div key={i} style={{ display: "flex", gap: 6 }}>
          <input
            className="input"
            style={{ flex: 1, fontSize: F.body, padding: S.inputPad }}
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
            style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
            onClick={() => onChange(items.filter((_, j) => j !== i))}
          >
            ×
          </button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6 }}>
        <input
          className="input"
          style={{ flex: 1, fontSize: F.body, padding: S.inputPad }}
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
          style={{ fontSize: F.body, padding: S.btnPad }}
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
            <span style={{ fontSize: F.label, fontWeight: 500, color: "var(--text-secondary)" }}>
              {t(`settings.f_${field.key}`, field.label)}{" "}
              <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400 }}>
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
            style={{ fontSize: F.body, padding: S.inputPad }}
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

    case "kv":
      return (
        <div>
          <FieldLabel field={field} t={t} />
          <KvEditor
            items={(value && typeof value === "object" && !Array.isArray(value)) ? value as Record<string, string> : {}}
            onChange={(kv) => onChange(Object.keys(kv).length > 0 ? kv : undefined)}
          />
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
            style={{ fontSize: F.body, padding: S.inputPad }}
            placeholder={field.placeholder}
            value={value ?? ""}
            onChange={(e) => onChange(e.target.value || undefined)}
            list={field.options?.length ? `dl-${field.key}` : undefined}
          />
          {field.options?.length && (
            <datalist id={`dl-${field.key}`}>
              {field.options.map((opt) => (
                <option key={opt} value={opt} />
              ))}
            </datalist>
          )}
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
        gap: 24,
        maxWidth: 780,
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
          padding: S.pad,
          display: "flex",
          flexDirection: "column",
          gap: S.gap,
        }}
      >
        {/* Mode toggle + Load Recommended */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            borderBottom: "1px solid var(--border)",
            paddingBottom: 10,
          }}
        >
          <div style={{ display: "flex", gap: 6 }}>
            <button
              className={`btn ${mode === "gui" ? "btn-primary" : "btn-ghost"}`}
              style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={() => setMode("gui")}
            >
              {t("settings.guiMode")}
            </button>
            <button
              className={`btn ${mode === "json" ? "btn-primary" : "btn-ghost"}`}
              style={{ fontSize: F.body, padding: S.btnPad }}
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
            style={{ fontSize: F.hint, padding: "5px 12px" }}
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
              fontSize: F.body,
              lineHeight: 1.6,
              minHeight: 520,
              resize: "vertical",
              whiteSpace: "pre",
              padding: S.inputPad,
            }}
            value={editJson}
            onChange={(e) => setEditJson(e.target.value)}
            spellCheck={false}
          />
        )}

        {/* GUI mode — schema-driven sections */}
        {mode === "gui" && (
          <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
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
                          fontSize: F.label,
                          fontWeight: 500,
                          color: "var(--text-secondary)",
                          marginBottom: 6,
                        }}
                      >
                        {t("settings.permissionsDefaultMode")}{" "}
                        <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400 }}>
                          (permissions.defaultMode)
                        </span>
                      </label>
                      <select
                        className="input"
                        style={{ fontSize: F.body, padding: S.inputPad }}
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
                      <div style={{ fontSize: F.hint, fontWeight: 500, color: "var(--text-tertiary)", marginBottom: 6 }}>
                        {t("settings.permissionsAllow")} <span style={{ fontSize: F.hint }}>(permissions.allow)</span>
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
                      <div style={{ fontSize: F.hint, fontWeight: 500, color: "var(--text-tertiary)", marginBottom: 6 }}>
                        {t("settings.permissionsAsk")} <span style={{ fontSize: F.hint }}>(permissions.ask)</span>
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
                      <div style={{ fontSize: F.hint, fontWeight: 500, color: "var(--text-tertiary)", marginBottom: 6 }}>
                        {t("settings.permissionsDeny")} <span style={{ fontSize: F.hint }}>(permissions.deny)</span>
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
              fontSize: F.body,
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
            gap: 10,
            paddingTop: 10,
            borderTop: "1px solid var(--border)",
          }}
        >
          {toast && (
            <span
              style={{
                fontSize: F.body,
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
            style={{ fontSize: F.body }}
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
