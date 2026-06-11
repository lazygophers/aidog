import { useState, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { settingsApi, statuslineApi, claudeSettingsImportApi } from "../services/api";
import {
  SECTIONS,
  RECOMMENDED_CONFIG,
  ENV_VAR_DEFS,
  ENV_VAR_GROUP_ORDER,
  type SettingField,
  type EnvVarDef,
} from "../services/claude-settings-schema";

const CONFIG_KEY = "claude_code";

// ─── Design tokens ───

const F = {
  title: 20,        // section heading
  label: 15,        // field label
  body: 15,         // input / button / general text
  hint: 13,         // secondary / key-in-parens / description
  small: 12,        // arrow icon / error
} as const;

const S = {
  sectionGap: 20,   // between section cards
  gap: 18,          // between fields within a section
  row: 12,          // kv row gap
  pad: 28,          // card padding
  inputPad: "10px 14px",
  btnPad: "8px 18px",
  btnIcon: 34,      // icon button size
} as const;

// ─── Inline SVG Icons ──────────────────────────────────────

/** 16×16 inline SVG icons — replace all emojis for consistent rendering */
function SvgIcon({ d, size = 16, stroke = "currentColor", fill = "none", strokeWidth = 1.5, style }: {
  d: string; size?: number; stroke?: string; fill?: string; strokeWidth?: number;
  style?: React.CSSProperties;
}) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill={fill}
      stroke={stroke} strokeWidth={strokeWidth} strokeLinecap="round" strokeLinejoin="round"
      style={{ flexShrink: 0, ...style }}>
      <path d={d} />
    </svg>
  );
}

const ICON_PATHS: Record<string, string> = {
  // Sidebar section icons
  core: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z",
  behavior: "M12 2a8 8 0 0 0-8 8c0 3.4 2.1 6.3 5 7.5V20h6v-2.5c2.9-1.2 5-4.1 5-7.5a8 8 0 0 0-8-8Z M9 22h6 M10 2v2 M14 2v2 M9.5 14l2-3 2 3 M12 11v4",
  ui: "M2 3h20v14H2z M8 21h8 M12 17v4",
  team: "M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2 M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z M23 21v-2a4 4 0 0 0-3-3.87 M16 3.13a4 4 0 0 1 0 7.75",
  permissions: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z M9 12l2 2 4-4",
  env: "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Z M2 12h20 M12 2a15 15 0 0 1 4 10 15 15 0 0 1-4 10 15 15 0 0 1-4-10A15 15 0 0 1 12 2Z",
  hooks: "M18 4a3 3 0 0 0-3 3v4a3 3 0 0 0 6 0V7a3 3 0 0 0-3-3Z M15 11a3 3 0 1 0 3 3 M3 7a3 3 0 0 1 6 0",
  plugins: "M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82Z M7 7h.01",
  sandbox: "M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z M3.27 6.96 12 12.01l8.73-5.05 M12 22.08V12",
  attribution: "M12 20h9 M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z",
  status: "M18 20V10 M12 20V4 M6 20v-6",
  network: "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2Z M2 12h20 M12 2c2.5 3 4 6.5 4 10s-1.5 7-4 10c-2.5-3-4-6.5-4-10s1.5-7 4-10Z",
  memory: "M4 4h16v16H4z M4 9h16 M9 4v16",
  worktree: "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z M12 11v6 M9 14h6",
  advanced: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z",
  // Other icons
  folder: "M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2Z",
  file: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z M14 2v6h6",
  bolt: "M13 2L3 14h9l-1 8 10-12h-9l1-8Z",
};

/** Render named SVG icon (16×16 by default) */
function SectionIcon({ name, size = 16, style }: { name: string; size?: number; style?: React.CSSProperties }) {
  const d = ICON_PATHS[name];
  if (!d) return <SvgIcon d={ICON_PATHS.core} size={size} style={style} />;
  return <SvgIcon d={d} size={size} style={style} />;
}

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

/** Collapsible section — own card */
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
      className="glass-surface"
      style={{
        padding: S.pad,
        borderRadius: "var(--radius-lg)",
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
            letterSpacing: "-0.01em",
          }}
        >
          {title}
        </span>
        <span
          style={{
            fontSize: F.small,
            color: "var(--text-tertiary)",
            transition: "transform 0.2s cubic-bezier(0.4, 0, 0.2, 1)",
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

/** Label cell for left-right layout */
function FieldLabel({ field, t, style }: { field: SettingField; t: ReturnType<typeof useTranslation>["t"]; style?: React.CSSProperties }) {
  const translated = t(`settings.f_${field.key}`, field.label);
  return (
    <label
      style={{
        flexShrink: 0,
        width: 200,
        fontSize: F.label,
        fontWeight: 500,
        color: "var(--text-primary)",
        lineHeight: 1.5,
        paddingTop: 10,
        ...style,
      }}
    >
      {translated}
      <span style={{ display: "block", fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginTop: 3, fontFamily: '"SF Mono", "Fira Code", monospace' }}>
        {field.key}
      </span>
      {field.description && (
        <span style={{ display: "block", fontWeight: 400, fontSize: F.hint, color: "var(--text-tertiary)", marginTop: 3, lineHeight: 1.5 }}>
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

// ─── Env Var Editor (structured) ────────────────────────────

/** Parse env boolean: "1"/"true"/"yes"/"on" → true */
function envBool(v: string | undefined): boolean {
  if (!v) return false;
  return ["1", "true", "yes", "on"].includes(v.toLowerCase());
}

/** Label width constant for symmetric layout */
const ENV_LABEL_W = 220;

/** Styled env var row — symmetric label | control */
function EnvVarRow({ def, value, onChange, t }: {
  def: EnvVarDef;
  value: string | undefined;
  onChange: (v: string | undefined) => void;
  t: (key: string, fallback: string) => string;
}) {
  const { key, type, options, placeholder, min, max } = def;
  const label = t(`env.${key}`, def.label);
  const desc = t(`env.${key}.desc`, def.description ?? "");
  const isSet = value !== undefined && value !== "";

  const removeBtn = (
    <button type="button" className="btn btn-ghost btn-icon"
      style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.small, color: "var(--text-tertiary)" }}
      onClick={() => onChange(undefined)} title={t("action.remove", "Remove")}>×</button>
  );

  const renderControl = () => {
    switch (type) {
      case "boolean":
        return (
          <div style={{ display: "flex", alignItems: "center", gap: 8, justifyContent: "flex-end" }}>
            {isSet && removeBtn}
            <Toggle active={envBool(value)} onChange={(v) => onChange(v ? "1" : "0")} />
          </div>
        );
      case "select": {
        const opts = options ?? [];
        return (
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <select className="input" style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
              value={value ?? ""} onChange={(e) => onChange(e.target.value || undefined)}>
              <option value="">—</option>
              {opts.map((o) => <option key={o} value={o}>{o}</option>)}
            </select>
            {isSet && removeBtn}
          </div>
        );
      }
      case "number":
        return (
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <input className="input" type="number"
              style={{ fontSize: F.body, padding: S.inputPad, width: 160 }}
              placeholder={placeholder} value={value ?? ""} min={min} max={max}
              onChange={(e) => onChange(e.target.value || undefined)} />
            {isSet && removeBtn}
          </div>
        );
      case "password": {
        const [show, setShow] = useState(false);
        return (
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <input className="input" type={show ? "text" : "password"}
              style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
              placeholder={placeholder} value={value ?? ""}
              onChange={(e) => onChange(e.target.value || undefined)} />
            <button type="button" className="btn btn-ghost btn-icon"
              style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon }}
              onClick={() => setShow(!show)}>
              <SvgIcon d={show
                ? "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8Z M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z"
                : "M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19M1 1l22 22"} size={14} />
            </button>
            {isSet && removeBtn}
          </div>
        );
      }
      case "string":
      default:
        return (
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
              placeholder={placeholder} value={value ?? ""}
              onChange={(e) => onChange(e.target.value || undefined)} />
            {isSet && removeBtn}
          </div>
        );
    }
  };

  return (
    <div style={{ display: "grid", gridTemplateColumns: `${ENV_LABEL_W}px 1fr`, alignItems: "start", gap: 12 }}>
      <div style={{ paddingTop: 10 }}>
        <div style={{ fontSize: F.label, fontWeight: 500, color: "var(--text-primary)", lineHeight: 1.4 }}>{label}</div>
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", fontFamily: '"SF Mono", "Fira Code", monospace', marginTop: 2 }}>{key}</div>
        {desc && <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: 3, lineHeight: 1.5 }}>{desc}</div>}
      </div>
      <div style={{ paddingTop: 10 }}>
        {renderControl()}
      </div>
    </div>
  );
}

/** Group heading separator */
function EnvGroupHeading({ label }: { label: string }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12, paddingTop: 16, paddingBottom: 4 }}>
      <span style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{label}</span>
      <div style={{ flex: 1, height: 1, background: "var(--border)" }} />
    </div>
  );
}

/** Structured env var editor — search + dedicated UI + i18n + symmetric layout */
function EnvEditor({ env, onChange, t }: {
  env: Record<string, string>;
  onChange: (env: Record<string, string>) => void;
  t: (key: string, fallback: string) => string;
}) {
  const knownKeys = useMemo(() => new Set(ENV_VAR_DEFS.map(d => d.key)), []);
  const [showAddMenu, setShowAddMenu] = useState(false);
  const [customKey, setCustomKey] = useState("");
  const [customVal, setCustomVal] = useState("");
  const [search, setSearch] = useState("");

  const lowerSearch = search.toLowerCase();

  const knownDefs = useMemo(() => ENV_VAR_DEFS.filter(d => d.key in env), [env]);
  const unknownEntries = useMemo(() => Object.entries(env).filter(([k]) => !knownKeys.has(k)), [env, knownKeys]);
  const addableDefs = useMemo(() => ENV_VAR_DEFS.filter(d => !(d.key in env)), [env]);

  const updateEnv = useCallback((key: string, value: string | undefined) => {
    onChange(
      value !== undefined && value !== ""
        ? { ...env, [key]: value }
        : Object.fromEntries(Object.entries(env).filter(([k]) => k !== key)),
    );
  }, [env, onChange]);

  /** Filter defs by search query (match key, label, description, i18n label) */
  const filterDefs = (defs: EnvVarDef[]) => {
    if (!lowerSearch) return defs;
    return defs.filter(d => {
      const i18nLabel = t(`env.${d.key}`, d.label);
      const i18nDesc = t(`env.${d.key}.desc`, d.description ?? "");
      return d.key.toLowerCase().includes(lowerSearch)
        || i18nLabel.toLowerCase().includes(lowerSearch)
        || i18nDesc.toLowerCase().includes(lowerSearch)
        || (d.label ?? "").toLowerCase().includes(lowerSearch);
    });
  };

  const grouped = useMemo(() =>
    ENV_VAR_GROUP_ORDER
      .map(g => ({ group: g, defs: filterDefs(knownDefs.filter(d => d.group === g)) }))
      .filter(g => g.defs.length > 0),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [knownDefs, lowerSearch],
  );

  const filteredUnknown = lowerSearch
    ? unknownEntries.filter(([k, v]) => k.toLowerCase().includes(lowerSearch) || v.toLowerCase().includes(lowerSearch))
    : unknownEntries;

  const hasResults = grouped.some(g => g.defs.length > 0) || filteredUnknown.length > 0;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {/* Search bar */}
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <div style={{ position: "relative", flex: 1 }}>
          <SvgIcon d="M11 3a8 8 0 1 0 0 16 8 8 0 0 0 0-16Z M21 21l-4.35-4.35" size={14}
            style={{ position: "absolute", left: 10, top: "50%", transform: "translateY(-50%)", color: "var(--text-tertiary)" }} />
          <input className="input" style={{ fontSize: F.body, padding: S.inputPad, paddingLeft: 32, width: "100%" }}
            placeholder={t("env.searchPlaceholder", "Search environment variables…")}
            value={search} onChange={(e) => setSearch(e.target.value)} />
          {search && (
            <button type="button" style={{
              position: "absolute", right: 6, top: "50%", transform: "translateY(-50%)",
              background: "none", border: "none", cursor: "pointer", color: "var(--text-tertiary)", fontSize: 14,
            }} onClick={() => setSearch("")}>×</button>
          )}
        </div>
      </div>

      {!hasResults && search && (
        <div style={{ padding: 20, textAlign: "center", color: "var(--text-tertiary)", fontSize: F.body }}>
          {t("env.noResults", "No matching environment variables")}
        </div>
      )}

      {/* Known env var groups */}
      {grouped.map(({ group, defs }) => (
        <div key={group}>
          <EnvGroupHeading label={t(`env.group.${group}`, group)} />
          <div style={{ display: "flex", flexDirection: "column", gap: S.row }}>
            {defs.map(def => (
              <EnvVarRow key={def.key} def={def} value={env[def.key]}
                onChange={(v) => updateEnv(def.key, v)} t={t} />
            ))}
          </div>
        </div>
      ))}

      {/* Unknown / custom env vars */}
      {filteredUnknown.length > 0 && (
        <div>
          <EnvGroupHeading label={t("env.group.custom", "Custom Variables")} />
          <div style={{ display: "flex", flexDirection: "column", gap: S.row }}>
            {filteredUnknown.map(([k, v]) => (
              <div key={k} style={{ display: "grid", gridTemplateColumns: `${ENV_LABEL_W}px 1fr`, alignItems: "center", gap: 12 }}>
                <input className="input" style={{ fontSize: F.body, padding: S.inputPad }} value={k} readOnly />
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <input className="input" style={{ flex: 1, fontSize: F.body, padding: S.inputPad }} value={v}
                    onChange={(e) => updateEnv(k, e.target.value)} />
                  <button type="button" className="btn btn-ghost btn-icon"
                    style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
                    onClick={() => updateEnv(k, undefined)}>×</button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Add variable — right-aligned */}
      {!search && (
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          {/* Custom variable add */}
          <div style={{ display: "flex", gap: 6 }}>
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad, width: 120 }}
              placeholder="KEY" value={customKey} onChange={(e) => setCustomKey(e.target.value)} />
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad, width: 120 }}
              placeholder="VALUE" value={customVal} onChange={(e) => setCustomVal(e.target.value)} />
            <button className="btn btn-ghost" style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={() => {
                if (customKey.trim()) { updateEnv(customKey.trim(), customVal); setCustomKey(""); setCustomVal(""); }
              }}>
              {t("env.addCustom", "+ Custom")}
            </button>
          </div>

          {/* Add known dropdown */}
          {addableDefs.length > 0 && (
            <div style={{ position: "relative" }}>
              <button className="btn btn-ghost" style={{ fontSize: F.body, padding: S.btnPad }}
                onClick={() => setShowAddMenu(!showAddMenu)}>
                {t("env.addKnown", "+ Add Known")}
              </button>
              {showAddMenu && (
                <div style={{
                  position: "absolute", bottom: "100%", right: 0, zIndex: 100,
                  background: "var(--bg-surface)", border: "1px solid var(--border)",
                  borderRadius: "var(--radius-md)", padding: 4,
                  maxHeight: 360, overflow: "auto", minWidth: 340,
                  boxShadow: "0 8px 32px rgba(0,0,0,0.2)",
                }}>
                  {ENV_VAR_GROUP_ORDER.map(g => {
                    const defs = addableDefs.filter(d => d.group === g);
                    if (defs.length === 0) return null;
                    return (
                      <div key={g}>
                        <div style={{ fontSize: F.hint, fontWeight: 600, color: "var(--text-tertiary)", padding: "6px 10px 2px" }}>
                          {t(`env.group.${g}`, g)}
                        </div>
                        {defs.map(d => (
                          <button key={d.key} style={{
                            display: "block", width: "100%", textAlign: "left",
                            padding: "6px 10px", fontSize: F.body,
                            background: "transparent", border: "none", borderRadius: "var(--radius-sm)",
                            cursor: "pointer", color: "var(--text-primary)",
                          }}
                            onMouseEnter={(e) => { e.currentTarget.style.background = "var(--bg-glass)"; }}
                            onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
                            onClick={() => {
                              const defaultVal = d.type === "boolean" ? "1" : d.type === "select" ? (d.options?.[0] ?? "") : "";
                              updateEnv(d.key, defaultVal || "1");
                              setShowAddMenu(false);
                            }}
                          >
                            <span style={{ fontWeight: 500 }}>{t(`env.${d.key}`, d.label)}</span>
                            <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginLeft: 8, fontFamily: '"SF Mono", "Fira Code", monospace' }}>
                              {d.key}
                            </span>
                          </button>
                        ))}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          )}
        </div>
      )}
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

// ─── Permissions Section (extracted for hooks compliance) ───

type RuleMode = "allow" | "ask" | "deny";

const MODE_COLORS: Record<RuleMode, string> = {
  allow: "#34c759",
  ask: "#ff9f0a",
  deny: "#ff453a",
};

const PERMISSION_MODES: { value: string; desc: string; hint: string }[] = [
  { value: "default", desc: "标准模式", hint: "首次使用每个工具时提示权限" },
  { value: "acceptEdits", desc: "接受编辑", hint: "自动接受工作目录内的文件编辑和常见文件系统命令" },
  { value: "plan", desc: "计划模式", hint: "只读 — 读取文件和只读命令，不编辑源文件" },
  { value: "auto", desc: "自动模式", hint: "自动批准 + 后台安全检查（研究预览）" },
  { value: "dontAsk", desc: "不再询问", hint: "未预先批准的工具自动拒绝" },
  { value: "bypassPermissions", desc: "跳过权限", hint: "跳过所有权限提示（根目录删除仍会提示）" },
];

/** Tool categories with syntax hints and template examples — aligned with permissions docs */
const TOOL_GROUPS: { tool: string; label: string; syntax: string; examples: string[] }[] = [
  { tool: "Bash", label: "Bash / Shell", syntax: "Bash(cmd) / Bash(prefix *) / Bash", examples: [
    "Bash(npm run build)", "Bash(npm run *)", "Bash(git commit *)", "Bash(git * main)",
    "Bash(docker *)", "Bash(* --version)", "Bash",
  ] },
  { tool: "PowerShell", label: "PowerShell", syntax: "PowerShell(cmd) / PowerShell(prefix *) / PowerShell", examples: [
    "PowerShell(Get-ChildItem *)", "PowerShell(git commit *)", "PowerShell",
  ] },
  { tool: "Read", label: "Read", syntax: "Read(path) — //绝对 / ~/主目录 / /项目根 / ./当前", examples: [
    "Read(./.env)", "Read(//**/*.key)", "Read(~/.ssh/**)", "Read(src/**)", "Read(**/.env)",
  ] },
  { tool: "Edit", label: "Edit / Write", syntax: "Edit(path) — 同 Read 路径规则", examples: [
    "Edit(/src/**/*.ts)", "Edit(./config.json)", "Edit(/docs/**)",
  ] },
  { tool: "WebFetch", label: "WebFetch", syntax: "WebFetch(domain:host) / WebFetch", examples: [
    "WebFetch(domain:example.com)", "WebFetch",
  ] },
  { tool: "mcp__", label: "MCP", syntax: "mcp__server__tool / mcp__server__*", examples: [
    "mcp__puppeteer__*", "mcp__puppeteer__puppeteer_navigate",
  ] },
  { tool: "Agent", label: "Agent (子代理)", syntax: "Agent(name)", examples: [
    "Agent(Explore)", "Agent(Plan)", "Agent(my-custom-agent)",
  ] },
];

/** Detect which tool group a rule pattern belongs to */
function ruleToolGroup(pattern: string): string {
  if (pattern.startsWith("mcp__")) return "mcp__";
  const m = pattern.match(/^([A-Za-z_]+)/);
  return m ? m[1] : "";
}

/** Shared permissions logic — used by both PermissionsSection & PermissionsSectionInline */
function PermissionsEditor({ perms, updateField, t }: {
  perms: Record<string, any>;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const [draftRule, setDraftRule] = useState("");
  const [draftMode, setDraftMode] = useState<RuleMode>("allow");
  const [showTemplates, setShowTemplates] = useState(false);
  const [activeToolGroup, setActiveToolGroup] = useState<string>("Bash");

  // Flatten allow/ask/deny into unified rule list
  const rules: { pattern: string; mode: RuleMode }[] = [
    ...(perms.allow ?? []).map((p: string) => ({ pattern: p, mode: "allow" as RuleMode })),
    ...(perms.ask ?? []).map((p: string) => ({ pattern: p, mode: "ask" as RuleMode })),
    ...(perms.deny ?? []).map((p: string) => ({ pattern: p, mode: "deny" as RuleMode })),
  ];

  // Group rules by tool type
  const grouped = useMemo(() => {
    const map = new Map<string, { pattern: string; mode: RuleMode; idx: number }[]>();
    rules.forEach((r, idx) => {
      const group = ruleToolGroup(r.pattern);
      if (!map.has(group)) map.set(group, []);
      map.get(group)!.push({ ...r, idx });
    });
    return map;
  }, [rules]);

  const syncRules = (updated: { pattern: string; mode: RuleMode }[]) => {
    const next: Record<string, any> = {};
    if (perms.defaultMode) next.defaultMode = perms.defaultMode;
    if (perms.disableBypassPermissionsMode) next.disableBypassPermissionsMode = perms.disableBypassPermissionsMode;
    if (perms.disableAutoMode) next.disableAutoMode = perms.disableAutoMode;
    const allow = updated.filter(r => r.mode === "allow").map(r => r.pattern);
    const ask = updated.filter(r => r.mode === "ask").map(r => r.pattern);
    const deny = updated.filter(r => r.mode === "deny").map(r => r.pattern);
    if (allow.length) next.allow = allow;
    if (ask.length) next.ask = ask;
    if (deny.length) next.deny = deny;
    updateField("permissions", Object.keys(next).length > 0 ? next : undefined);
  };

  const updatePermKey = (key: string, value: any) => {
    const next: Record<string, any> = { ...perms };
    if (value) next[key] = value;
    else delete next[key];
    if (Object.keys(next).length === 0) updateField("permissions", undefined);
    else updateField("permissions", next);
  };

  const modeLabel = (m: RuleMode) =>
    t(`settings.permissions${m.charAt(0).toUpperCase() + m.slice(1)}`);

  const ALL_MODES: RuleMode[] = ["allow", "ask", "deny"];

  /** Styled mode dropdown — colored border + background per mode */
  const ModeSelect = ({ mode, onChange }: { mode: RuleMode; onChange: (m: RuleMode) => void }) => (
    <select
      className="input"
      value={mode}
      onChange={(e) => onChange(e.target.value as RuleMode)}
      style={{
        fontSize: F.small, fontWeight: 600, minWidth: 72,
        padding: "4px 8px", borderRadius: "var(--radius-sm)",
        background: `${MODE_COLORS[mode]}12`,
        color: MODE_COLORS[mode],
        border: `1px solid ${MODE_COLORS[mode]}35`,
        cursor: "pointer", outline: "none",
      }}
    >
      {ALL_MODES.map(m => (
        <option key={m} value={m}>{modeLabel(m)}</option>
      ))}
    </select>
  );

  const toolGroup = TOOL_GROUPS.find(g => g.tool === activeToolGroup) ?? TOOL_GROUPS[0];

  return (
    <>
      {/* ── Default Mode ── */}
      <FieldRow label={t("settings.permissionsDefaultMode")} icon={<SectionIcon name="permissions" size={14} />}>
        <select
          className="input"
          style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
          value={perms.defaultMode ?? ""}
          onChange={(e) => updatePermKey("defaultMode", e.target.value || undefined)}
        >
          <option value="">—</option>
          {PERMISSION_MODES.map(m => (
            <option key={m.value} value={m.value}>{m.desc} — {m.hint}</option>
          ))}
        </select>
      </FieldRow>
      <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.6, paddingLeft: 92 }}>
        规则优先级: <span style={{ color: MODE_COLORS.deny, fontWeight: 600 }}>deny</span> →{" "}
        <span style={{ color: MODE_COLORS.ask, fontWeight: 600 }}>ask</span> →{" "}
        <span style={{ color: MODE_COLORS.allow, fontWeight: 600 }}>allow</span>。第一个匹配的规则生效。
      </div>

      {/* ── Safety Toggles ── */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <FieldRow label="禁用绕过模式" icon={<SectionIcon name="bolt" size={14} />}>
          <div
            className={`toggle${perms.disableBypassPermissionsMode ? " active" : ""}`}
            onClick={() => updatePermKey("disableBypassPermissionsMode", perms.disableBypassPermissionsMode ? undefined : "disable")}
          />
          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>disableBypassPermissionsMode</span>
        </FieldRow>
        <FieldRow label="禁用自动模式" icon={<SectionIcon name="bolt" size={14} />}>
          <div
            className={`toggle${perms.disableAutoMode ? " active" : ""}`}
            onClick={() => updatePermKey("disableAutoMode", perms.disableAutoMode ? undefined : "disable")}
          />
          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>disableAutoMode</span>
        </FieldRow>
      </div>

      {/* ── Tool Group Tabs ── */}
      <div style={{ display: "flex", gap: 0, borderBottom: "1px solid var(--border)", flexShrink: 0 }}>
        {TOOL_GROUPS.map(g => {
          const count = grouped.get(g.tool)?.length ?? 0;
          const active = activeToolGroup === g.tool;
          return (
            <button key={g.tool} type="button"
              style={{
                padding: "6px 12px", fontSize: F.small, fontWeight: active ? 600 : 400,
                color: active ? "var(--accent)" : "var(--text-secondary)",
                background: "transparent", border: "none", borderBottom: active ? "2px solid var(--accent)" : "2px solid transparent",
                cursor: "pointer", display: "flex", alignItems: "center", gap: 4,
                transition: "all 150ms ease",
              }}
              onClick={() => setActiveToolGroup(g.tool)}
            >
              {g.label}
              {count > 0 && (
                <span style={{
                  fontSize: 10, padding: "1px 5px", borderRadius: 8,
                  background: active ? "var(--accent)" : "var(--bg-glass)",
                  color: active ? "#fff" : "var(--text-tertiary)", fontWeight: 600,
                }}>{count}</span>
              )}
            </button>
          );
        })}
      </div>

      {/* ── Syntax Hint for Active Group ── */}
      <div style={{
        fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.5,
        padding: "8px 12px", background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
        fontFamily: '"SF Mono", "Fira Code", monospace',
      }}>
        <span style={{ fontWeight: 600, color: "var(--accent)" }}>{toolGroup.label}</span>: {toolGroup.syntax}
      </div>

      {/* ── Rules for Active Group ── */}
      {(() => {
        const groupRules = grouped.get(activeToolGroup) ?? [];
        if (groupRules.length === 0) return (
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", padding: "12px 0", textAlign: "center" }}>
            暂无 {toolGroup.label} 规则。使用下方输入框添加。
          </div>
        );
        return (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {groupRules.map((rule) => (
              <div key={rule.idx} style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <input
                  className="input"
                  style={{ flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0, fontFamily: '"SF Mono", "Fira Code", monospace' }}
                  value={rule.pattern}
                  onChange={(e) => {
                    const updated = [...rules];
                    updated[rule.idx] = { ...updated[rule.idx], pattern: e.target.value };
                    syncRules(updated);
                  }}
                />
                <ModeSelect
                  mode={rule.mode}
                  onChange={(m) => {
                    const updated = [...rules];
                    updated[rule.idx] = { ...updated[rule.idx], mode: m };
                    syncRules(updated);
                  }}
                />
                <button type="button" className="btn btn-ghost btn-icon"
                  style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
                  onClick={() => syncRules(rules.filter((_, j) => j !== rule.idx))}
                >
                  ×
                </button>
              </div>
            ))}
          </div>
        );
      })()}

      {/* ── Add Rule ── */}
      <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
        <div style={{ position: "relative", flex: 1 }}>
          <input
            className="input"
            style={{ fontSize: F.body, padding: S.inputPad, width: "100%", paddingRight: 28, fontFamily: '"SF Mono", "Fira Code", monospace' }}
            placeholder={toolGroup.examples[0]}
            value={draftRule}
            onChange={(e) => setDraftRule(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && draftRule.trim()) {
                syncRules([...rules, { pattern: draftRule.trim(), mode: draftMode }]);
                setDraftRule("");
              }
            }}
          />
          <button type="button" className="btn btn-ghost btn-icon"
            style={{
              position: "absolute", right: 2, top: "50%", transform: "translateY(-50%)",
              width: 24, height: 24, minWidth: 24, padding: 0,
              color: showTemplates ? "var(--accent)" : "var(--text-tertiary)",
            }}
            onClick={() => setShowTemplates(!showTemplates)}
            title="规则模板"
          >
            <SectionIcon name="bolt" size={14} />
          </button>
          {showTemplates && (
            <>
              <div style={{ position: "fixed", inset: 0, zIndex: 99 }} onClick={() => setShowTemplates(false)} />
              <div className="glass-elevated"
                style={{
                  position: "absolute", top: "100%", left: 0, right: 0,
                  marginTop: 4, maxHeight: 300, overflowY: "auto",
                  zIndex: 100, padding: 10, animation: "fadeIn 150ms ease both",
                }}
              >
                {TOOL_GROUPS.map(g => (
                  <div key={g.tool} style={{ marginBottom: 8 }}>
                    <div style={{ fontSize: 12, fontWeight: 600, color: "var(--accent)", marginBottom: 4, display: "flex", alignItems: "center", gap: 4 }}>
                      {g.label}
                      <span style={{ fontSize: 10, color: "var(--text-tertiary)", fontWeight: 400, fontFamily: '"SF Mono", "Fira Code", monospace' }}>
                        {g.syntax}
                      </span>
                    </div>
                    <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
                      {g.examples.map(ex => (
                        <button key={ex} type="button" className="btn btn-ghost"
                          style={{
                            padding: "3px 8px", fontSize: 13, fontWeight: 400,
                            color: "var(--text-primary)", borderRadius: "var(--radius-sm)",
                            fontFamily: '"SF Mono", "Fira Code", monospace',
                          }}
                          onClick={() => { setDraftRule(ex); setShowTemplates(false); }}
                        >
                          {ex}
                        </button>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
        <ModeSelect mode={draftMode} onChange={setDraftMode} />
        <button type="button" className="btn btn-ghost"
          style={{ fontSize: F.body, padding: S.btnPad, width: S.btnIcon, minWidth: S.btnIcon }}
          onClick={() => {
            if (draftRule.trim()) {
              syncRules([...rules, { pattern: draftRule.trim(), mode: draftMode }]);
              setDraftRule("");
            }
          }}
        >
          +
        </button>
      </div>

      {/* ── All Rules Summary ── */}
      {rules.length > 0 && (
        <div style={{
          padding: "10px 12px", background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
          display: "flex", flexDirection: "column", gap: 4,
        }}>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 4, display: "flex", gap: 12 }}>
            <span>共 {rules.length} 条规则</span>
            <span style={{ color: MODE_COLORS.deny }}>✗ deny: {rules.filter(r => r.mode === "deny").length}</span>
            <span style={{ color: MODE_COLORS.ask }}>? ask: {rules.filter(r => r.mode === "ask").length}</span>
            <span style={{ color: MODE_COLORS.allow }}>✓ allow: {rules.filter(r => r.mode === "allow").length}</span>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
            {rules.map((r, i) => (
              <div key={i} style={{
                display: "flex", alignItems: "center", gap: 6,
                fontSize: F.small, padding: "3px 8px", borderRadius: "var(--radius-sm)",
                borderLeft: `3px solid ${MODE_COLORS[r.mode]}`,
                background: `${MODE_COLORS[r.mode]}08`,
              }}>
                <span style={{
                  fontSize: 10, fontWeight: 600, color: MODE_COLORS[r.mode],
                  textTransform: "uppercase", width: 32, flexShrink: 0,
                }}>{r.mode}</span>
                <code style={{
                  flex: 1, fontSize: F.small, color: "var(--text-primary)",
                  fontFamily: '"SF Mono", "Fira Code", monospace',
                  overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                }}>{r.pattern}</code>
                <span style={{ fontSize: 10, color: "var(--text-tertiary)" }}>
                  {ruleToolGroup(r.pattern)}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </>
  );
}

function PermissionsSection({
  perms,
  updateField,
  t,
}: {
  perms: Record<string, any>;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  return (
    <Section title={t("settings.sectionPermissions")} defaultOpen>
      <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
        <PermissionsEditor perms={perms} updateField={updateField} t={t} />
      </div>
    </Section>
  );
}

/** Permissions without Section wrapper — for tab content pane */
function PermissionsSectionInline({ perms, updateField, t }: {
  perms: Record<string, any>;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  return <PermissionsEditor perms={perms} updateField={updateField} t={t} />;
}

// ─── Sandbox Section (structured editor) ────────────────────

/** Editable string list with add/remove — plain text input */
function TagList({
  items,
  onChange,
  placeholder,
}: {
  items: string[];
  onChange: (v: string[]) => void;
  placeholder?: string;
}) {
  const [draft, setDraft] = useState("");
  const add = () => {
    const v = draft.trim();
    if (v && !items.includes(v)) onChange([...items, v]);
    setDraft("");
  };
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {items.map((p, i) => (
        <div key={i} style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <code style={{
            flex: 1, fontSize: F.hint, padding: "6px 10px",
            background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
            color: "var(--text-primary)", fontFamily: "monospace",
            overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
          }}>
            {p}
          </code>
          <button type="button" onClick={() => onChange(items.filter((_, j) => j !== i))}
            style={{
              background: "none", border: "none", cursor: "pointer",
              color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1,
            }}>✕</button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
        <input
          className="input"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={placeholder}
          onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); add(); } }}
          style={{ flex: 1, fontSize: F.hint, fontFamily: "monospace", padding: "6px 10px" }}
        />
        <button type="button" disabled={!draft.trim()} onClick={add}
          style={{
            background: "var(--accent)", color: "#fff", border: "none", borderRadius: "var(--radius-sm)",
            padding: "5px 10px", fontSize: F.hint, cursor: draft.trim() ? "pointer" : "default",
            opacity: draft.trim() ? 1 : 0.4,
          }}>+</button>
      </div>
    </div>
  );
}

/** Editable path list with add/remove — uses PathInput with directory picker + autocomplete */
function PathList({
  items,
  onChange,
  placeholder,
}: {
  items: string[];
  onChange: (v: string[]) => void;
  placeholder?: string;
}) {
  const [draft, setDraft] = useState<string | undefined>(undefined);
  const draftStr = draft ?? "";
  const add = () => {
    const v = draftStr.trim();
    if (v && !items.includes(v)) onChange([...items, v]);
    setDraft(undefined);
  };
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {items.map((p, i) => (
        <div key={i} style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <code style={{
            flex: 1, fontSize: F.hint, padding: "6px 10px",
            background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
            color: "var(--text-primary)", fontFamily: "monospace",
            overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
          }}>
            {p}
          </code>
          <button type="button" onClick={() => onChange(items.filter((_, j) => j !== i))}
            style={{
              background: "none", border: "none", cursor: "pointer",
              color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1,
            }}>✕</button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6, alignItems: "stretch" }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <PathInput
            value={draft}
            onChange={setDraft}
            pathType="directory"
            placeholder={placeholder ?? "选择目录或输入路径…"}
          />
        </div>
        <button type="button" disabled={!draftStr.trim()} onClick={add}
          style={{
            background: "var(--accent)", color: "#fff", border: "none", borderRadius: "var(--radius-sm)",
            padding: "5px 10px", fontSize: F.hint, cursor: draftStr.trim() ? "pointer" : "default",
            opacity: draftStr.trim() ? 1 : 0.4, flexShrink: 0,
          }}>+</button>
      </div>
    </div>
  );
}

/** Sub-section heading */
function SubHeading({ children }: { children: React.ReactNode }) {
  return (
    <div style={{
      fontSize: F.label, fontWeight: 600, color: "var(--text-primary)",
      borderBottom: "1px solid var(--border)", paddingBottom: 6, marginBottom: 4,
      display: "flex", alignItems: "center", gap: 6,
    }}>
      {children}
    </div>
  );
}

/** Inline hint text */
function Hint({ children }: { children: React.ReactNode }) {
  return <span style={{ fontSize: F.small, color: "var(--text-tertiary)", lineHeight: 1.4 }}>{children}</span>;
}

function SandboxEditor({
  sandboxValue,
  updateField,
}: {
  sandboxValue: Record<string, any> | undefined;
  updateField: (field: string, value: any) => void;
}) {
  const sb = sandboxValue ?? {};
  const fs = sb.filesystem ?? {};
  const net = sb.network ?? {};
  const enabled = !!sb.enabled;

  const sync = (patch: Record<string, any>) => {
    const next = { ...sb, ...patch };
    // Remove empty arrays and falsy booleans at top level
    for (const k of Object.keys(next)) {
      if (Array.isArray(next[k]) && next[k].length === 0) delete next[k];
      if (next[k] === false || next[k] === undefined) delete next[k];
    }
    // Clean empty sub-objects
    if (next.filesystem) {
      const fso = next.filesystem as Record<string, any>;
      for (const k of Object.keys(fso)) {
        if (Array.isArray(fso[k]) && fso[k].length === 0) delete fso[k];
      }
      if (Object.keys(fso).length === 0) delete next.filesystem;
    }
    if (next.network) {
      const no = next.network as Record<string, any>;
      for (const k of Object.keys(no)) {
        if (Array.isArray(no[k]) && no[k].length === 0) delete no[k];
        if (no[k] === false || no[k] === undefined) delete no[k];
      }
      if (Object.keys(no).length === 0) delete next.network;
    }
    updateField("sandbox", Object.keys(next).length > 0 ? next : undefined);
  };

  const toggleSb = (key: string, val: boolean) => {
    sync({ [key]: val });
  };

  const setFsArray = (key: string, arr: string[]) => {
    sync({ filesystem: { ...fs, [key]: arr } });
  };

  const setNetArray = (key: string, arr: string[]) => {
    sync({ network: { ...net, [key]: arr } });
  };

  const setNetPort = (key: string, val: string) => {
    const port = parseInt(val, 10);
    if (val && (isNaN(port) || port < 0 || port > 65535)) return;
    sync({ network: { ...net, [key]: val ? port : undefined } });
  };

  const setExcludedCommands = (arr: string[]) => {
    sync({ excludedCommands: arr });
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {/* ── Enable Toggle ── */}
      <div style={{
        display: "flex", alignItems: "center", gap: 12,
        padding: "12px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
      }}>
        <Toggle active={enabled} onChange={(v) => sync({ enabled: v })} />
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-primary)" }}>
            启用沙箱
          </div>
          <Hint>Bash 命令及其子进程的文件系统和网络隔离 (Seatbelt / bubblewrap)</Hint>
        </div>
        {enabled && (
          <span style={{
            fontSize: F.small, fontWeight: 600, color: "#34c759",
            padding: "2px 8px", background: "rgba(52,199,89,0.12)", borderRadius: "var(--radius-sm)",
          }}>● 已启用</span>
        )}
      </div>

      {!enabled && (
        <div style={{
          fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.6,
          padding: "10px 14px", background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
        }}>
          启用后，Claude 运行的每个 Bash 命令将被限制在指定的文件系统和网络边界内。
          macOS 使用 Seatbelt，Linux/WSL2 使用 bubblewrap。不支持原生 Windows。
        </div>
      )}

      {enabled && (
        <>
          {/* ── Filesystem Isolation ── */}
          <div style={{
            padding: "14px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
            display: "flex", flexDirection: "column", gap: 12,
          }}>
            <SubHeading>
              <SvgIcon d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" size={15} />
              文件系统隔离
            </SubHeading>
            <Hint>
              默认：可读整个文件系统，仅可写当前工作目录。路径前缀：/（绝对）、~/（主目录）、./（项目相对）
            </Hint>

            <FieldRow label="允许写入">
              <PathList
                items={fs.allowWrite ?? []}
                onChange={(v) => setFsArray("allowWrite", v)}
                placeholder="如 ~/.kube, /tmp/build"
              />
            </FieldRow>

            <FieldRow label="拒绝写入">
              <PathList
                items={fs.denyWrite ?? []}
                onChange={(v) => setFsArray("denyWrite", v)}
                placeholder="如 ~/.bashrc, /etc"
              />
            </FieldRow>

            <FieldRow label="允许读取">
              <PathList
                items={fs.allowRead ?? []}
                onChange={(v) => setFsArray("allowRead", v)}
                placeholder="如 .（项目目录）"
              />
            </FieldRow>

            <FieldRow label="拒绝读取">
              <PathList
                items={fs.denyRead ?? []}
                onChange={(v) => setFsArray("denyRead", v)}
                placeholder="如 ~/（阻止读主目录）, ~/.ssh"
              />
            </FieldRow>
          </div>

          {/* ── Network Isolation ── */}
          <div style={{
            padding: "14px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
            display: "flex", flexDirection: "column", gap: 12,
          }}>
            <SubHeading>
              <SvgIcon d="M12 2a10 10 0 100 20 10 10 0 000-20zM2 12h20M12 2a15 15 0 014 10 15 15 0 01-4 10 15 15 0 01-4-10A15 15 0 0112 2z" size={15} />
              网络隔离
            </SubHeading>
            <Hint>
              默认：无预允许域名。命令首次需要新域名时提示批准。设置 allowedDomains 可预授权域名。
            </Hint>

            <FieldRow label="允许域名">
              <TagList
                items={net.allowedDomains ?? []}
                onChange={(v) => setNetArray("allowedDomains", v)}
                placeholder="如 api.anthropic.com, *.github.com"
              />
            </FieldRow>

            <FieldRow label="拒绝域名">
              <TagList
                items={net.deniedDomains ?? []}
                onChange={(v) => setNetArray("deniedDomains", v)}
                placeholder="即使 allowedDomains 通配符允许，也会被阻止"
              />
            </FieldRow>

            <div style={{ display: "flex", gap: 16, flexWrap: "wrap" }}>
              <FieldRow label="HTTP 代理">
                <input
                  className="input"
                  type="number"
                  value={net.httpProxyPort ?? ""}
                  onChange={(e) => setNetPort("httpProxyPort", e.target.value)}
                  placeholder="端口"
                  style={{ width: 100, fontSize: F.hint, padding: "6px 10px" }}
                />
              </FieldRow>
              <FieldRow label="SOCKS 代理">
                <input
                  className="input"
                  type="number"
                  value={net.socksProxyPort ?? ""}
                  onChange={(e) => setNetPort("socksProxyPort", e.target.value)}
                  placeholder="端口"
                  style={{ width: 100, fontSize: F.hint, padding: "6px 10px" }}
                />
              </FieldRow>
            </div>
          </div>

          {/* ── Safety & Policy Toggles ── */}
          <div style={{
            padding: "14px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
            display: "flex", flexDirection: "column", gap: 10,
          }}>
            <SubHeading>
              <SvgIcon d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10zM9 12l2 2 4-4" size={15} />
              安全与策略
            </SubHeading>

            <FieldRow label="不可用时报错">
              <Toggle active={!!sb.failIfUnavailable} onChange={(v) => toggleSb("failIfUnavailable", v)} />
              <Hint>缺少依赖时阻止启动而非回退到非沙箱执行</Hint>
            </FieldRow>

            <FieldRow label="禁止逃逸">
              <Toggle active={sb.allowUnsandboxedCommands === false} onChange={(v) => sync({ allowUnsandboxedCommands: !v })} />
              <Hint>禁用 dangerouslyDisableSandbox 逃生舱，所有命令必须沙箱化</Hint>
            </FieldRow>

            <FieldRow label="锁定域名">
              <Toggle active={!!net.allowManagedDomainsOnly} onChange={(v) => sync({ network: { ...net, allowManagedDomainsOnly: v } })} />
              <Hint>仅尊重托管设置的 allowedDomains，忽略本地配置</Hint>
            </FieldRow>

            <FieldRow label="锁定读取路径">
              <Toggle active={!!sb.allowManagedReadPathsOnly} onChange={(v) => toggleSb("allowManagedReadPathsOnly", v)} />
              <Hint>仅尊重托管设置的 allowRead，忽略本地配置</Hint>
            </FieldRow>

            <FieldRow label="弱网络隔离">
              <Toggle active={!!sb.enableWeakerNetworkIsolation} onChange={(v) => toggleSb("enableWeakerNetworkIsolation", v)} />
              <Hint>MITM 代理 + 自定义 CA 场景下启用</Hint>
            </FieldRow>

            <FieldRow label="弱嵌套沙箱">
              <Toggle active={!!sb.enableWeakerNestedSandbox} onChange={(v) => toggleSb("enableWeakerNestedSandbox", v)} />
              <Hint>无特权容器内运行时启用（绑定挂载 /proc 而非新建）</Hint>
            </FieldRow>

            <FieldRow label="Unix 套接字">
              <Toggle active={!!sb.allowUnixSockets} onChange={(v) => toggleSb("allowUnixSockets", v)} />
              <Hint>允许 Unix 域套接字访问（注意：Docker socket 等可能绕过沙箱）</Hint>
            </FieldRow>
          </div>

          {/* ── Excluded Commands ── */}
          <div style={{
            padding: "14px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
            display: "flex", flexDirection: "column", gap: 10,
          }}>
            <SubHeading>
              <SvgIcon d="M18 6L6 18M6 6l12 12" size={15} />
              排除命令
            </SubHeading>
            <Hint>
              列出的命令在沙箱外运行（如 docker, gh, terraform 等与沙箱不兼容的工具）
            </Hint>
            <TagList
              items={sb.excludedCommands ?? []}
              onChange={setExcludedCommands}
              placeholder="如 docker, gh, terraform, watchman"
            />
          </div>
        </>
      )}
    </div>
  );
}

/** Sandbox with Section wrapper — for card-based layout */
function SandboxSection({
  sandboxValue,
  updateField,
  t,
}: {
  sandboxValue: Record<string, any> | undefined;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  return (
    <Section title={t("settings.sectionSandbox")} defaultOpen>
      <SandboxEditor sandboxValue={sandboxValue} updateField={updateField} />
    </Section>
  );
}

/** Sandbox without Section wrapper — for tab content pane */
function SandboxSectionInline({ sandboxValue, updateField }: {
  sandboxValue: Record<string, any> | undefined;
  updateField: (field: string, value: any) => void;
}) {
  return <SandboxEditor sandboxValue={sandboxValue} updateField={updateField} />;
}

// ─── StatusLine Section (structured editor) ────────────────────

/** A single display segment in the statusline */
interface StatusLineSegment {
  id: string;
  type: SegmentType;
  enabled: boolean;
  newline: boolean; // insert line break before this segment
  options: Record<string, any>;
}

type SegmentType =
  | "model"          // Model display name
  | "context-bar"    // Context window progress bar
  | "context-pct"    // Context window percentage
  | "git"            // Git branch + repo
  | "cost"           // API cost + duration
  | "rate-limits"    // Rate limit usage
  | "effort"         // Effort level
  | "vim"            // Vim mode
  | "separator"      // Visual separator (· or |)
  | "custom";        // Custom jq expression

interface SegmentDef {
  type: SegmentType;
  name: string;
  icon: string;
  desc: string;
  defaultOptions: Record<string, any>;
  /** Generate bash snippet for this segment */
  toBash: (opts: Record<string, any>) => string;
  /** Render preview text (static mock) */
  toPreview: (opts: Record<string, any>) => string;
  /** Editable fields for the modal */
  fields: { key: string; label: string; type: "string" | "number" | "select"; options?: string[]; placeholder?: string }[];
}

const SEGMENT_DEFS: SegmentDef[] = [
  {
    type: "model",
    name: "模型名称",
    icon: "core",
    desc: "当前模型显示名称",
    defaultOptions: { format: "short" },
    toBash: (o) => {
      const jq = o.format === "full"
        ? ".model.id // \"claude\""
        : ".model.display_name // \"Claude\"";
      return `echo -n "$(echo "$input" | jq -r '${jq}')"`;
    },
    toPreview: (o) => o.format === "full" ? "claude-sonnet-4-6" : "Opus",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["short", "full"] },
    ],
  },
  {
    type: "context-bar",
    name: "上下文进度条",
    icon: "status",
    desc: "10 字符进度条 + 百分比",
    defaultOptions: { width: 10, filled: "▓", empty: "░" },
    toBash: (o) => {
      const w = o.width || 10;
      return `__pct=$(echo "$input" | jq -r '(.context_window.used_percentage // 0) | round')
__filled=$((__pct * ${w} / 100))
__empty=$((${w} - __filled))
__bar=""
for ((i=0; i<__filled; i++)); do __bar+="${o.filled || "▓"}"; done
for ((i=0; i<__empty; i++)); do __bar+="${o.empty || "░"}"; done
echo -n "$__bar $__pct%"`;
    },
    toPreview: (o) => {
      const w = o.width || 10;
      const pct = 65;
      const filled = Math.round(pct * w / 100);
      return (o.filled || "▓").repeat(filled) + (o.empty || "░").repeat(w - filled) + ` ${pct}%`;
    },
    fields: [
      { key: "width", label: "宽度", type: "number", placeholder: "10" },
      { key: "filled", label: "填充字符", type: "string", placeholder: "▓" },
      { key: "empty", label: "空字符", type: "string", placeholder: "░" },
    ],
  },
  {
    type: "context-pct",
    name: "上下文百分比",
    icon: "status",
    desc: "仅百分比数字",
    defaultOptions: { suffix: "%" },
    toBash: () => `echo -n "$(echo "$input" | jq -r '(.context_window.used_percentage // 0) | round')%"`,
    toPreview: () => "65%",
    fields: [],
  },
  {
    type: "git",
    name: "Git 状态",
    icon: "folder",
    desc: "分支名 + 仓库名",
    defaultOptions: { showRepo: false },
    toBash: (o) => o.showRepo
      ? `echo -n "$(echo "$input" | jq -r '[.workspace.repo.owner, .workspace.repo.name] | join("/") // "detached"')"`
      : `echo -n "$(echo "$input" | jq -r '.workspace.repo.name // "detached"')"`,
    toPreview: (o) => o.showRepo ? "anthropics/claude-code" : "claude-code",
    fields: [
      { key: "showRepo", label: "显示完整路径 (owner/name)", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "cost",
    name: "成本追踪",
    icon: "bolt",
    desc: "API 成本 + 持续时间",
    defaultOptions: { showDuration: true },
    toBash: (o) => {
      const lines: string[] = [];
      lines.push('echo -n "$(echo "$input" | jq -r \'(.cost.total_cost_usd // 0) * 100 | round / 100\' | xargs printf \'\\$%.2f\')"');
      if (o.showDuration) {
        lines.push('echo -n " · $(echo "$input" | jq -r \'(.cost.total_duration_ms // 0) / 1000 | round\')s"');
      }
      return lines.join("\n");
    },
    toPreview: (o) => o.showDuration ? "$0.12 · 155s" : "$0.12",
    fields: [
      { key: "showDuration", label: "显示持续时间", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "rate-limits",
    name: "速率限制",
    icon: "permissions",
    desc: "5h / 7d 限制使用百分比",
    defaultOptions: { windows: "both" },
    toBash: (o) => {
      if (o.windows === "5h") return `echo -n "5h:$(echo "$input" | jq -r '(.rate_limits.five_hour.used_percentage // "?")')%"`;
      if (o.windows === "7d") return `echo -n "7d:$(echo "$input" | jq -r '(.rate_limits.seven_day.used_percentage // "?")')%"`;
      return `echo -n "5h:$(echo "$input" | jq -r '(.rate_limits.five_hour.used_percentage // "?")')% 7d:$(echo "$input" | jq -r '(.rate_limits.seven_day.used_percentage // "?")')%"`;
    },
    toPreview: (o) => o.windows === "5h" ? "5h:23%" : o.windows === "7d" ? "7d:41%" : "5h:23% 7d:41%",
    fields: [
      { key: "windows", label: "窗口", type: "select", options: ["both", "5h", "7d"] },
    ],
  },
  {
    type: "effort",
    name: "Effort Level",
    icon: "behavior",
    desc: "推理工作量等级",
    defaultOptions: {},
    toBash: () => `echo -n "$(echo "$input" | jq -r '.effort.level // ""')"`,
    toPreview: () => "high",
    fields: [],
  },
  {
    type: "vim",
    name: "Vim 模式",
    icon: "ui",
    desc: "当前 vim 模式",
    defaultOptions: {},
    toBash: () => `echo -n "$(echo "$input" | jq -r '.vim.mode // ""')"`,
    toPreview: () => "NORMAL",
    fields: [],
  },
  {
    type: "separator",
    name: "分隔符",
    icon: "advanced",
    desc: "视觉分隔符",
    defaultOptions: { char: " · " },
    toBash: (o) => `echo -n "${o.char || " · "}"`,
    toPreview: (o) => o.char || " · ",
    fields: [
      { key: "char", label: "分隔符字符", type: "string", placeholder: " · " },
    ],
  },
  {
    type: "custom",
    name: "自定义",
    icon: "bolt",
    desc: "自定义 jq 表达式",
    defaultOptions: { expr: ".model.display_name" },
    toBash: (o) => `echo -n "$(echo "$input" | jq -r '${o.expr || ".model.display_name"}')"`,
    toPreview: (o) => `<${o.expr || ".model.display_name"}>`,
    fields: [
      { key: "expr", label: "jq 表达式", type: "string", placeholder: ".model.display_name" },
    ],
  },
];

const SEGMENT_DEF_MAP = new Map(SEGMENT_DEFS.map(d => [d.type, d]));

const DEFAULT_SEGMENTS: StatusLineSegment[] = [
  { id: "s1", type: "model", enabled: true, newline: false, options: {} },
  { id: "s2", type: "separator", enabled: true, newline: false, options: {} },
  { id: "s3", type: "context-bar", enabled: true, newline: false, options: {} },
];

const SUBAGENT_TEMPLATES = [
  { id: "default", name: "默认", generate: () => `#!/usr/bin/env bash\ninput=$(cat)\necho "$input" | jq -r '.tasks[]? | "\\(.name) · \\(.status) · \\(.tokenCount)t" // empty' | head -1` },
  { id: "compact", name: "紧凑", generate: () => `#!/usr/bin/env bash\ninput=$(cat)\necho "$input" | jq -r '.tasks[]? | .name' | head -1` },
  { id: "detailed", name: "详细", generate: () => `#!/usr/bin/env bash\ninput=$(cat)\necho "$input" | jq -r '.tasks[]? | "\\(.name) \\(.description // "") \\(.tokenCount)t"' | head -1` },
];

// ── Available data fields reference ──

const STATUSLINE_DATA_FIELDS = [
  { group: "模型", fields: [
    { key: "model.id", desc: "模型标识符" },
    { key: "model.display_name", desc: "模型显示名称" },
  ]},
  { group: "工作区", fields: [
    { key: "workspace.current_dir", desc: "当前工作目录" },
    { key: "workspace.project_dir", desc: "项目启动目录" },
    { key: "workspace.repo.owner/name", desc: "Git 仓库标识" },
  ]},
  { group: "成本", fields: [
    { key: "cost.total_cost_usd", desc: "累计预估成本 ($)" },
    { key: "cost.total_duration_ms", desc: "总持续时间 (ms)" },
    { key: "cost.total_api_duration_ms", desc: "API 等待时间 (ms)" },
  ]},
  { group: "上下文窗口", fields: [
    { key: "context_window.used_percentage", desc: "已使用百分比" },
    { key: "context_window.context_window_size", desc: "最大窗口大小" },
  ]},
  { group: "速率限制", fields: [
    { key: "rate_limits.five_hour.used_percentage", desc: "5小时窗口使用 %" },
    { key: "rate_limits.seven_day.used_percentage", desc: "7天窗口使用 %" },
  ]},
  { group: "其他", fields: [
    { key: "effort.level", desc: "推理工作量" },
    { key: "vim.mode", desc: "Vim 模式" },
    { key: "session_id", desc: "会话 ID" },
    { key: "version", desc: "Claude Code 版本" },
  ]},
];

// ── Script generation from segments ──

function generateStatusLineScript(segments: StatusLineSegment[]): string {
  const active = segments.filter(s => s.enabled);
  if (active.length === 0) return "#!/usr/bin/env bash\necho ''\n";

  const lines: string[] = ["#!/usr/bin/env bash", "# Generated by aidog — do not edit manually", "input=$(cat)", ""];

  // Collect segments into lines (split by newline=true)
  const outputLines: string[][] = [];
  let currentLine: string[] = [];

  for (const seg of active) {
    if (seg.newline && currentLine.length > 0) {
      outputLines.push(currentLine);
      currentLine = [];
    }
    const def = SEGMENT_DEF_MAP.get(seg.type);
    if (!def) continue;
    const opts = { ...def.defaultOptions, ...seg.options };
    currentLine.push(def.toBash(opts));
  }
  if (currentLine.length > 0) {
    outputLines.push(currentLine);
  }

  // Generate script: each output line = array of command outputs joined
  for (let i = 0; i < outputLines.length; i++) {
    const parts = outputLines[i];
    lines.push(`__line${i}=()`);
    for (const part of parts) {
      lines.push(`__line${i}+=($(${part}))`);
    }
    lines.push(`echo "${"${__line" + i + "[*]}"}"`);
    if (i < outputLines.length - 1) lines.push("");
  }

  return lines.join("\n");
}

function generatePreview(segments: StatusLineSegment[]): string {
  const active = segments.filter(s => s.enabled);
  let result = "";
  for (const seg of active) {
    if (seg.newline) result += "\n";
    const def = SEGMENT_DEF_MAP.get(seg.type);
    if (!def) continue;
    const opts = { ...def.defaultOptions, ...seg.options };
    result += def.toPreview(opts);
  }
  return result || "(空)";
}

// ── Segment Edit Modal ──

function SegmentEditModal({
  segment,
  onSave,
  onClose,
}: {
  segment: StatusLineSegment;
  onSave: (patch: Partial<StatusLineSegment>) => void;
  onClose: () => void;
}) {
  const def = SEGMENT_DEF_MAP.get(segment.type);
  if (!def) return null;
  const [opts, setOpts] = useState({ ...def.defaultOptions, ...segment.options });
  const [newline, setNewline] = useState(segment.newline);

  return (
    <div style={{
      position: "fixed", inset: 0, zIndex: 1000,
      display: "flex", alignItems: "center", justifyContent: "center",
      background: "rgba(0,0,0,0.5)", animation: "fadeIn 150ms ease both",
    }} onClick={onClose}>
      <div className="glass-elevated"
        style={{
          width: 420, maxHeight: "80vh", overflow: "auto",
          padding: 24, borderRadius: "var(--radius-lg)",
          animation: "fadeIn 200ms ease both",
        }}
        onClick={(e) => e.stopPropagation()}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
          <div style={{ fontSize: F.title, fontWeight: 600, color: "var(--text-primary)" }}>
            {def.name}
          </div>
          <button type="button" className="btn btn-ghost btn-icon"
            style={{ width: 28, height: 28, fontSize: F.body }}
            onClick={onClose}>×</button>
        </div>
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 16 }}>{def.desc}</div>

        {/* Newline toggle */}
        <label style={{
          display: "flex", alignItems: "center", gap: 8, marginBottom: 16,
          padding: "8px 12px", background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
          fontSize: F.body, color: "var(--text-primary)", cursor: "pointer",
        }}>
          <Toggle active={newline} onChange={setNewline} />
          换行显示（在此段前插入新行）
        </label>

        {/* Type-specific fields */}
        {def.fields.length > 0 && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12, marginBottom: 16 }}>
            {def.fields.map(f => (
              <div key={f.key} style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <label style={{ fontSize: F.body, color: "var(--text-secondary)", minWidth: 100, flexShrink: 0 }}>
                  {f.label}
                </label>
                {f.type === "select" ? (
                  <select className="input" style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
                    value={String(opts[f.key] ?? f.options?.[0] ?? "")}
                    onChange={(e) => setOpts({ ...opts, [f.key]: e.target.value })}>
                    {f.options?.map(o => <option key={o} value={o}>{o}</option>)}
                  </select>
                ) : f.type === "number" ? (
                  <input className="input" type="number" style={{ fontSize: F.body, padding: S.inputPad, flex: 1, width: 80 }}
                    value={opts[f.key] ?? ""} placeholder={f.placeholder}
                    onChange={(e) => setOpts({ ...opts, [f.key]: Number(e.target.value) })} />
                ) : (
                  <input className="input" style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
                    value={String(opts[f.key] ?? "")} placeholder={f.placeholder}
                    onChange={(e) => setOpts({ ...opts, [f.key]: e.target.value })} />
                )}
              </div>
            ))}
          </div>
        )}

        {/* Preview */}
        <div style={{ marginBottom: 16 }}>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 4 }}>预览</div>
          <div style={{
            padding: "8px 14px", background: "var(--bg-surface)",
            borderRadius: "var(--radius-sm)", fontSize: F.body,
            fontFamily: '"SF Mono", "Fira Code", monospace',
            color: "var(--text-primary)",
          }}>
            {def.toPreview(opts)}
          </div>
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="btn btn-ghost" style={{ fontSize: F.body, padding: S.btnPad }} onClick={onClose}>
            取消
          </button>
          <button className="btn btn-primary" style={{ fontSize: F.body, padding: S.btnPad }}
            onClick={() => { onSave({ options: opts, newline }); onClose(); }}>
            保存
          </button>
        </div>
      </div>
    </div>
  );
}

// ── StatusLine Panel Component ──

function StatusLinePanel({
  config,
  updateField,
  scriptType,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
  scriptType: "statusline" | "subagent";
}) {
  const isMain = scriptType === "statusline";
  const aidogKey = isMain ? "_aidog_statusline" : "_aidog_subagent_statusline";
  const fieldName = isMain ? "statusLine" : "subagentStatusLine";

  const stored = (config[aidogKey] ?? {}) as Record<string, any>;
  const enabled = !!stored.enabled;
  const padding = stored.padding ?? 2;
  const hideVimModeIndicator = !!stored.hideVimModeIndicator;

  // Segments for main statusline, template ID for subagent
  const segments: StatusLineSegment[] = isMain
    ? (stored.segments ?? DEFAULT_SEGMENTS.map(s => ({ ...s })))
    : [];
  const subagentTemplateId = !isMain ? (stored.template ?? "default") : "";

  const [showScript, setShowScript] = useState(false);
  const [saving, setSaving] = useState(false);
  const [editSeg, setEditSeg] = useState<StatusLineSegment | null>(null);
  const [dragIdx, setDragIdx] = useState<number | null>(null);
  const [showAddMenu, setShowAddMenu] = useState(false);

  const setStored = (patch: Record<string, any>) => {
    updateField(aidogKey, { ...stored, ...patch });
  };

  const handleToggle = (val: boolean) => {
    if (!val) {
      updateField(fieldName, undefined);
      setStored({ enabled: false });
    } else {
      setStored({ enabled: true });
    }
  };

  const updateSegments = (next: StatusLineSegment[]) => setStored({ segments: next });

  const moveSegment = (from: number, to: number) => {
    const next = [...segments];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    updateSegments(next);
  };

  // Generate script
  const scriptPreview = isMain
    ? generateStatusLineScript(segments)
    : SUBAGENT_TEMPLATES.find(tp => tp.id === subagentTemplateId)?.generate() ?? SUBAGENT_TEMPLATES[0].generate();

  const previewText = isMain ? generatePreview(segments) : "<subagent statusline>";

  const handleSave = async () => {
    setSaving(true);
    try {
      const path = await statuslineApi.generate(scriptType, scriptPreview);
      const value: Record<string, any> = { type: "command", command: path };
      if (isMain && padding > 0) value.padding = padding;
      if (isMain && hideVimModeIndicator) value.hideVimModeIndicator = true;
      updateField(fieldName, value);
    } catch (e: any) {
      console.error("generate_statusline_script:", e);
    }
    setSaving(false);
  };

  const addSegment = (type: SegmentType) => {
    const def = SEGMENT_DEF_MAP.get(type);
    if (!def) return;
    const newSeg: StatusLineSegment = {
      id: `s${Date.now()}`,
      type,
      enabled: true,
      newline: false,
      options: { ...def.defaultOptions },
    };
    updateSegments([...segments, newSeg]);
    setShowAddMenu(false);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {/* Enable toggle */}
      <div style={{
        display: "flex", alignItems: "center", gap: 12,
        padding: "12px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
      }}>
        <Toggle active={enabled} onChange={handleToggle} />
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-primary)" }}>
            {isMain ? "使用内置状态栏" : "使用内置子代理状态栏"}
          </div>
          <Hint>{isMain
            ? "开启后 aidog 生成脚本到 ~/.aidog/aidog-statusline.sh"
            : "开启后 aidog 生成脚本到 ~/.aidog/aidog-subagent-statusline.sh"}</Hint>
        </div>
        {enabled && (
          <span style={{
            fontSize: F.small, fontWeight: 600, color: "#34c759",
            padding: "2px 8px", background: "rgba(52,199,89,0.12)", borderRadius: "var(--radius-sm)",
          }}>● 已启用</span>
        )}
      </div>

      {enabled && (
        <>
          {/* Preview */}
          <div style={{
            padding: "12px 16px", background: "var(--bg-surface)", borderRadius: "var(--radius-md)",
            border: "1px solid var(--border)",
          }}>
            <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 6 }}>预览</div>
            <div style={{
              fontFamily: '"SF Mono", "Fira Code", monospace', fontSize: F.body,
              color: "var(--text-primary)", whiteSpace: "pre-wrap", lineHeight: 1.6,
            }}>
              {previewText}
            </div>
          </div>

          {isMain ? (
            /* ── Main: Drag-sortable segment list ── */
            <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              {segments.map((seg, idx) => {
                const def = SEGMENT_DEF_MAP.get(seg.type);
                if (!def) return null;
                return (
                  <div key={seg.id}
                    draggable
                    onDragStart={() => setDragIdx(idx)}
                    onDragOver={(e) => { e.preventDefault(); }}
                    onDrop={() => { if (dragIdx !== null && dragIdx !== idx) moveSegment(dragIdx, idx); setDragIdx(null); }}
                    onDragEnd={() => setDragIdx(null)}
                    style={{
                      display: "flex", alignItems: "center", gap: 8,
                      padding: "8px 12px", background: dragIdx === idx ? "var(--accent-subtle, rgba(0,122,255,0.1))" : "var(--bg-glass)",
                      borderRadius: "var(--radius-sm)", cursor: "grab",
                      opacity: seg.enabled ? 1 : 0.4,
                      border: "1px solid var(--border)",
                      transition: "background 150ms, opacity 150ms",
                    }}>
                    {/* Drag handle */}
                    <span style={{ color: "var(--text-tertiary)", fontSize: F.hint, cursor: "grab", userSelect: "none" }}>☰</span>
                    {/* Toggle */}
                    <Toggle active={seg.enabled} onChange={(v) => {
                      const next = [...segments];
                      next[idx] = { ...next[idx], enabled: v };
                      updateSegments(next);
                    }} />
                    {/* Icon + name */}
                    <SectionIcon name={def.icon} size={14} />
                    <span style={{ fontSize: F.body, fontWeight: 500, color: "var(--text-primary)", flexShrink: 0 }}>
                      {def.name}
                    </span>
                    {/* Inline preview */}
                    <span style={{
                      flex: 1, fontSize: F.hint, color: "var(--text-tertiary)",
                      fontFamily: '"SF Mono", "Fira Code", monospace',
                      overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                    }}>
                      {def.toPreview({ ...def.defaultOptions, ...seg.options })}
                    </span>
                    {/* Edit button */}
                    <button type="button" className="btn btn-ghost"
                      style={{ fontSize: F.hint, padding: "2px 8px", color: "var(--accent)" }}
                      onClick={() => setEditSeg({ ...seg })}>
                      ✎
                    </button>
                    {/* Delete */}
                    <button type="button" className="btn btn-ghost btn-icon"
                      style={{ width: 24, height: 24, minWidth: 24, fontSize: F.hint, color: "var(--text-tertiary)" }}
                      onClick={() => updateSegments(segments.filter((_, i) => i !== idx))}>
                      ×
                    </button>
                  </div>
                );
              })}

              {/* Add segment */}
              <div style={{ position: "relative", display: "flex", justifyContent: "flex-end", marginTop: 4 }}>
                <button className="btn btn-ghost" style={{ fontSize: F.body, padding: "6px 14px" }}
                  onClick={() => setShowAddMenu(!showAddMenu)}>
                  + 添加行
                </button>
                {showAddMenu && (
                  <div style={{
                    position: "absolute", bottom: "100%", right: 0, zIndex: 100,
                    background: "var(--bg-surface)", border: "1px solid var(--border)",
                    borderRadius: "var(--radius-md)", padding: 4,
                    maxHeight: 320, overflow: "auto", minWidth: 240,
                    boxShadow: "0 8px 32px rgba(0,0,0,0.2)",
                  }}>
                    {SEGMENT_DEFS.filter(d => d.type !== "separator").map(d => (
                      <button key={d.type} type="button" style={{
                        display: "block", width: "100%", textAlign: "left",
                        padding: "6px 12px", fontSize: F.body,
                        background: "transparent", border: "none", borderRadius: "var(--radius-sm)",
                        cursor: "pointer", color: "var(--text-primary)",
                      }}
                        onMouseEnter={(e) => { e.currentTarget.style.background = "var(--bg-glass)"; }}
                        onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
                        onClick={() => addSegment(d.type)}>
                        <span style={{ fontWeight: 500 }}>{d.name}</span>
                        <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginLeft: 8 }}>{d.desc}</span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          ) : (
            /* ── Subagent: Template selector ── */
            <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
              {SUBAGENT_TEMPLATES.map(tp => {
                const active = tp.id === subagentTemplateId;
                return (
                  <button key={tp.id} type="button"
                    style={{
                      padding: "6px 14px", fontSize: F.body, fontWeight: active ? 600 : 400,
                      color: active ? "var(--accent)" : "var(--text-secondary)",
                      background: active ? "var(--accent-subtle, rgba(0,122,255,0.1))" : "transparent",
                      border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
                      borderRadius: "var(--radius-sm)", cursor: "pointer",
                    }}
                    onClick={() => setStored({ template: tp.id })}>
                    {tp.name}
                  </button>
                );
              })}
            </div>
          )}

          {/* Options row */}
          {isMain && (
            <div style={{
              padding: "10px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
              display: "flex", gap: 20, flexWrap: "wrap", alignItems: "center",
            }}>
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <label style={{ fontSize: F.body, color: "var(--text-secondary)" }}>水平间距</label>
                <input className="input" type="number" min={0} max={20}
                  style={{ width: 60, fontSize: F.body, padding: S.inputPad }}
                  value={padding}
                  onChange={(e) => setStored({ padding: Math.max(0, Number(e.target.value)) })} />
              </div>
              <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: F.body, color: "var(--text-secondary)", cursor: "pointer" }}>
                <Toggle active={hideVimModeIndicator} onChange={(v) => setStored({ hideVimModeIndicator: v })} />
                隐藏 Vim 模式指示器
              </label>
            </div>
          )}

          {/* Script preview (collapsible) */}
          <div style={{
            padding: "10px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
          }}>
            <button type="button" className="btn btn-ghost"
              style={{ fontSize: F.body, padding: "4px 8px", display: "flex", alignItems: "center", gap: 4, width: "100%", justifyContent: "space-between" }}
              onClick={() => setShowScript(!showScript)}>
              <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                <span style={{ transform: showScript ? "rotate(90deg)" : "rotate(0deg)", transition: "transform 150ms", display: "inline-block" }}>▶</span>
                脚本预览
              </span>
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)", fontFamily: '"SF Mono", "Fira Code", monospace' }}>
                ~/.aidog/aidog-{scriptType === "subagent" ? "subagent-" : ""}statusline.sh
              </span>
            </button>
            {showScript && (
              <pre style={{
                fontFamily: '"SF Mono", "Fira Code", monospace',
                fontSize: F.hint, lineHeight: 1.6,
                background: "var(--bg-surface)", borderRadius: "var(--radius-sm)",
                padding: 12, overflow: "auto", whiteSpace: "pre",
                color: "var(--text-primary)", margin: 0, marginTop: 8,
              }}>
                {scriptPreview}
              </pre>
            )}
          </div>

          {/* Apply button */}
          <div style={{ display: "flex", justifyContent: "flex-end" }}>
            <button className="btn btn-primary" style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={handleSave} disabled={saving}>
              {saving ? "生成中…" : "应用并生成脚本"}
            </button>
          </div>
        </>
      )}

      {/* Edit modal */}
      {editSeg && (
        <SegmentEditModal
          segment={editSeg}
          onClose={() => setEditSeg(null)}
          onSave={(patch) => {
            const idx = segments.findIndex(s => s.id === editSeg.id);
            if (idx >= 0) {
              const next = [...segments];
              next[idx] = { ...next[idx], ...patch };
              updateSegments(next);
            }
            setEditSeg(null);
          }}
        />
      )}
    </div>
  );
}

/** Combined section for status tab */
function StatusLineSection({
  config,
  updateField,
  t,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const [showDataRef, setShowDataRef] = useState(false);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {/* StatusLine */}
      <div style={{
        padding: 16, border: "1px solid var(--border)", borderRadius: "var(--radius-md)",
        display: "flex", flexDirection: "column", gap: 4,
      }}>
        <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-primary)", marginBottom: 8, display: "flex", alignItems: "center", gap: 6 }}>
          <SectionIcon name="status" size={15} />
          StatusLine
        </div>
        <StatusLinePanel config={config} updateField={updateField} scriptType="statusline" />
      </div>

      {/* SubagentStatusLine */}
      <div style={{
        padding: 16, border: "1px solid var(--border)", borderRadius: "var(--radius-md)",
        display: "flex", flexDirection: "column", gap: 4,
      }}>
        <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-primary)", marginBottom: 8, display: "flex", alignItems: "center", gap: 6 }}>
          <SectionIcon name="team" size={15} />
          SubagentStatusLine
        </div>
        <StatusLinePanel config={config} updateField={updateField} scriptType="subagent" />
      </div>

      {/* FileSuggestion (keep existing behavior) */}
      {(() => {
        const field: SettingField = {
          key: "fileSuggestion",
          label: "File Suggestion",
          type: "string",
          description: "自定义文件建议脚本路径",
          pathType: "file",
        };
        return (
          <FieldRenderer
            field={field}
            value={config.fileSuggestion}
            onChange={(v) => updateField("fileSuggestion", v)}
            t={t}
          />
        );
      })()}

      {/* Data reference panel */}
      <div style={{
        padding: 16, border: "1px solid var(--border)", borderRadius: "var(--radius-md)",
      }}>
        <button type="button" className="btn btn-ghost"
          style={{ fontSize: F.body, padding: "4px 8px", display: "flex", alignItems: "center", gap: 4, width: "100%", justifyContent: "flex-start" }}
          onClick={() => setShowDataRef(!showDataRef)}>
          <span style={{ transform: showDataRef ? "rotate(90deg)" : "rotate(0deg)", transition: "transform 150ms", display: "inline-block" }}>▶</span>
          可用数据字段参考
        </button>
        {showDataRef && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 12 }}>
            <Hint>Claude Code 通过 stdin 注入以下 JSON 字段，可在脚本中用 jq 提取</Hint>
            {STATUSLINE_DATA_FIELDS.map(group => (
              <div key={group.group}>
                <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 6 }}>
                  {group.group}
                </div>
                <table style={{ width: "100%", borderCollapse: "collapse" }}>
                  {group.fields.map(f => (
                    <tr key={f.key} style={{ borderBottom: "1px solid var(--border)" }}>
                      <td style={{
                        padding: "4px 12px 4px 0", fontSize: F.hint,
                        fontFamily: '"SF Mono", "Fira Code", monospace',
                        color: "var(--accent)", whiteSpace: "nowrap",
                      }}>
                        {f.key}
                      </td>
                      <td style={{ padding: "4px 0", fontSize: F.hint, color: "var(--text-tertiary)" }}>
                        {f.desc}
                      </td>
                    </tr>
                  ))}
                </table>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ── Import Diff Modal ──

function ImportDiffModal({
  diff,
  onApply,
  onClose,
}: {
  diff: { key: string; current: any; incoming: any; selected: boolean }[];
  onApply: (selectedKeys: Set<string>) => void;
  onClose: () => void;
}) {
  const [selected, setSelected] = useState<Set<string>>(() => {
    const s = new Set<string>();
    diff.forEach(d => { if (d.selected) s.add(d.key); });
    return s;
  });

  const toggle = (key: string) => {
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key); else next.add(key);
      return next;
    });
  };

  const toggleAll = () => {
    if (selected.size === diff.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(diff.map(d => d.key)));
    }
  };

  const formatValue = (v: any): string => {
    if (v === undefined) return "(无)";
    if (typeof v === "object") return JSON.stringify(v, null, 2);
    return String(v);
  };

  const getChangeType = (d: { current: any; incoming: any }) => {
    if (d.current === undefined) return "added";
    if (d.incoming === undefined) return "removed";
    return "changed";
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
            从 Claude Code 导入配置
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <button className="btn btn-ghost" style={{ fontSize: F.hint, padding: "4px 10px" }}
              onClick={toggleAll}>
              {selected.size === diff.length ? "取消全选" : "全选"}
            </button>
            <button type="button" className="btn btn-ghost btn-icon"
              style={{ width: 28, height: 28, fontSize: F.body }}
              onClick={onClose}>×</button>
          </div>
        </div>

        {/* Diff list */}
        <div style={{ flex: 1, overflowY: "auto", padding: "8px 0" }}>
          {diff.map(d => {
            const changeType = getChangeType(d);
            const isSelected = selected.has(d.key);
            const bgColor = changeType === "added" ? "rgba(52,199,89,0.06)"
              : changeType === "removed" ? "rgba(255,69,58,0.06)"
              : "var(--bg-glass)";
            const labelColor = changeType === "added" ? "#34c759"
              : changeType === "removed" ? "#ff453a"
              : "var(--accent)";
            const label = changeType === "added" ? "新增" : changeType === "removed" ? "删除" : "变更";

            return (
              <div key={d.key} style={{
                margin: "4px 12px", padding: "10px 14px",
                background: isSelected ? bgColor : "var(--bg-surface)",
                border: `1px solid ${isSelected ? "var(--border)" : "transparent"}`,
                borderRadius: "var(--radius-sm)",
                opacity: isSelected ? 1 : 0.5,
                transition: "all 150ms",
              }}>
                {/* Key header */}
                <div style={{
                  display: "flex", alignItems: "center", gap: 8,
                  cursor: "pointer",
                }} onClick={() => toggle(d.key)}>
                  <Toggle active={isSelected} onChange={() => toggle(d.key)} />
                  <span style={{
                    fontSize: F.body, fontWeight: 600, color: "var(--text-primary)",
                    fontFamily: '"SF Mono", "Fira Code", monospace',
                  }}>
                    {d.key}
                  </span>
                  <span style={{
                    fontSize: F.hint, fontWeight: 600, color: labelColor,
                    padding: "1px 6px", background: `${labelColor}18`, borderRadius: "var(--radius-sm)",
                  }}>
                    {label}
                  </span>
                </div>

                {/* Values diff */}
                {isSelected && (
                  <div style={{
                    display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8,
                    marginTop: 8, marginLeft: 36,
                  }}>
                    <div>
                      <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 2 }}>当前</div>
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
                      <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 2 }}>导入</div>
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
          })}
        </div>

        {/* Footer */}
        <div style={{
          padding: "12px 20px", borderTop: "1px solid var(--border)",
          display: "flex", justifyContent: "space-between", alignItems: "center",
        }}>
          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>
            已选 {selected.size}/{diff.length} 项
          </span>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn btn-ghost" style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={onClose}>取消</button>
            <button className="btn btn-primary" style={{ fontSize: F.body, padding: S.btnPad }}
              disabled={selected.size === 0}
              onClick={() => onApply(selected)}>
              导入选中 ({selected.size})
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ─── Plugins Section (structured editor) ─────────────────────

const MARKETPLACE_SOURCE_TYPES = ["github", "git", "url", "npm", "file", "directory", "settings", "hostPattern", "pathPattern"] as const;
type SourceType = typeof MARKETPLACE_SOURCE_TYPES[number];

const SOURCE_TYPE_LABELS: Record<SourceType, string> = {
  github: "GitHub",
  git: "Git URL",
  url: "URL (marketplace.json)",
  npm: "NPM Package",
  file: "File (marketplace.json)",
  directory: "Directory",
  settings: "Inline Settings",
  hostPattern: "Host Pattern (regex)",
  pathPattern: "Path Pattern (regex)",
};

/** Type-specific field definitions */
const SOURCE_FIELDS: Record<SourceType, { key: string; label: string; placeholder: string; required?: boolean }[]> = {
  github: [
    { key: "repo", label: "Repository", placeholder: "owner/repo", required: true },
    { key: "ref", label: "Ref (branch/tag/sha)", placeholder: "main" },
    { key: "path", label: "Subdirectory", placeholder: "marketplace" },
  ],
  git: [
    { key: "url", label: "Git URL", placeholder: "https://git.example.com/plugins.git", required: true },
    { key: "ref", label: "Ref (branch/tag/sha)", placeholder: "main" },
    { key: "path", label: "Subdirectory", placeholder: "marketplace" },
  ],
  url: [
    { key: "url", label: "Marketplace JSON URL", placeholder: "https://plugins.example.com/marketplace.json", required: true },
  ],
  npm: [
    { key: "package", label: "NPM Package", placeholder: "@acme-corp/claude-plugins", required: true },
  ],
  file: [
    { key: "path", label: "File Path", placeholder: "/usr/local/share/claude/marketplace.json", required: true },
  ],
  directory: [
    { key: "path", label: "Directory Path", placeholder: "/usr/local/share/claude/plugins", required: true },
  ],
  settings: [
    { key: "name", label: "Marketplace Name", placeholder: "team-tools", required: true },
  ],
  hostPattern: [
    { key: "hostPattern", label: "Host Pattern (regex)", placeholder: "^github\\.example\\.com$", required: true },
  ],
  pathPattern: [
    { key: "pathPattern", label: "Path Pattern (regex)", placeholder: "^/opt/approved/", required: true },
  ],
};

/** Source config for a single marketplace entry */
function MarketplaceSourceEditor({
  source,
  onChange,
  compact = false,
}: {
  source: Record<string, any>;
  onChange: (s: Record<string, any>) => void;
  compact?: boolean;
}) {
  const srcType = (source.source ?? "github") as SourceType;
  const fields = SOURCE_FIELDS[srcType] ?? [];
  const setField = (key: string, val: string | boolean) => {
    onChange({ ...source, [key]: val || undefined });
  };
  const fs = compact ? F.hint : F.body;
  const pad = compact ? "4px 8px" : "6px 10px";

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6, paddingLeft: 8, borderLeft: "2px solid var(--border)" }}>
      {/* Source type selector */}
      <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: compact ? 50 : 80, flexShrink: 0, whiteSpace: "nowrap" }}>Type</span>
        <select className="input" style={{ fontSize: fs, padding: pad, flex: 1 }}
          value={srcType}
          onChange={(e) => {
            const newType = e.target.value as SourceType;
            // Keep only source type, clear type-specific fields
            onChange({ source: newType });
          }}>
          {MARKETPLACE_SOURCE_TYPES.map((t) => (
            <option key={t} value={t}>{SOURCE_TYPE_LABELS[t]}</option>
          ))}
        </select>
      </div>

      {/* Type-specific fields */}
      {fields.map((f) => (
        <div key={f.key} style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: compact ? 50 : 80, flexShrink: 0, whiteSpace: "nowrap" }}>
            {f.label}{f.required && " *"}
          </span>
          <input className="input" style={{ fontSize: fs, padding: pad, flex: 1 }}
            placeholder={f.placeholder} value={source[f.key] ?? ""}
            onChange={(e) => setField(f.key, e.target.value)} />
        </div>
      ))}

      {/* skipLfs for github/git */}
      {(srcType === "github" || srcType === "git") && (
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: compact ? 50 : 80, flexShrink: 0, whiteSpace: "nowrap" }}>skipLfs</span>
          <Toggle active={!!source.skipLfs} onChange={(v) => setField("skipLfs", v)} />
          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>跳过 LFS 下载</span>
        </div>
      )}

      {/* URL headers for url type */}
      {srcType === "url" && (
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: 80, flexShrink: 0, whiteSpace: "nowrap" }}>Headers</span>
          <input className="input" style={{ fontSize: fs, padding: pad, flex: 1 }}
            placeholder='{"Authorization": "Bearer ${TOKEN}"}'
            value={source.headers ? JSON.stringify(source.headers) : ""}
            onChange={(e) => {
              try { onChange({ ...source, headers: JSON.parse(e.target.value) }); }
              catch { /* invalid JSON, keep as-is */ }
            }} />
        </div>
      )}

      {/* settings: inline plugins list */}
      {srcType === "settings" && (
        <>
          {(source.plugins as Array<Record<string, any>> | undefined)?.map((plug, pi) => (
            <div key={pi} style={{ display: "flex", gap: 4, alignItems: "flex-start", paddingLeft: 8, paddingTop: 4 }}>
              <input className="input" style={{ fontSize: F.hint, padding: "4px 8px", width: 100, flexShrink: 0 }}
                placeholder="plugin-name" value={plug.name ?? ""}
                onChange={(e) => {
                  const plugs = [...(source.plugins ?? [])];
                  plugs[pi] = { ...plug, name: e.target.value };
                  onChange({ ...source, plugins: plugs });
                }} />
              <div style={{ flex: 1 }}>
                <MarketplaceSourceEditor
                  source={plug.source ?? { source: "github" }}
                  onChange={(s) => {
                    const plugs = [...(source.plugins ?? [])];
                    plugs[pi] = { ...plug, source: s };
                    onChange({ ...source, plugins: plugs });
                  }}
                  compact
                />
              </div>
              <button type="button" onClick={() => {
                const plugs = (source.plugins ?? []).filter((_: any, j: number) => j !== pi);
                onChange({ ...source, plugins: plugs.length > 0 ? plugs : undefined });
              }} style={{
                background: "none", border: "none", cursor: "pointer",
                color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1, flexShrink: 0,
              }}>✕</button>
            </div>
          ))}
          <button type="button" className="btn btn-ghost" style={{ fontSize: F.small, padding: "4px 10px", alignSelf: "flex-start", marginLeft: 8 }}
            onClick={() => {
              const plugs = [...(source.plugins ?? []), { name: "", source: { source: "github" } }];
              onChange({ ...source, plugins: plugs });
            }}>+ Plugin</button>
        </>
      )}

      {/* autoUpdate toggle */}
      <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
        <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: compact ? 50 : 80, flexShrink: 0, whiteSpace: "nowrap" }}>auto</span>
        <Toggle active={!!source.autoUpdate} onChange={(v) => setField("autoUpdate", v)} />
        <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>启动时自动刷新</span>
      </div>
    </div>
  );
}

/** Main plugins structured editor */
function PluginsEditor({
  config,
  updateField,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
}) {
  const enabledPlugins = (config.enabledPlugins ?? {}) as Record<string, boolean>;
  const extraMarketplaces = (config.extraKnownMarketplaces ?? {}) as Record<string, any>;

  // ── Enabled Plugins ──
  const [newPluginKey, setNewPluginKey] = useState("");
  const pluginEntries = Object.entries(enabledPlugins);

  const setPluginEnabled = (key: string, val: boolean) => {
    const next = { ...enabledPlugins, [key]: val };
    updateField("enabledPlugins", next);
  };
  const addPlugin = () => {
    const k = newPluginKey.trim();
    if (!k) return;
    setPluginEnabled(k, true);
    setNewPluginKey("");
  };
  const removePlugin = (key: string) => {
    const next = { ...enabledPlugins };
    delete next[key];
    updateField("enabledPlugins", Object.keys(next).length > 0 ? next : undefined);
  };

  // ── Extra Marketplaces ──
  const [newMktName, setNewMktName] = useState("");
  const mktEntries = Object.entries(extraMarketplaces);

  const addMarketplace = () => {
    const name = newMktName.trim();
    if (!name) return;
    const next = { ...extraMarketplaces, [name]: { source: { source: "github" } } };
    updateField("extraKnownMarketplaces", next);
    setNewMktName("");
  };
  const updateMarketplace = (name: string, val: any) => {
    const next = { ...extraMarketplaces, [name]: val };
    updateField("extraKnownMarketplaces", next);
  };
  const removeMarketplace = (name: string) => {
    const next = { ...extraMarketplaces };
    delete next[name];
    updateField("extraKnownMarketplaces", Object.keys(next).length > 0 ? next : undefined);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.sectionGap }}>
      {/* ── Enabled Plugins ── */}
      <div>
        <SubHeading>
          <SvgIcon d={ICON_PATHS.plugins} size={14} style={{ opacity: 0.6 }} />
          Enabled Plugins
        </SubHeading>
        <Hint>格式: plugin-name@marketplace → 启用/禁用</Hint>
        <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 8 }}>
          {pluginEntries.map(([key, val]) => (
            <div key={key} style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <code style={{
                flex: 1, fontSize: F.hint, padding: "6px 10px",
                background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
                color: "var(--text-primary)", fontFamily: "monospace",
                overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
              }}>
                {key}
              </code>
              <Toggle active={val} onChange={(v) => setPluginEnabled(key, v)} />
              <button type="button" onClick={() => removePlugin(key)} style={{
                background: "none", border: "none", cursor: "pointer",
                color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1,
              }}>✕</button>
            </div>
          ))}
          <div style={{ display: "flex", gap: 6, alignItems: "center", marginTop: 2 }}>
            <input
              className="input"
              style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
              placeholder="plugin-name@marketplace"
              value={newPluginKey}
              onChange={(e) => setNewPluginKey(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addPlugin()}
            />
            <button type="button" className="btn btn-ghost" style={{ fontSize: F.small, padding: "4px 12px" }}
              onClick={addPlugin}>+</button>
          </div>
        </div>
      </div>

      {/* ── Extra Marketplaces ── */}
      <div>
        <SubHeading>
          <SvgIcon d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2Z" size={14} style={{ opacity: 0.6 }} />
          Extra Marketplaces
        </SubHeading>
        <Hint>命名市场源定义（github / git / directory / settings）</Hint>
        <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 8 }}>
          {mktEntries.map(([name, mktConfig]) => (
            <div key={name} style={{
              padding: "10px 12px", background: "var(--bg-glass)",
              borderRadius: "var(--radius-md)", display: "flex", flexDirection: "column", gap: 6,
            }}>
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <span style={{
                  fontSize: F.body, fontWeight: 600, color: "var(--accent)",
                  fontFamily: "monospace",
                }}>{name}</span>
                <div style={{ flex: 1 }} />
                <button type="button" onClick={() => removeMarketplace(name)} style={{
                  background: "none", border: "none", cursor: "pointer",
                  color: "var(--text-tertiary)", fontSize: F.small, padding: 4, lineHeight: 1,
                }}>✕</button>
              </div>
              <MarketplaceSourceEditor
                source={mktConfig.source ?? { source: "github" }}
                onChange={(s) => updateMarketplace(name, { ...mktConfig, source: s })}
              />
              {/* Path field — local installation path */}
              <div style={{ display: "flex", gap: 6, alignItems: "center", marginTop: 2 }}>
                <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: 80, flexShrink: 0, whiteSpace: "nowrap" }}>Path</span>
                <input className="input" style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
                  placeholder="本地安装路径（留空自动管理）"
                  value={mktConfig.path ?? ""}
                  onChange={(e) => updateMarketplace(name, { ...mktConfig, path: e.target.value || undefined })}
                />
              </div>
            </div>
          ))}
          <div style={{ display: "flex", gap: 6, alignItems: "center", marginTop: 2 }}>
            <input
              className="input"
              style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
              placeholder="marketplace-name"
              value={newMktName}
              onChange={(e) => setNewMktName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addMarketplace()}
            />
            <button type="button" className="btn btn-ghost" style={{ fontSize: F.small, padding: "4px 12px" }}
              onClick={addMarketplace}>+</button>
          </div>
        </div>
      </div>
    </div>
  );
}

/** Plugins with Section wrapper — for card-based layout */
function PluginsSection({
  config,
  updateField,
  t,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  return (
    <Section title={t("settings.sectionPlugins")} defaultOpen>
      <PluginsEditor config={config} updateField={updateField} />
    </Section>
  );
}

/** Plugins without Section wrapper — for tab content pane */
function PluginsSectionInline({ config, updateField }: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
}) {
  return <PluginsEditor config={config} updateField={updateField} />;
}

// ─── Skills Section (structured editor) ─────────────────────

/**
 * Known built-in (non-plugin) skills for skillOverrides.
 * NOTE: skillOverrides does NOT apply to plugin skills — those are managed via /plugin.
 * This list covers only built-in Claude Code skills and non-plugin user-invocable skills.
 */
// ─── Hooks Section (friendly editor) ────────────────────────

const HOOK_EVENTS: { id: string; label: string; desc: string; hasMatcher: boolean; matcherOptions: string[]; matcherFreeform: boolean }[] = [
  { id: "SessionStart", label: "会话启动", desc: "会话启动或恢复时触发", hasMatcher: true, matcherOptions: ["startup", "resume", "clear", "compact"], matcherFreeform: false },
  { id: "UserPromptSubmit", label: "提交提示", desc: "用户提交提示时触发", hasMatcher: false, matcherOptions: [], matcherFreeform: false },
  { id: "PreToolUse", label: "工具调用前", desc: "工具调用前触发，可阻止", hasMatcher: true, matcherOptions: ["Bash", "Edit", "Write", "Read", "Glob", "Grep", "WebFetch", "Agent"], matcherFreeform: true },
  { id: "PostToolUse", label: "工具调用后", desc: "工具调用成功后触发", hasMatcher: true, matcherOptions: ["Bash", "Edit", "Write", "Read", "Glob", "Grep", "WebFetch", "Agent"], matcherFreeform: true },
  { id: "Notification", label: "通知", desc: "发送通知时触发", hasMatcher: true, matcherOptions: ["permission_prompt", "idle_prompt", "auth_success", "elicitation_dialog"], matcherFreeform: false },
  { id: "Stop", label: "停止", desc: "Claude 完成响应时触发", hasMatcher: false, matcherOptions: [], matcherFreeform: false },
  { id: "SubagentStop", label: "子代理停止", desc: "子代理完成时触发", hasMatcher: true, matcherOptions: ["general-purpose", "Explore", "Plan"], matcherFreeform: true },
  { id: "ConfigChange", label: "配置变更", desc: "配置文件变更时触发", hasMatcher: true, matcherOptions: ["user_settings", "project_settings", "local_settings", "policy_settings", "skills"], matcherFreeform: false },
  { id: "FileChanged", label: "文件变更", desc: "监视文件变更时触发", hasMatcher: true, matcherOptions: [], matcherFreeform: true },
  { id: "CwdChanged", label: "目录切换", desc: "工作目录切换时触发", hasMatcher: false, matcherOptions: [], matcherFreeform: false },
  { id: "PreCompact", label: "压缩前", desc: "上下文压缩前触发", hasMatcher: true, matcherOptions: ["manual", "auto"], matcherFreeform: false },
  { id: "SessionEnd", label: "会话结束", desc: "会话结束时触发", hasMatcher: true, matcherOptions: ["clear", "resume", "logout", "prompt_input_exit", "other"], matcherFreeform: false },
];

const HANDLER_TYPES = ["command", "http", "mcp_tool", "prompt", "agent"] as const;
type HandlerType = (typeof HANDLER_TYPES)[number];

const HANDLER_LABELS: Record<HandlerType, string> = {
  command: "命令",
  http: "HTTP",
  mcp_tool: "MCP 工具",
  prompt: "LLM 提示",
  agent: "Agent 验证",
};

type HookHandler = {
  type: HandlerType;
  command?: string;
  args?: string[];
  url?: string;
  headers?: Record<string, string>;
  allowedEnvVars?: string[];
  server?: string;
  tool?: string;
  input?: Record<string, any>;
  prompt?: string;
  model?: string;
  timeout?: number;
  async?: boolean;
  "if"?: string;
  statusMessage?: string;
  shell?: string;
};

type MatcherGroup = {
  matcher: string;
  hooks: HookHandler[];
};

type HooksConfig = Record<string, MatcherGroup[]>;

/** Reusable field-row with inline label for handler cards */
function FieldRow({ label, icon, children }: {
  label: string; icon?: React.ReactNode; children: React.ReactNode;
}) {
  return (
    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
      <span style={{
        fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0, fontWeight: 500,
        display: "flex", alignItems: "center", gap: 4, minWidth: 80, whiteSpace: "nowrap",
      }}>
        {icon}{label}
      </span>
      <div style={{ flex: 1, minWidth: 0, display: "flex", gap: 8, alignItems: "center" }}>
        {children}
      </div>
    </div>
  );
}

function HooksSection({
  hooksValue,
  updateField,
  t,
}: {
  hooksValue: HooksConfig | undefined;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const hooks: HooksConfig = hooksValue ?? {};
  const [expandedEvent, setExpandedEvent] = useState<string | null>(null);

  // Count total hooks for badge
  const totalHooks = Object.values(hooks).reduce((sum, groups) => sum + groups.reduce((s, g) => s + g.hooks.length, 0), 0);

  const syncHooks = (updated: HooksConfig) => {
    const cleaned: HooksConfig = {};
    for (const [evt, groups] of Object.entries(updated)) {
      const nonEmpty = groups.filter(g => g.hooks.length > 0);
      if (nonEmpty.length > 0) cleaned[evt] = nonEmpty;
    }
    updateField("hooks", Object.keys(cleaned).length > 0 ? cleaned : undefined);
  };

  const addMatcherGroup = (eventId: string) => {
    const updated = { ...hooks };
    const existing = updated[eventId] ?? [];
    updated[eventId] = [...existing, { matcher: "", hooks: [{ type: "command" as HandlerType, command: "" }] }];
    syncHooks(updated);
    setExpandedEvent(eventId);
  };

  const removeMatcherGroup = (eventId: string, groupIdx: number) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    groups.splice(groupIdx, 1);
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const updateMatcher = (eventId: string, groupIdx: number, matcher: string) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    groups[groupIdx] = { ...groups[groupIdx], matcher };
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const addHandler = (eventId: string, groupIdx: number) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    const group = { ...groups[groupIdx], hooks: [...groups[groupIdx].hooks, { type: "command" as HandlerType, command: "" }] };
    groups[groupIdx] = group;
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const removeHandler = (eventId: string, groupIdx: number, handlerIdx: number) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    const handlers = [...groups[groupIdx].hooks];
    handlers.splice(handlerIdx, 1);
    groups[groupIdx] = { ...groups[groupIdx], hooks: handlers };
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const updateHandler = (eventId: string, groupIdx: number, handlerIdx: number, patch: Partial<HookHandler>) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    const handlers = [...groups[groupIdx].hooks];
    handlers[handlerIdx] = { ...handlers[handlerIdx], ...patch };
    groups[groupIdx] = { ...groups[groupIdx], hooks: handlers };
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const eventHookCount = (eventId: string) => {
    const groups = hooks[eventId];
    if (!groups) return 0;
    return groups.reduce((s, g) => s + g.hooks.length, 0);
  };

  const inputStyle: React.CSSProperties = {
    fontSize: F.body,
    padding: S.inputPad,
    minWidth: 0,
  };

  return (
    <Section title={`${t("settings.sectionHooks")}${totalHooks > 0 ? ` (${totalHooks})` : ""}`} defaultOpen={totalHooks > 0}>
      {/* Event selector — add new hook */}
      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <select
          className="input"
          style={{ fontSize: F.body, padding: S.inputPad, flex: 1, minWidth: 200 }}
          value=""
          onChange={(e) => {
            if (e.target.value) addMatcherGroup(e.target.value);
          }}
        >
          <option value="">+ 添加 Hook 事件…</option>
          {HOOK_EVENTS.map(ev => (
            <option key={ev.id} value={ev.id}>
              {ev.id} — {ev.desc}
            </option>
          ))}
        </select>
      </div>

      {/* Hint */}
      {totalHooks === 0 && (
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
          Hooks 在 Claude Code 生命周期的特定点自动执行命令/HTTP请求/LLM提示。
          <br />选择事件类型开始配置。
        </div>
      )}

      {/* Configured events */}
      {Object.entries(hooks).map(([eventId, groups]) => {
        const eventMeta = HOOK_EVENTS.find(e => e.id === eventId);
        const isExpanded = expandedEvent === eventId || groups.length > 0;
        const count = eventHookCount(eventId);

        return (
          <div
            key={eventId}
            style={{
              background: "var(--bg-glass)",
              border: "1px solid var(--border)",
              borderRadius: "var(--radius-md)",
              padding: "16px 20px",
              display: "flex",
              flexDirection: "column",
              gap: 14,
            }}
          >
            {/* Event header */}
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span
                style={{ cursor: "pointer", userSelect: "none", fontSize: F.small, color: "var(--text-tertiary)",
                  transition: "transform 0.2s", transform: isExpanded ? "rotate(90deg)" : "rotate(0deg)"
                }}
                onClick={() => setExpandedEvent(isExpanded ? null : eventId)}
              >
                ▶
              </span>
              <span style={{ fontSize: 16, fontWeight: 600, color: "var(--accent)" }}>
                {eventId}
              </span>
              {eventMeta && (
                <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>
                  — {eventMeta.desc}
                </span>
              )}
              <span style={{
                fontSize: 12, fontWeight: 600, padding: "2px 10px", borderRadius: 10,
                background: "var(--accent-subtle)", color: "var(--accent)", marginLeft: "auto",
              }}>
                {count} handler{count !== 1 ? "s" : ""}
              </span>
              <button
                type="button"
                className="btn btn-ghost btn-icon"
                style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)" }}
                onClick={() => {
                  const updated = { ...hooks };
                  delete updated[eventId];
                  syncHooks(updated);
                }}
                title="删除此事件所有 hooks"
              >
                ×
              </button>
            </div>

            {/* Matcher groups */}
            {isExpanded && groups.map((group, gi) => {
              // Parse current matcher into selected tags
              const matcherTags = group.matcher ? group.matcher.split("|").map(s => s.trim()).filter(Boolean) : [];
              const toggleMatcherTag = (tag: string) => {
                const next = matcherTags.includes(tag)
                  ? matcherTags.filter(t => t !== tag)
                  : [...matcherTags, tag];
                updateMatcher(eventId, gi, next.join("|"));
              };

              return (
              <div
                key={gi}
                style={{
                  borderLeft: "3px solid var(--accent)",
                  paddingLeft: 16,
                  display: "flex",
                  flexDirection: "column",
                  gap: 12,
                }}
              >
                {/* Matcher: tag chips or freeform input */}
                <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                  <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0, fontWeight: 500 }}>
                    匹配器
                  </span>
                  {eventMeta && eventMeta.matcherOptions.length > 0 ? (
                    <>
                      {eventMeta.matcherOptions.map(opt => {
                        const selected = matcherTags.includes(opt);
                        return (
                          <button
                            key={opt}
                            type="button"
                            className="btn btn-ghost"
                            style={{
                              fontSize: 13,
                              padding: "4px 12px",
                              borderRadius: 16,
                              fontWeight: selected ? 600 : 400,
                              background: selected ? "var(--accent-subtle)" : "transparent",
                              color: selected ? "var(--accent)" : "var(--text-secondary)",
                              border: selected ? "1px solid var(--accent)" : "1px solid var(--border)",
                              transition: "all 150ms",
                            }}
                            onClick={() => toggleMatcherTag(opt)}
                          >
                            {opt}
                          </button>
                        );
                      })}
                      {/* Selected indicator */}
                      {matcherTags.length > 0 && !matcherTags.every(t => eventMeta.matcherOptions.includes(t)) && (
                        <span style={{ fontSize: F.hint, color: "var(--accent)" }}>
                          + 自定义: {matcherTags.filter(t => !eventMeta.matcherOptions.includes(t)).join(", ")}
                        </span>
                      )}
                    </>
                  ) : eventMeta?.matcherFreeform ? (
                    <input
                      className="input"
                      style={{ ...inputStyle, flex: 1 }}
                      placeholder={eventMeta?.id === "FileChanged" ? "文件名，如 .envrc|.env" : "工具名称或正则，多个用 | 分隔"}
                      value={group.matcher}
                      onChange={(e) => updateMatcher(eventId, gi, e.target.value)}
                    />
                  ) : (
                    <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>匹配所有</span>
                  )}
                  <button
                    type="button"
                    className="btn btn-ghost btn-icon"
                    style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)" }}
                    onClick={() => removeMatcherGroup(eventId, gi)}
                    title="删除此匹配器组"
                  >
                    ×
                  </button>
                </div>

                {/* Handlers — each in its own sub-card */}
                {group.hooks.map((handler, hi) => (
                  <div
                    key={hi}
                    style={{
                      marginLeft: 72,
                      background: "var(--bg-surface)",
                      border: "1px solid var(--border)",
                      borderRadius: "var(--radius-sm)",
                      padding: "14px 16px",
                      display: "flex",
                      flexDirection: "column",
                      gap: 10,
                    }}
                  >
                    {/* Header: type selector + delete */}
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <span style={{
                        fontSize: 13, fontWeight: 600, padding: "3px 10px", borderRadius: 6,
                        background: "var(--bg-glass)", color: "var(--accent)", border: "1px solid var(--border)",
                        flexShrink: 0,
                      }}>
                        {HANDLER_LABELS[handler.type]}
                      </span>
                      <select
                        className="input"
                        style={{ ...inputStyle, width: 130, flexShrink: 0 }}
                        value={handler.type}
                        onChange={(e) => updateHandler(eventId, gi, hi, { type: e.target.value as HandlerType })}
                      >
                        {HANDLER_TYPES.map(ht => (
                          <option key={ht} value={ht}>{HANDLER_LABELS[ht]}</option>
                        ))}
                      </select>
                      <button
                        type="button"
                        className="btn btn-ghost btn-icon"
                        style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)", marginLeft: "auto" }}
                        onClick={() => removeHandler(eventId, gi, hi)}
                        title="删除此处理器"
                      >
                        ×
                      </button>
                    </div>

                    {/* Command — textarea + shell selector on own row */}
                    {handler.type === "command" && (
                      <>
                        <FieldRow label="命令" icon={<SectionIcon name="bolt" size={13} />}>
                          <textarea
                            className="input"
                            style={{
                              flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0,
                              fontFamily: '"SF Mono", "Fira Code", monospace', lineHeight: 1.5,
                              minHeight: 56, resize: "vertical",
                            }}
                            placeholder="命令或脚本路径，如 ./scripts/check.sh&#10;支持多行命令，每行独立执行"
                            value={handler.command ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { command: e.target.value || undefined })}
                          />
                        </FieldRow>
                        <FieldRow label="Shell" icon={<SectionIcon name="advanced" size={13} />}>
                          <select
                            className="input"
                            style={{ ...inputStyle, width: 140 }}
                            value={handler.shell ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { shell: e.target.value || undefined })}
                          >
                            <option value="">Bash</option>
                            <option value="powershell">PowerShell</option>
                          </select>
                        </FieldRow>
                      </>
                    )}
                    {/* HTTP URL */}
                    {handler.type === "http" && (
                      <FieldRow label="URL" icon={<SectionIcon name="network" size={13} />}>
                        <input
                          className="input"
                          style={{ ...inputStyle, flex: 1 }}
                          placeholder="HTTP URL，如 http://localhost:8080/hooks/pre-tool-use"
                          value={handler.url ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { url: e.target.value || undefined })}
                        />
                      </FieldRow>
                    )}
                    {/* MCP Tool — server + tool each on own row */}
                    {handler.type === "mcp_tool" && (
                      <>
                        <FieldRow label="服务器" icon={<SectionIcon name="network" size={13} />}>
                          <input
                            className="input"
                            style={{ ...inputStyle, flex: 1 }}
                            placeholder="MCP 服务器名称"
                            value={handler.server ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { server: e.target.value || undefined })}
                          />
                        </FieldRow>
                        <FieldRow label="工具" icon={<SectionIcon name="advanced" size={13} />}>
                          <input
                            className="input"
                            style={{ ...inputStyle, flex: 1 }}
                            placeholder="工具名称"
                            value={handler.tool ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { tool: e.target.value || undefined })}
                          />
                        </FieldRow>
                      </>
                    )}
                    {/* Prompt / Agent — textarea */}
                    {(handler.type === "prompt" || handler.type === "agent") && (
                      <FieldRow label="提示" icon={<SectionIcon name="behavior" size={13} />}>
                        <textarea
                          className="input"
                          style={{
                            flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0,
                            fontFamily: '"SF Mono", "Fira Code", monospace', lineHeight: 1.5,
                            minHeight: 56, resize: "vertical",
                          }}
                          placeholder="提示文本，用 $ARGUMENTS 插入 hook 输入数据&#10;支持多行提示内容"
                          value={handler.prompt ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { prompt: e.target.value || undefined })}
                        />
                      </FieldRow>
                    )}

                    {/* ── Auxiliary options, each on its own row ── */}
                    {eventMeta?.hasMatcher && (
                      <FieldRow label="条件 if" icon={<SectionIcon name="permissions" size={13} />}>
                        <input
                          className="input"
                          style={{ ...inputStyle, flex: 1, fontSize: F.hint }}
                          placeholder="匹配条件，如 Bash(rm *)"
                          value={handler["if"] ?? ""}
                          onChange={(e) => {
                            const patch: Partial<HookHandler> = {};
                            if (e.target.value) (patch as any)["if"] = e.target.value;
                            else (patch as any)["if"] = undefined;
                            updateHandler(eventId, gi, hi, patch);
                          }}
                        />
                      </FieldRow>
                    )}
                    <FieldRow label="超时" icon={<SectionIcon name="status" size={13} />}>
                      <input
                        className="input"
                        style={{ ...inputStyle, width: 80, fontSize: F.hint }}
                        type="number"
                        placeholder="600"
                        value={handler.timeout ?? ""}
                        onChange={(e) => updateHandler(eventId, gi, hi, { timeout: e.target.value ? Number(e.target.value) : undefined })}
                      />
                      <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>秒</span>
                    </FieldRow>
                    {handler.type === "command" && (
                      <FieldRow label="async" icon={<SectionIcon name="ui" size={13} />}>
                        <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: F.hint, color: "var(--text-tertiary)", cursor: "pointer" }}>
                          <Toggle active={!!handler.async} onChange={(v) => updateHandler(eventId, gi, hi, { async: v || undefined })} />
                          后台运行（不阻塞主流程）
                        </label>
                      </FieldRow>
                    )}
                    <FieldRow label="状态" icon={<SectionIcon name="status" size={13} />}>
                      <input
                        className="input"
                        style={{ ...inputStyle, flex: 1, fontSize: F.hint }}
                        placeholder="运行时显示的状态消息"
                        value={handler.statusMessage ?? ""}
                        onChange={(e) => updateHandler(eventId, gi, hi, { statusMessage: e.target.value || undefined })}
                      />
                    </FieldRow>
                  </div>
                ))}

                {/* Add handler button */}
                <button
                  type="button"
                  className="btn btn-ghost"
                  style={{ fontSize: F.hint, padding: "6px 14px", alignSelf: "flex-start", marginLeft: 72 }}
                  onClick={() => addHandler(eventId, gi)}
                >
                  + 处理器
                </button>
              </div>
            );
            })}


            {/* Add matcher group to existing event */}
            {isExpanded && (
              <button
                type="button"
                className="btn btn-ghost"
                style={{ fontSize: F.hint, padding: "6px 14px", alignSelf: "flex-start" }}
                onClick={() => addMatcherGroup(eventId)}
              >
                + 匹配器组
              </button>
            )}
          </div>
        );
      })}
    </Section>
  );
}

/** Hooks without Section wrapper — for tab content pane */
function HooksSectionInline(props: {
  hooksValue: HooksConfig | undefined;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  // Reuse same logic but render flat — extract hooks data from props
  const { hooksValue, updateField } = props;
  const hooks: HooksConfig = hooksValue ?? {};
  const [expandedEvent, setExpandedEvent] = useState<string | null>(null);

  const totalHooks = Object.values(hooks).reduce((sum, groups) => sum + groups.reduce((s, g) => s + g.hooks.length, 0), 0);

  const syncHooks = (updated: HooksConfig) => {
    const cleaned: HooksConfig = {};
    for (const [evt, groups] of Object.entries(updated)) {
      const nonEmpty = groups.filter(g => g.hooks.length > 0);
      if (nonEmpty.length > 0) cleaned[evt] = nonEmpty;
    }
    updateField("hooks", Object.keys(cleaned).length > 0 ? cleaned : undefined);
  };

  const addMatcherGroup = (eventId: string) => {
    const updated = { ...hooks };
    const existing = updated[eventId] ?? [];
    updated[eventId] = [...existing, { matcher: "", hooks: [{ type: "command" as HandlerType, command: "" }] }];
    syncHooks(updated);
    setExpandedEvent(eventId);
  };

  const removeMatcherGroup = (eventId: string, groupIdx: number) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    groups.splice(groupIdx, 1);
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const updateMatcher = (eventId: string, groupIdx: number, matcher: string) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    groups[groupIdx] = { ...groups[groupIdx], matcher };
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const addHandler = (eventId: string, groupIdx: number) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    const group = { ...groups[groupIdx], hooks: [...groups[groupIdx].hooks, { type: "command" as HandlerType, command: "" }] };
    groups[groupIdx] = group;
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const removeHandler = (eventId: string, groupIdx: number, handlerIdx: number) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    const handlers = [...groups[groupIdx].hooks];
    handlers.splice(handlerIdx, 1);
    groups[groupIdx] = { ...groups[groupIdx], hooks: handlers };
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const updateHandler = (eventId: string, groupIdx: number, handlerIdx: number, patch: Partial<HookHandler>) => {
    const updated = { ...hooks };
    const groups = [...(updated[eventId] ?? [])];
    const handlers = [...groups[groupIdx].hooks];
    handlers[handlerIdx] = { ...handlers[handlerIdx], ...patch };
    groups[groupIdx] = { ...groups[groupIdx], hooks: handlers };
    updated[eventId] = groups;
    syncHooks(updated);
  };

  const eventHookCount = (eventId: string) => {
    const groups = hooks[eventId];
    if (!groups) return 0;
    return groups.reduce((s, g) => s + g.hooks.length, 0);
  };

  const inputStyle: React.CSSProperties = { fontSize: F.body, padding: S.inputPad, minWidth: 0 };

  // Render the same JSX as HooksSection but without <Section> wrapper
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {/* Event selector */}
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <select className="input" style={{ fontSize: F.body, padding: S.inputPad, flex: 1, minWidth: 200 }} value=""
          onChange={(e) => { if (e.target.value) addMatcherGroup(e.target.value); }}>
          <option value="">+ 添加 Hook 事件…</option>
          {HOOK_EVENTS.map(ev => (
            <option key={ev.id} value={ev.id}>{ev.id} — {ev.desc}</option>
          ))}
        </select>
      </div>

      {totalHooks === 0 && (
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
          Hooks 在 Claude Code 生命周期的特定点自动执行命令/HTTP请求/LLM提示。
          <br />选择事件类型开始配置。
        </div>
      )}

      {/* Reuse exact same event rendering as HooksSection — copy the JSX */}
      {Object.entries(hooks).map(([eventId, groups]) => {
        const eventMeta = HOOK_EVENTS.find(e => e.id === eventId);
        const isExpanded = expandedEvent === eventId || groups.length > 0;
        const count = eventHookCount(eventId);

        return (
          <div key={eventId} style={{
            background: "var(--bg-glass)", border: "1px solid var(--border)",
            borderRadius: "var(--radius-md)", padding: "16px 20px",
            display: "flex", flexDirection: "column", gap: 14,
          }}>
            {/* Event header */}
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span style={{ cursor: "pointer", userSelect: "none", fontSize: F.small, color: "var(--text-tertiary)",
                transition: "transform 0.2s", transform: isExpanded ? "rotate(90deg)" : "rotate(0deg)" }}
                onClick={() => setExpandedEvent(isExpanded ? null : eventId)}>▶</span>
              <span style={{ fontSize: 16, fontWeight: 600, color: "var(--accent)" }}>{eventId}</span>
              {eventMeta && <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>— {eventMeta.desc}</span>}
              <span style={{ fontSize: 12, fontWeight: 600, padding: "2px 10px", borderRadius: 10,
                background: "var(--accent-subtle)", color: "var(--accent)", marginLeft: "auto" }}>
                {count} handler{count !== 1 ? "s" : ""}
              </span>
              <button type="button" className="btn btn-ghost btn-icon"
                style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)" }}
                onClick={() => { const u = { ...hooks }; delete u[eventId]; syncHooks(u); }} title="删除">×
              </button>
            </div>

            {/* Matcher groups + handlers — same as HooksSection */}
            {isExpanded && groups.map((group, gi) => {
              const matcherTags = group.matcher ? group.matcher.split("|").map(s => s.trim()).filter(Boolean) : [];
              const toggleMatcherTag = (tag: string) => {
                const next = matcherTags.includes(tag) ? matcherTags.filter(t => t !== tag) : [...matcherTags, tag];
                updateMatcher(eventId, gi, next.join("|"));
              };
              return (
                <div key={gi} style={{ borderLeft: "3px solid var(--accent)", paddingLeft: 16, display: "flex", flexDirection: "column", gap: 12 }}>
                  <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                    <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0, fontWeight: 500 }}>匹配器</span>
                    {eventMeta && eventMeta.matcherOptions.length > 0 ? (
                      <>
                        {eventMeta.matcherOptions.map(opt => {
                          const selected = matcherTags.includes(opt);
                          return (
                            <button key={opt} type="button" className="btn btn-ghost" style={{
                              fontSize: 13, padding: "4px 12px", borderRadius: 16,
                              fontWeight: selected ? 600 : 400,
                              background: selected ? "var(--accent-subtle)" : "transparent",
                              color: selected ? "var(--accent)" : "var(--text-secondary)",
                              border: selected ? "1px solid var(--accent)" : "1px solid var(--border)",
                            }} onClick={() => toggleMatcherTag(opt)}>{opt}</button>
                          );
                        })}
                      </>
                    ) : eventMeta?.matcherFreeform ? (
                      <input className="input" style={{ ...inputStyle, flex: 1 }}
                        placeholder={eventMeta?.id === "FileChanged" ? "文件名，如 .envrc|.env" : "工具名称或正则，多个用 | 分隔"}
                        value={group.matcher} onChange={(e) => updateMatcher(eventId, gi, e.target.value)} />
                    ) : (
                      <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>匹配所有</span>
                    )}
                    <button type="button" className="btn btn-ghost btn-icon"
                      style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)" }}
                      onClick={() => removeMatcherGroup(eventId, gi)} title="删除">×
                    </button>
                  </div>

                  {/* Handlers */}
                  {group.hooks.map((handler, hi) => (
                    <div key={hi} style={{
                      marginLeft: 72, background: "var(--bg-surface)", border: "1px solid var(--border)",
                      borderRadius: "var(--radius-sm)", padding: "14px 16px",
                      display: "flex", flexDirection: "column", gap: 10,
                    }}>
                      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                        <span style={{ fontSize: 13, fontWeight: 600, padding: "3px 10px", borderRadius: 6,
                          background: "var(--bg-glass)", color: "var(--accent)", border: "1px solid var(--border)", flexShrink: 0 }}>
                          {HANDLER_LABELS[handler.type]}
                        </span>
                        <select className="input" style={{ ...inputStyle, width: 130, flexShrink: 0 }}
                          value={handler.type} onChange={(e) => updateHandler(eventId, gi, hi, { type: e.target.value as HandlerType })}>
                          {HANDLER_TYPES.map(ht => <option key={ht} value={ht}>{HANDLER_LABELS[ht]}</option>)}
                        </select>
                        <button type="button" className="btn btn-ghost btn-icon"
                          style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)", marginLeft: "auto" }}
                          onClick={() => removeHandler(eventId, gi, hi)} title="删除">×
                        </button>
                      </div>

                      {handler.type === "command" && (
                        <>
                          <FieldRow label="命令" icon={<SectionIcon name="bolt" size={13} />}>
                            <textarea
                              className="input"
                              style={{
                                flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0,
                                fontFamily: '"SF Mono", "Fira Code", monospace', lineHeight: 1.5,
                                minHeight: 56, resize: "vertical",
                              }}
                              placeholder="命令或脚本路径，如 ./scripts/check.sh&#10;支持多行命令，每行独立执行"
                              value={handler.command ?? ""}
                              onChange={(e) => updateHandler(eventId, gi, hi, { command: e.target.value || undefined })}
                            />
                          </FieldRow>
                          <FieldRow label="Shell" icon={<SectionIcon name="advanced" size={13} />}>
                            <select className="input" style={{ ...inputStyle, width: 140 }}
                              value={handler.shell ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { shell: e.target.value || undefined })}>
                              <option value="">Bash</option><option value="powershell">PowerShell</option>
                            </select>
                          </FieldRow>
                        </>
                      )}
                      {handler.type === "http" && (
                        <FieldRow label="URL" icon={<SectionIcon name="network" size={13} />}>
                          <input className="input" style={{ ...inputStyle, flex: 1 }} placeholder="HTTP URL，如 http://localhost:8080/hooks/pre-tool-use"
                            value={handler.url ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { url: e.target.value || undefined })} />
                        </FieldRow>
                      )}
                      {handler.type === "mcp_tool" && (
                        <>
                          <FieldRow label="服务器" icon={<SectionIcon name="network" size={13} />}>
                            <input className="input" style={{ ...inputStyle, flex: 1 }} placeholder="MCP 服务器名称"
                              value={handler.server ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { server: e.target.value || undefined })} />
                          </FieldRow>
                          <FieldRow label="工具" icon={<SectionIcon name="advanced" size={13} />}>
                            <input className="input" style={{ ...inputStyle, flex: 1 }} placeholder="工具名称"
                              value={handler.tool ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { tool: e.target.value || undefined })} />
                          </FieldRow>
                        </>
                      )}
                      {(handler.type === "prompt" || handler.type === "agent") && (
                        <FieldRow label="提示" icon={<SectionIcon name="behavior" size={13} />}>
                          <textarea
                            className="input"
                            style={{
                              flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0,
                              fontFamily: '"SF Mono", "Fira Code", monospace', lineHeight: 1.5,
                              minHeight: 56, resize: "vertical",
                            }}
                            placeholder="提示文本，用 $ARGUMENTS 插入 hook 输入数据&#10;支持多行提示内容"
                            value={handler.prompt ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { prompt: e.target.value || undefined })}
                          />
                        </FieldRow>
                      )}

                      {eventMeta?.hasMatcher && (
                        <FieldRow label="条件 if" icon={<SectionIcon name="permissions" size={13} />}>
                          <input className="input" style={{ ...inputStyle, flex: 1, fontSize: F.hint }} placeholder="匹配条件，如 Bash(rm *)"
                            value={handler["if"] ?? ""} onChange={(e) => {
                              const patch: Partial<HookHandler> = {};
                              if (e.target.value) (patch as any)["if"] = e.target.value;
                              else (patch as any)["if"] = undefined;
                              updateHandler(eventId, gi, hi, patch);
                            }} />
                        </FieldRow>
                      )}
                      <FieldRow label="超时" icon={<SectionIcon name="status" size={13} />}>
                        <input className="input" style={{ ...inputStyle, width: 80, fontSize: F.hint }} type="number" placeholder="600"
                          value={handler.timeout ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { timeout: e.target.value ? Number(e.target.value) : undefined })} />
                        <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>秒</span>
                      </FieldRow>
                      {handler.type === "command" && (
                        <FieldRow label="async" icon={<SectionIcon name="ui" size={13} />}>
                          <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: F.hint, color: "var(--text-tertiary)", cursor: "pointer" }}>
                            <Toggle active={!!handler.async} onChange={(v) => updateHandler(eventId, gi, hi, { async: v || undefined })} />
                            后台运行（不阻塞主流程）
                          </label>
                        </FieldRow>
                      )}
                      <FieldRow label="状态" icon={<SectionIcon name="status" size={13} />}>
                        <input className="input" style={{ ...inputStyle, flex: 1, fontSize: F.hint }}
                          placeholder="运行时显示的状态消息" value={handler.statusMessage ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { statusMessage: e.target.value || undefined })} />
                      </FieldRow>
                    </div>
                  ))}

                  <button type="button" className="btn btn-ghost"
                    style={{ fontSize: F.hint, padding: "6px 14px", alignSelf: "flex-start", marginLeft: 72 }}
                    onClick={() => addHandler(eventId, gi)}>+ 处理器</button>
                </div>
              );
            })}

            {isExpanded && (
              <button type="button" className="btn btn-ghost"
                style={{ fontSize: F.hint, padding: "6px 14px", alignSelf: "flex-start" }}
                onClick={() => addMatcherGroup(eventId)}>+ 匹配器组</button>
            )}
          </div>
        );
      })}
    </div>
  );
}

// ─── Path Input (text + system picker + autocomplete) ─────

interface PathSuggestion {
  name: string;
  full_path: string;
  is_dir: boolean;
  modified: number;
}

function PathInput({
  value,
  onChange,
  pathType,
  placeholder,
}: {
  value: string | undefined;
  onChange: (v: string | undefined) => void;
  pathType: "file" | "directory";
  placeholder?: string;
}) {
  const [suggestions, setSuggestions] = useState<PathSuggestion[]>([]);
  const [showSugg, setShowSugg] = useState(false);
  const [hlIdx, setHlIdx] = useState(-1);
  const [timer, setTimer] = useState<ReturnType<typeof setTimeout> | null>(null);

  const fetchSuggestions = useCallback((input: string) => {
    if (timer) clearTimeout(timer);
    if (!input || input.length < 1) {
      setSuggestions([]);
      setShowSugg(false);
      return;
    }
    const t = setTimeout(async () => {
      try {
        let result: PathSuggestion[] = [];
        if ((window as any).__TAURI_INTERNALS__) {
          const core = await import("@tauri-apps/api/core");
          result = await core.invoke<PathSuggestion[]>("fs_autocomplete", { input });
        }
        setSuggestions(result);
        setShowSugg(result.length > 0);
        setHlIdx(-1);
      } catch {
        setSuggestions([]);
        setShowSugg(false);
      }
    }, 150);
    setTimer(t);
  }, [timer]);

  const pick = async () => {
    try {
      const selected = await open({
        directory: pathType === "directory",
        multiple: false,
        title: pathType === "directory" ? "选择目录" : "选择文件",
      });
      if (selected) onChange(selected);
    } catch {
      // user cancelled
    }
  };

  const selectSuggestion = (s: PathSuggestion) => {
    // For directory picker, if user selects a dir, append "/" so they can drill deeper
    if (s.is_dir) {
      onChange(s.full_path + "/");
      fetchSuggestions(s.full_path + "/");
    } else {
      onChange(s.full_path);
      setShowSugg(false);
    }
  };

  const formatTime = (ts: number) => {
    if (!ts) return "";
    const d = new Date(ts * 1000);
    const now = new Date();
    const diffMs = now.getTime() - d.getTime();
    const diffDays = Math.floor(diffMs / 86400000);
    if (diffDays === 0) {
      const diffH = Math.floor(diffMs / 3600000);
      return diffH === 0 ? "刚刚" : `${diffH}小时前`;
    }
    if (diffDays < 30) return `${diffDays}天前`;
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4, position: "relative" }}>
      <div style={{ display: "flex", gap: 6 }}>
        <input
          className="input"
          style={{ flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0 }}
          placeholder={placeholder ?? (pathType === "directory" ? "选择目录或直接输入路径…" : "选择文件或直接输入路径…")}
          value={value ?? ""}
          onChange={(e) => {
            const v = e.target.value || undefined;
            onChange(v);
            fetchSuggestions(e.target.value);
          }}
          onFocus={() => {
            if (suggestions.length > 0) setShowSugg(true);
          }}
          onBlur={() => {
            setTimeout(() => setShowSugg(false), 200);
          }}
          onKeyDown={(e) => {
            if (!showSugg || suggestions.length === 0) return;
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setHlIdx(i => (i + 1) % suggestions.length);
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setHlIdx(i => (i <= 0 ? suggestions.length - 1 : i - 1));
            } else if (e.key === "Tab") {
              e.preventDefault();
              selectSuggestion(suggestions[hlIdx >= 0 ? hlIdx : 0]);
            } else if (e.key === "Enter" && hlIdx >= 0) {
              e.preventDefault();
              selectSuggestion(suggestions[hlIdx]);
            } else if (e.key === "Escape") {
              setShowSugg(false);
            }
          }}
        />
        <button
          type="button"
          className="btn btn-ghost"
          style={{ fontSize: F.body, padding: S.inputPad, flexShrink: 0 }}
          onClick={pick}
          title={pathType === "directory" ? "选择目录" : "选择文件"}
        >
          <SectionIcon name="folder" size={15} />
        </button>
      </div>

      {/* Autocomplete dropdown */}
      {showSugg && suggestions.length > 0 && (
        <div
          className="glass-elevated"
          style={{
            position: "absolute",
            top: "100%",
            left: 0,
            right: 36,
            marginTop: 2,
            maxHeight: 240,
            overflowY: "auto",
            zIndex: 200,
            padding: 4,
            animation: "fadeIn 120ms ease both",
          }}
        >
          {suggestions.map((s, i) => (
            <button
              key={s.full_path}
              type="button"
              className="btn btn-ghost"
              style={{
                width: "100%",
                justifyContent: "space-between",
                alignItems: "center",
                padding: "6px 10px",
                fontSize: 14,
                fontWeight: 400,
                color: "var(--text-primary)",
                borderRadius: "var(--radius-sm)",
                background: i === hlIdx ? "var(--accent-subtle)" : "transparent",
              }}
              onMouseDown={(e) => {
                e.preventDefault();
                selectSuggestion(s);
              }}
            >
              <span style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
                <span style={{ fontSize: 13, flexShrink: 0 }}>
                  {s.is_dir ? <SectionIcon name="folder" size={13} /> : <SectionIcon name="file" size={13} />}
                </span>
                <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {s.name}
                </span>
              </span>
              <span style={{ fontSize: 12, color: "var(--text-tertiary)", flexShrink: 0 }}>
                {formatTime(s.modified)}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Hint when empty and no suggestions */}
      {!value && !showSugg && (
        <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.4 }}>
          {pathType === "directory"
            ? "输入 ~/ 浏览主目录，支持 Tab 补全"
            : "输入路径浏览文件，如 ~/ 或 ./"}
        </span>
      )}
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
  // Shared left-right row style
  const rowStyle: React.CSSProperties = {
    display: "flex",
    alignItems: "flex-start",
    gap: 12,
  };

  switch (field.type) {
    case "boolean":
      return (
        <div style={{ ...rowStyle, alignItems: "center" }}>
          <FieldLabel field={field} t={t} style={{ paddingTop: 0 }} />
          <div style={{ flex: 1, minWidth: 0, display: "flex", justifyContent: "flex-end", paddingTop: 2 }}>
            <Toggle active={!!value} onChange={(v) => onChange(v || undefined)} />
          </div>
        </div>
      );

    case "select":
      return (
        <div style={rowStyle}>
          <FieldLabel field={field} t={t} />
          <select
            className="input"
            style={{ fontSize: F.body, padding: S.inputPad, flex: 1, minWidth: 0 }}
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
        <div style={rowStyle}>
          <FieldLabel field={field} t={t} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <JsonEditor value={value} onChange={onChange} placeholder="{}" />
          </div>
        </div>
      );

    case "kv":
      return (
        <div style={rowStyle}>
          <FieldLabel field={field} t={t} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <KvEditor
              items={(value && typeof value === "object" && !Array.isArray(value)) ? value as Record<string, string> : {}}
              onChange={(kv) => onChange(Object.keys(kv).length > 0 ? kv : undefined)}
            />
          </div>
        </div>
      );

    case "string[]":
      return (
        <div style={rowStyle}>
          <FieldLabel field={field} t={t} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <StringListEditor
              items={Array.isArray(value) ? value : []}
              onChange={(list) => onChange(list.length > 0 ? list : undefined)}
              addLabel={t("settings.addRule")}
            />
          </div>
        </div>
      );

    case "string":
    default:
      // Path-type string fields get picker + hint
      if (field.pathType) {
        return (
          <div style={rowStyle}>
            <FieldLabel field={field} t={t} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <PathInput
                value={value}
                onChange={onChange}
                pathType={field.pathType}
                placeholder={field.placeholder}
              />
            </div>
          </div>
        );
      }
      return (
        <div style={rowStyle}>
          <FieldLabel field={field} t={t} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <input
              className="input"
              style={{ fontSize: F.body, padding: S.inputPad, width: "100%" }}
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
  const [importDiff, setImportDiff] = useState<{
    source: Record<string, any>;
    diff: { key: string; current: any; incoming: any; selected: boolean }[];
  } | null>(null);

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

  const handleImportFromClaudeCode = async () => {
    try {
      const source = await claudeSettingsImportApi.readDefault();
      // Build diff: only top-level keys, skip _aidog_ internal keys
      const diff: { key: string; current: any; incoming: any; selected: boolean }[] = [];
      const allKeys = new Set([...Object.keys(config), ...Object.keys(source)]);
      for (const key of allKeys) {
        if (key.startsWith("_aidog_")) continue;
        const current = config[key];
        const incoming = source[key];
        if (JSON.stringify(current) === JSON.stringify(incoming)) continue;
        diff.push({ key, current, incoming, selected: true });
      }
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

  const applyImport = (selectedKeys: Set<string>) => {
    if (!importDiff) return;
    const next = { ...config };
    for (const key of selectedKeys) {
      next[key] = importDiff.source[key];
    }
    setConfig(next);
    setEditJson(JSON.stringify(next, null, 2));
    setImportDiff(null);
    setToast(t("settings.imported", "已导入"));
    setTimeout(() => setToast(""), 2000);
  };

  // Permissions helpers for the special permissions sub-editor
  const perms = (config.permissions ?? {}) as Record<string, any>;

  // Active section tab
  const [activeTab, setActiveTab] = useState("core");
  const [searchQuery, setSearchQuery] = useState("");

  // Render a single section's content (no card wrapper — card is the content pane)
  const renderSectionContent = (section: typeof SECTIONS[number]) => {
    if (section.id === "permissions") {
      return (
        <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
          {/* PermissionsSection renders its own Section card — unwrap it */}
          <PermissionsSection perms={perms} updateField={updateField} t={t} />
        </div>
      );
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
        <SandboxSection
          sandboxValue={config.sandbox as Record<string, any> | undefined}
          updateField={updateField}
          t={t}
        />
      );
    }

    if (section.id === "plugins") {
      return (
        <PluginsSection
          config={config}
          updateField={updateField}
          t={t}
        />
      );
    }

    if (section.id === "hooks") {
      return (
        <HooksSection
          hooksValue={config.hooks as HooksConfig | undefined}
          updateField={updateField}
          t={t}
        />
      );
    }

    if (section.id === "status") {
      return (
        <StatusLineSection
          config={config}
          updateField={updateField}
          t={t}
        />
      );
    }

    const visibleFields = section.fields.filter((f) => !f.skipGui);
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
        {visibleFields.map((field) => (
          <FieldRenderer
            key={field.key}
            field={field}
            value={config[field.key]}
            onChange={(v) => updateField(field.key, v)}
            t={t}
          />
        ))}
        {/* Attribution fixed editor (commit + pr only) */}
        {section.id === "advanced" && (() => {
          const attr = (config.attribution ?? {}) as Record<string, string>;
          const rowStyle: React.CSSProperties = { display: "flex", alignItems: "center", gap: 12 };
          return (
            <div style={{ display: "flex", flexDirection: "column", gap: S.row, borderTop: "1px solid var(--border)", paddingTop: S.gap }}>
              <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-secondary)" }}>
                {t("settings.f_attribution", "Attribution")}
              </div>
              {(["commit", "pr"] as const).map(field => (
                <div key={field} style={rowStyle}>
                  <label style={{ flexShrink: 0, width: 200, fontSize: F.label, fontWeight: 500, color: "var(--text-primary)", paddingTop: 10 }}>
                    {field === "commit" ? t("settings.attribution.commit", "Commit Author") : t("settings.attribution.pr", "PR Author")}
                    <span style={{ display: "block", fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginTop: 2 }}>{field}</span>
                  </label>
                  <input className="input" style={{ flex: 1, fontSize: F.body, padding: S.inputPad }}
                    placeholder={field === "commit" ? "e.g. Your Name <you@example.com>" : "e.g. Your Name <you@example.com>"}
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
      {/* Header bar */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 0 16px 0",
          flexShrink: 0,
        }}
      >
        <div>
          <div style={{ fontSize: 22, fontWeight: 700, color: "var(--text-primary)", letterSpacing: "-0.02em" }}>
            {t("settings.title")}
          </div>
          <div style={{ fontSize: F.body, color: "var(--text-secondary)", marginTop: 2 }}>
            {t("settings.desc")}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
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
          <div style={{ width: 1, height: 20, background: "var(--border)", margin: "0 4px" }} />
          <button
            className="btn btn-ghost"
            style={{ fontSize: F.hint, padding: "6px 14px" }}
            onClick={handleLoadRecommended}
          >
            <SectionIcon name="bolt" size={14} /> {t("settings.loadRecommended")}
          </button>
          <button
            className="btn btn-ghost"
            style={{ fontSize: F.hint, padding: "6px 14px" }}
            onClick={handleImportFromClaudeCode}
          >
            <SectionIcon name="folder" size={14} /> {t("settings.importFromClaudeCode", "从 Claude Code 导入")}
          </button>
          {toast && (
            <span style={{ fontSize: F.body, color: "#34c759" }}>{toast}</span>
          )}
          <button
            className="btn btn-primary"
            style={{ fontSize: F.body, padding: S.btnPad, minWidth: 80 }}
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? t("status.loading") : t("action.save")}
          </button>
        </div>
      </div>

      {/* JSON mode */}
      {mode === "json" && (
        <div
          className="glass-surface"
          style={{ flex: 1, display: "flex", flexDirection: "column", padding: S.pad, borderRadius: "var(--radius-lg)", overflow: "hidden" }}
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
      )}

      {/* GUI mode — VS Code sidebar + content */}
      {mode === "gui" && (
        <div style={{ display: "flex", flex: 1, minHeight: 0 }}>
          {/* ── Left sidebar ── */}
          <aside style={{
            width: 220,
            flexShrink: 0,
            display: "flex",
            flexDirection: "column",
            borderRight: "1px solid var(--border)",
            background: "var(--bg-glass)",
            overflow: "hidden",
          }}>
            {/* Search */}
            <div style={{ padding: "10px 12px", borderBottom: "1px solid var(--border)" }}>
              <div style={{ position: "relative" }}>
                <SvgIcon d="M11 3a8 8 0 1 0 0 16 8 8 0 0 0 0-16Z M21 21l-4.35-4.35" size={14}
                  style={{ position: "absolute", left: 8, top: "50%", transform: "translateY(-50%)", color: "var(--text-tertiary)" }} />
                <input className="input" style={{ fontSize: F.hint, padding: "6px 10px 6px 28", width: "100%" }}
                  placeholder={t("settings.search", "搜索设置…")}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)} />
                {searchQuery && (
                  <button type="button" style={{
                    position: "absolute", right: 4, top: "50%", transform: "translateY(-50%)",
                    background: "none", border: "none", cursor: "pointer", color: "var(--text-tertiary)", fontSize: 12,
                  }} onClick={() => setSearchQuery("")}>×</button>
                )}
              </div>
            </div>

            {/* Nav list */}
            <nav style={{ flex: 1, overflowY: "auto", padding: "6px 0" }}>
              {SECTIONS.map((section) => {
                const visibleFields = section.fields.filter((f) => !f.skipGui);
                const alwaysShow = ["hooks", "plugins", "sandbox", "permissions", "env", "status"].includes(section.id);
                if (visibleFields.length === 0 && !alwaysShow) return null;

                // Filter by search query
                if (searchQuery) {
                  const q = searchQuery.toLowerCase();
                  const sectionMatch = t(section.labelKey).toLowerCase().includes(q);
                  const fieldMatch = section.fields.some(f =>
                    f.key.toLowerCase().includes(q) ||
                    f.label.toLowerCase().includes(q) ||
                    (f.description ?? "").toLowerCase().includes(q),
                  );
                  if (!sectionMatch && !fieldMatch) return null;
                }

                const isActive = activeTab === section.id;
                return (
                  <button
                    key={section.id}
                    type="button"
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      width: "100%",
                      padding: "8px 14px",
                      fontSize: F.body,
                      fontWeight: isActive ? 600 : 400,
                      color: isActive ? "var(--accent)" : "var(--text-secondary)",
                      background: isActive ? "var(--accent-subtle, rgba(0,122,255,0.08))" : "transparent",
                      border: "none",
                      borderLeft: isActive ? "3px solid var(--accent)" : "3px solid transparent",
                      cursor: "pointer",
                      textAlign: "left",
                      transition: "all 100ms ease",
                    }}
                    onClick={() => setActiveTab(section.id)}
                    onMouseEnter={(e) => {
                      if (!isActive) e.currentTarget.style.background = "var(--bg-glass)";
                    }}
                    onMouseLeave={(e) => {
                      if (!isActive) e.currentTarget.style.background = "transparent";
                    }}
                  >
                    <SectionIcon name={section.id} size={15} />
                    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {t(section.labelKey)}
                    </span>
                  </button>
                );
              })}
            </nav>
          </aside>

          {/* ── Right content pane ── */}
          <main
            style={{
              flex: 1,
              minWidth: 0,
              padding: S.pad,
              overflowY: "auto",
            }}
          >
            {(() => {
              const section = SECTIONS.find((s) => s.id === activeTab);
              if (!section) return null;

              // Section heading inside content pane
              const heading = (
                <div style={{ marginBottom: S.gap + 4 }}>
                  <div style={{ fontSize: F.title, fontWeight: 600, color: "var(--text-primary)", letterSpacing: "-0.01em", display: "flex", alignItems: "center", gap: 8 }}>
                    <SectionIcon name={section.id} size={20} />
                    {t(section.labelKey)}
                  </div>
                </div>
              );

              // PermissionsSection renders its own Section card — need special wrapper
              if (section.id === "permissions") {
                return (
                  <div>
                    {heading}
                    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
                      {/* Inline permissions content — reimplement without Section wrapper */}
                      <PermissionsSectionInline perms={perms} updateField={updateField} t={t} />
                    </div>
                  </div>
                );
              }

              if (section.id === "sandbox") {
                return (
                  <div>
                    {heading}
                    <SandboxSectionInline
                      sandboxValue={config.sandbox as Record<string, any> | undefined}
                      updateField={updateField}
                    />
                  </div>
                );
              }

              if (section.id === "plugins") {
                return (
                  <div>
                    {heading}
                    <PluginsSectionInline
                      config={config}
                      updateField={updateField}
                    />
                  </div>
                );
              }

              if (section.id === "hooks") {
                return (
                  <div>
                    {heading}
                    <HooksSectionInline hooksValue={config.hooks as HooksConfig | undefined} updateField={updateField} t={t} />
                  </div>
                );
              }

              if (section.id === "status") {
                return (
                  <div>
                    {heading}
                    <StatusLineSection config={config} updateField={updateField} t={t} />
                  </div>
                );
              }

              return (
                <div>
                  {heading}
                  {renderSectionContent(section)}
                </div>
              );
            })()}
          </main>
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
    </div>
  );
}
