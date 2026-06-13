// ─── Settings editors & primitives ─────────────────────────
// Extracted verbatim from the former monolithic Settings.tsx (D1 split).
// Behavior is unchanged; only module boundaries moved.

import React, { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { statuslineApi } from "../../services/api";
import {
  ENV_VAR_DEFS,
  ENV_VAR_GROUP_ORDER,
  type SettingField,
  type EnvVarDef,
} from "../../services/claude-settings-schema";
import { SortableList } from "../SortableList";
import { IconClose, IconCheck, IconMenu, IconEdit } from "../icons";

// ─── Design tokens ───

export const F = {
  title: 20,        // section heading
  label: 15,        // field label
  body: 15,         // input / button / general text
  hint: 13,         // secondary / key-in-parens / description
  small: 12,        // arrow icon / error
} as const;

export const S = {
  sectionGap: 20,   // between section cards
  gap: 18,          // between fields within a section
  row: 12,          // kv row gap
  pad: 28,          // card padding
  inputPad: "10px 14px",
  btnPad: "8px 18px",
  btnIcon: 34,      // icon button size
  labelW: 200,      // unified label column width (FieldLabel / EnvVarRow / attribution)
} as const;

// ─── Inline SVG Icons ──────────────────────────────────────

/** 16×16 inline SVG icons — replace all emojis for consistent rendering */
export function SvgIcon({ d, size = 16, stroke = "currentColor", fill = "none", strokeWidth = 1.5, style }: {
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
export function SectionIcon({ name, size = 16, style }: { name: string; size?: number; style?: React.CSSProperties }) {
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

/** R8: render `text` with the first case-insensitive `query` occurrence highlighted. */
function Highlighted({ text, query }: { text: string; query?: string }) {
  if (!query) return <>{text}</>;
  const idx = text.toLowerCase().indexOf(query.toLowerCase());
  if (idx < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, idx)}
      <mark style={{ background: "var(--accent)", color: "#fff", borderRadius: 3, padding: "0 2px" }}>
        {text.slice(idx, idx + query.length)}
      </mark>
      {text.slice(idx + query.length)}
    </>
  );
}

/** Label cell for left-right layout */
function FieldLabel({ field, t, style, nonDefault, onReset, highlight }: {
  field: SettingField;
  t: ReturnType<typeof useTranslation>["t"];
  style?: React.CSSProperties;
  /** R10: current value differs from the recommended default → show reset affordance */
  nonDefault?: boolean;
  /** R10: reset this field to its default value */
  onReset?: () => void;
  /** R8: search query to highlight within the label */
  highlight?: string;
}) {
  const translated = t(`settings.f_${field.key}`, field.label);
  return (
    <label
      style={{
        flexShrink: 0,
        width: S.labelW,
        fontSize: F.label,
        fontWeight: 500,
        color: "var(--text-primary)",
        lineHeight: 1.5,
        paddingTop: 10,
        ...style,
      }}
    >
      <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
        <Highlighted text={translated} query={highlight} />
        {nonDefault && onReset && (
          <button
            type="button"
            onClick={(e) => { e.preventDefault(); onReset(); }}
            title={t("settings.resetToDefault")}
            aria-label={t("settings.resetToDefault")}
            style={{
              display: "inline-flex", alignItems: "center", gap: 3,
              padding: "1px 6px", borderRadius: 999, cursor: "pointer",
              border: "1px solid var(--accent)", background: "transparent",
              color: "var(--accent)", fontSize: 10, fontWeight: 600, lineHeight: 1.4,
            }}
          >
            <span style={{ width: 5, height: 5, borderRadius: "50%", background: "var(--accent)", flexShrink: 0 }} />
            {t("settings.reset")}
          </button>
        )}
      </span>
      <span style={{ display: "block", fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginTop: 3, fontFamily: '"SF Mono", "Fira Code", monospace' }}>
        <Highlighted text={field.key} query={highlight} />
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

/** Label width constant for symmetric layout — unified with FieldLabel via S.labelW */
const ENV_LABEL_W = S.labelW;

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
export function EnvEditor({ env, onChange, t }: {
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
  // R9: visual list editor (default) ↔ raw JSON fallback. Both write the same
  // permissions object, so field names (allow/ask/deny/defaultMode/...) are preserved.
  const [viewMode, setViewMode] = useState<"visual" | "json">("visual");

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

  /** Segmented control: visual list editor ↔ raw JSON fallback */
  const ViewToggle = (
    <div style={{ display: "flex", justifyContent: "flex-end" }}>
      <div style={{ display: "inline-flex", gap: 2, padding: 2, background: "var(--bg-glass)", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)" }}>
        {(["visual", "json"] as const).map((m) => {
          const active = viewMode === m;
          return (
            <button key={m} type="button"
              onClick={() => setViewMode(m)}
              style={{
                fontSize: F.small, fontWeight: active ? 600 : 400,
                padding: "3px 12px", borderRadius: "var(--radius-sm)",
                border: "none", cursor: "pointer",
                color: active ? "#fff" : "var(--text-secondary)",
                background: active ? "var(--accent)" : "transparent",
                transition: "all 120ms ease",
              }}
            >
              {m === "visual" ? t("settings.permissionsVisualView") : t("settings.permissionsJsonView")}
            </button>
          );
        })}
      </div>
    </div>
  );

  if (viewMode === "json") {
    return (
      <>
        {ViewToggle}
        <JsonEditor
          value={Object.keys(perms).length > 0 ? perms : undefined}
          onChange={(v) => updateField("permissions", v && Object.keys(v).length > 0 ? v : undefined)}
          placeholder='{ "allow": [], "ask": [], "deny": [], "defaultMode": "default" }'
          rows={10}
        />
      </>
    );
  }

  return (
    <>
      {ViewToggle}
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
            <option key={m.value} value={m.value}>{t(`settings.perm.mode_${m.value}`, m.desc)} — {t(`settings.perm.mode_${m.value}_desc`, m.hint)}</option>
          ))}
        </select>
      </FieldRow>
      <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.6, paddingLeft: 92 }}>
        {t("settings.perm.priorityLabel", "规则优先级")}: <span style={{ color: MODE_COLORS.deny, fontWeight: 600 }}>deny</span> →{" "}
        <span style={{ color: MODE_COLORS.ask, fontWeight: 600 }}>ask</span> →{" "}
        <span style={{ color: MODE_COLORS.allow, fontWeight: 600 }}>allow</span>{t("settings.perm.priorityNote", "。第一个匹配的规则生效。")}
      </div>

      {/* ── Safety Toggles ── */}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <FieldRow label={t("settings.perm.disableBypass", "禁用绕过模式")} icon={<SectionIcon name="bolt" size={14} />}>
          <div
            className={`toggle${perms.disableBypassPermissionsMode ? " active" : ""}`}
            onClick={() => updatePermKey("disableBypassPermissionsMode", perms.disableBypassPermissionsMode ? undefined : "disable")}
          />
          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>disableBypassPermissionsMode</span>
        </FieldRow>
        <FieldRow label={t("settings.perm.disableAuto", "禁用自动模式")} icon={<SectionIcon name="bolt" size={14} />}>
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
              {t(`settings.perm.toolLabel_${g.tool}`, g.label)}
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
        <span style={{ fontWeight: 600, color: "var(--accent)" }}>{t(`settings.perm.toolLabel_${toolGroup.tool}`, toolGroup.label)}</span>: {t(`settings.perm.syntax_${toolGroup.tool}`, toolGroup.syntax)}
      </div>

      {/* ── Rules for Active Group ── */}
      {(() => {
        const groupRules = grouped.get(activeToolGroup) ?? [];
        if (groupRules.length === 0) return (
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", padding: "12px 0", textAlign: "center" }}>
            {t("settings.perm.noRulesPrefix", "暂无")} {t(`settings.perm.toolLabel_${toolGroup.tool}`, toolGroup.label)} {t("settings.perm.noRulesSuffix", "规则。使用下方输入框添加。")}
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
            title={t("settings.perm.ruleTemplates", "规则模板")}
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
                      {t(`settings.perm.toolLabel_${g.tool}`, g.label)}
                      <span style={{ fontSize: 10, color: "var(--text-tertiary)", fontWeight: 400, fontFamily: '"SF Mono", "Fira Code", monospace' }}>
                        {t(`settings.perm.syntax_${g.tool}`, g.syntax)}
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
            <span>{t("settings.perm.totalRulesPrefix", "共")} {rules.length} {t("settings.perm.totalRulesSuffix", "条规则")}</span>
            <span style={{ color: MODE_COLORS.deny, display: "inline-flex", alignItems: "center", gap: 4 }}><IconClose size={12} /> deny: {rules.filter(r => r.mode === "deny").length}</span>
            <span style={{ color: MODE_COLORS.ask }}>? ask: {rules.filter(r => r.mode === "ask").length}</span>
            <span style={{ color: MODE_COLORS.allow, display: "inline-flex", alignItems: "center", gap: 4 }}><IconCheck size={12} /> allow: {rules.filter(r => r.mode === "allow").length}</span>
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

export function PermissionsSection({
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
export function PermissionsSectionInline({ perms, updateField, t }: {
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
            }}><IconClose size={12} /></button>
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
  const { t } = useTranslation();
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
            }}><IconClose size={12} /></button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6, alignItems: "stretch" }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <PathInput
            value={draft}
            onChange={setDraft}
            pathType="directory"
            placeholder={placeholder ?? t("settings.editor.dirOrPathPh", "选择目录或输入路径…")}
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
  const { t } = useTranslation();
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
            {t("settings.sandbox.enable", "启用沙箱")}
          </div>
          <Hint>{t("settings.sandbox.enableDesc", "Bash 命令及其子进程的文件系统和网络隔离 (Seatbelt / bubblewrap)")}</Hint>
        </div>
        {enabled && (
          <span style={{
            fontSize: F.small, fontWeight: 600, color: "#34c759",
            padding: "2px 8px", background: "rgba(52,199,89,0.12)", borderRadius: "var(--radius-sm)",
          }}>● {t("settings.sandbox.enabled", "已启用")}</span>
        )}
      </div>

      {!enabled && (
        <div style={{
          fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.6,
          padding: "10px 14px", background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
        }}>
          {t("settings.sandbox.disabledHint", "启用后，Claude 运行的每个 Bash 命令将被限制在指定的文件系统和网络边界内。macOS 使用 Seatbelt，Linux/WSL2 使用 bubblewrap。不支持原生 Windows。")}
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
              {t("settings.sandbox.fsIsolation", "文件系统隔离")}
            </SubHeading>
            <Hint>
              {t("settings.sandbox.fsIsolationDesc", "默认：可读整个文件系统，仅可写当前工作目录。路径前缀：/（绝对）、~/（主目录）、./（项目相对）")}
            </Hint>

            <FieldRow label={t("settings.sandbox.allowWrite", "允许写入")}>
              <PathList
                items={fs.allowWrite ?? []}
                onChange={(v) => setFsArray("allowWrite", v)}
                placeholder={t("settings.sandbox.allowWritePh", "如 ~/.kube, /tmp/build")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.denyWrite", "拒绝写入")}>
              <PathList
                items={fs.denyWrite ?? []}
                onChange={(v) => setFsArray("denyWrite", v)}
                placeholder={t("settings.sandbox.denyWritePh", "如 ~/.bashrc, /etc")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.allowRead", "允许读取")}>
              <PathList
                items={fs.allowRead ?? []}
                onChange={(v) => setFsArray("allowRead", v)}
                placeholder={t("settings.sandbox.allowReadPh", "如 .（项目目录）")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.denyRead", "拒绝读取")}>
              <PathList
                items={fs.denyRead ?? []}
                onChange={(v) => setFsArray("denyRead", v)}
                placeholder={t("settings.sandbox.denyReadPh", "如 ~/（阻止读主目录）, ~/.ssh")}
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
              {t("settings.sandbox.netIsolation", "网络隔离")}
            </SubHeading>
            <Hint>
              {t("settings.sandbox.netIsolationDesc", "默认：无预允许域名。命令首次需要新域名时提示批准。设置 allowedDomains 可预授权域名。")}
            </Hint>

            <FieldRow label={t("settings.sandbox.allowedDomains", "允许域名")}>
              <TagList
                items={net.allowedDomains ?? []}
                onChange={(v) => setNetArray("allowedDomains", v)}
                placeholder={t("settings.sandbox.allowedDomainsPh", "如 api.anthropic.com, *.github.com")}
              />
            </FieldRow>

            <FieldRow label={t("settings.sandbox.deniedDomains", "拒绝域名")}>
              <TagList
                items={net.deniedDomains ?? []}
                onChange={(v) => setNetArray("deniedDomains", v)}
                placeholder={t("settings.sandbox.deniedDomainsPh", "即使 allowedDomains 通配符允许，也会被阻止")}
              />
            </FieldRow>

            <div style={{ display: "flex", gap: 16, flexWrap: "wrap" }}>
              <FieldRow label={t("settings.sandbox.httpProxy", "HTTP 代理")}>
                <input
                  className="input"
                  type="number"
                  value={net.httpProxyPort ?? ""}
                  onChange={(e) => setNetPort("httpProxyPort", e.target.value)}
                  placeholder={t("settings.sandbox.port", "端口")}
                  style={{ width: 100, fontSize: F.hint, padding: "6px 10px" }}
                />
              </FieldRow>
              <FieldRow label={t("settings.sandbox.socksProxy", "SOCKS 代理")}>
                <input
                  className="input"
                  type="number"
                  value={net.socksProxyPort ?? ""}
                  onChange={(e) => setNetPort("socksProxyPort", e.target.value)}
                  placeholder={t("settings.sandbox.port", "端口")}
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
              {t("settings.sandbox.safety", "安全与策略")}
            </SubHeading>

            <FieldRow label={t("settings.sandbox.failIfUnavailable", "不可用时报错")}>
              <Toggle active={!!sb.failIfUnavailable} onChange={(v) => toggleSb("failIfUnavailable", v)} />
              <Hint>{t("settings.sandbox.failIfUnavailableDesc", "缺少依赖时阻止启动而非回退到非沙箱执行")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.noEscape", "禁止逃逸")}>
              <Toggle active={sb.allowUnsandboxedCommands === false} onChange={(v) => sync({ allowUnsandboxedCommands: !v })} />
              <Hint>{t("settings.sandbox.noEscapeDesc", "禁用 dangerouslyDisableSandbox 逃生舱，所有命令必须沙箱化")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.lockDomains", "锁定域名")}>
              <Toggle active={!!net.allowManagedDomainsOnly} onChange={(v) => sync({ network: { ...net, allowManagedDomainsOnly: v } })} />
              <Hint>{t("settings.sandbox.lockDomainsDesc", "仅尊重托管设置的 allowedDomains，忽略本地配置")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.lockReadPaths", "锁定读取路径")}>
              <Toggle active={!!sb.allowManagedReadPathsOnly} onChange={(v) => toggleSb("allowManagedReadPathsOnly", v)} />
              <Hint>{t("settings.sandbox.lockReadPathsDesc", "仅尊重托管设置的 allowRead，忽略本地配置")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.weakNet", "弱网络隔离")}>
              <Toggle active={!!sb.enableWeakerNetworkIsolation} onChange={(v) => toggleSb("enableWeakerNetworkIsolation", v)} />
              <Hint>{t("settings.sandbox.weakNetDesc", "MITM 代理 + 自定义 CA 场景下启用")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.weakNested", "弱嵌套沙箱")}>
              <Toggle active={!!sb.enableWeakerNestedSandbox} onChange={(v) => toggleSb("enableWeakerNestedSandbox", v)} />
              <Hint>{t("settings.sandbox.weakNestedDesc", "无特权容器内运行时启用（绑定挂载 /proc 而非新建）")}</Hint>
            </FieldRow>

            <FieldRow label={t("settings.sandbox.unixSockets", "Unix 套接字")}>
              <Toggle active={!!sb.allowUnixSockets} onChange={(v) => toggleSb("allowUnixSockets", v)} />
              <Hint>{t("settings.sandbox.unixSocketsDesc", "允许 Unix 域套接字访问（注意：Docker socket 等可能绕过沙箱）")}</Hint>
            </FieldRow>
          </div>

          {/* ── Excluded Commands ── */}
          <div style={{
            padding: "14px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
            display: "flex", flexDirection: "column", gap: 10,
          }}>
            <SubHeading>
              <SvgIcon d="M18 6L6 18M6 6l12 12" size={15} />
              {t("settings.sandbox.excludedCommands", "排除命令")}
            </SubHeading>
            <Hint>
              {t("settings.sandbox.excludedCommandsDesc", "列出的命令在沙箱外运行（如 docker, gh, terraform 等与沙箱不兼容的工具）")}
            </Hint>
            <TagList
              items={sb.excludedCommands ?? []}
              onChange={setExcludedCommands}
              placeholder={t("settings.sandbox.excludedCommandsPh", "如 docker, gh, terraform, watchman")}
            />
          </div>
        </>
      )}
    </div>
  );
}

/** Sandbox with Section wrapper — for card-based layout */
export function SandboxSection({
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
export function SandboxSectionInline({ sandboxValue, updateField }: {
  sandboxValue: Record<string, any> | undefined;
  updateField: (field: string, value: any) => void;
}) {
  return <SandboxEditor sandboxValue={sandboxValue} updateField={updateField} />;
}

// ─── StatusLine Section (structured editor) ────────────────────

/** A single display segment in the statusline */
type RowAlign = "left" | "center" | "right";

interface StatusLineSegment {
  id: string;
  type: SegmentType;
  enabled: boolean;
  newline: boolean; // insert line break before this segment (row leader when true)
  options: Record<string, any>;
  color?: string;      // fixed hex foreground color, e.g. "#4A9EFF"
  autoColor?: boolean; // value-class segments: derive color from value via thresholds
  align?: RowAlign;    // row alignment — only meaningful on the row-leading segment
}

/** Segment types whose value can drive automatic semantic coloring. */
const VALUE_COLORABLE: Set<SegmentType> = new Set([
  "context-pct", "context-bar", "cost", "rate-limits",
  // Atomic value-class segments
  "cost-usd", "context-remaining", "rate-limit-5h", "rate-limit-7d",
  "session-duration", "api-duration",
]);

/** Parse "#RRGGBB" / "#RGB" → [r,g,b] (0–255) or null when invalid. */
function hexToRgb(hex?: string): [number, number, number] | null {
  if (!hex) return null;
  let h = hex.trim().replace(/^#/, "");
  if (h.length === 3) h = h.split("").map(c => c + c).join("");
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return null;
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
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
  // ── Atomic segments (one per raw statusline input field) ──
  // Cost / execution
  | "cost-usd"               // cost.total_cost_usd
  | "session-duration"       // cost.total_duration_ms
  | "api-duration"           // cost.total_api_duration_ms
  | "lines-changed"          // cost.total_lines_added / removed
  // Context window
  | "context-tokens"         // context_window.total_input/output_tokens
  | "context-max"            // context_window.context_window_size
  | "context-remaining"      // context_window.remaining_percentage
  | "context-cache"          // context_window.current_usage.cache_*_tokens
  // Rate limits (per window)
  | "rate-limit-5h"          // rate_limits.five_hour
  | "rate-limit-7d"          // rate_limits.seven_day
  // Git
  | "git-branch"             // git -C <cwd> branch --show-current（脚本内跑 git）
  | "git-host"               // workspace.repo.host
  | "git-owner"              // workspace.repo.owner
  | "git-repo"               // workspace.repo.name
  | "git-repo-full"          // owner/name
  | "git-worktree"           // workspace.git_worktree
  // Directory / session
  | "cwd"                    // workspace.current_dir
  | "project-dir"            // workspace.project_dir
  | "added-dirs"             // workspace.added_dirs
  | "session-id"             // session_id
  | "session-name"           // session_name
  | "transcript-path"        // transcript_path
  // Worktree
  | "worktree-name"          // worktree.name
  | "worktree-branch"        // worktree.branch
  | "worktree-original-branch" // worktree.original_branch
  // PR
  | "pr-number"              // pr.number
  | "pr-url"                 // pr.url
  | "pr-state"               // pr.review_state
  // Other single fields
  | "version"                // version
  | "output-style"           // output_style.name
  | "thinking"               // thinking.enabled
  | "token-warn"             // exceeds_200k_tokens
  | "agent"                  // agent.name
  // aidog group segments
  | "group-balance"  // aidog group: 预估余额
  | "group-spent"    // aidog group: 累计预估花费
  | "group-coding"   // aidog group: coding plan 利用率
  | "group-requests" // aidog group: 请求数 · 成功率
  | "group-cache"    // aidog group: 缓存命中率
  | "group-tokens"   // aidog group: 已使用总 tokens
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

/** Segment types that consume the shared aidog group-info endpoint. */
const GROUP_SEG_TYPES = new Set<SegmentType>([
  "group-balance", "group-spent", "group-coding",
  "group-requests", "group-cache", "group-tokens",
]);

/**
 * Build a bash snippet for an aidog group-info segment.
 *
 * Contract (relies on the script prelude `__aidog_fetch_info` having run once):
 *  - Gracefully degrades to empty output when `$ANTHROPIC_BASE_URL` is unset
 *    (main settings / non-group settings) or the cached payload is missing /
 *    not applicable (multi-platform or no-platform group / unreachable endpoint).
 *  - Reads the cached JSON written by the prelude (`$__AIDOG_INFO_FILE`) so a
 *    row with several group segments only curls the endpoint once.
 *  - `jqExpr` is passed verbatim to `jq` (may begin with flags like `-r`);
 *    it extracts the field(s) for this segment from the cached JSON.
 *  - `prefix` is a literal label prepended when output is non-empty.
 */
function groupSegBash(jqExpr: string, prefix: string): string {
  const pfx = bashEscapeDq(prefix);
  return `[ -z "\${ANTHROPIC_BASE_URL:-}" ] && exit 0
__gi="\${__AIDOG_INFO_FILE:-}"
[ -z "$__gi" ] || [ ! -s "$__gi" ] && exit 0
echo "$(cat "$__gi")" | jq -e '.applicable == true' >/dev/null 2>&1 || exit 0
__val=$(cat "$__gi" | jq ${jqExpr} 2>/dev/null)
[ -z "$__val" ] && exit 0
echo -n "${pfx}$__val"`;
}

/** ANSI truecolor reset/red/amber/green triples shared by dynamic group segments. */
const ANSI_RED = "255;69;58";    // #FF453A
const ANSI_AMBER = "255;214;10"; // #FFD60A
const ANSI_GREEN = "52;199;89";  // #34C759

/**
 * Prelude that all group segments share: degrade-to-empty guards + applicable
 * check, leaving the cached payload readable via `$__gi`.
 */
const GROUP_GUARD = `[ -z "\${ANTHROPIC_BASE_URL:-}" ] && exit 0
__gi="\${__AIDOG_INFO_FILE:-}"
[ -z "$__gi" ] || [ ! -s "$__gi" ] && exit 0
cat "$__gi" | jq -e '.applicable == true' >/dev/null 2>&1 || exit 0`;

/**
 * coding-plan 段（第 3 行动态色）：仅当端点含 coding_plan tiers 时展示，按各档最严重
 * level 上色——red→红 / yellow→黄 / green→绿 / neutral→无色（默认绿）。level 由后端
 * 按「使用速率（剩余可用时间%）」算（usage_color 唯一阈值源），statusline 只消费不重算。
 * 红色（red）时若有 reset_at 额外拼接人类可读重置时间。无 tiers → 降级空。
 * 直接输出 ANSI truecolor（不经 fixedColorBash），故段 color 应留空。
 */
function groupCodingDynBash(): string {
  return `${GROUP_GUARD}
__n=$(cat "$__gi" | jq -r '(.coding_plan // []) | length')
[ "$__n" = "0" ] || [ -z "$__n" ] && exit 0
__txt=$(cat "$__gi" | jq -r '(.coding_plan // []) | map((if .name == "five_hour" then "5h" elif (.name == "seven_day" or .name == "weekly_limit") then "7d" else .name end) + " " + ((.utilization // 0) | round | tostring) + "%") | join("·")')
[ -z "$__txt" ] && exit 0
__lvl=$(cat "$__gi" | jq -r 'if any(.coding_plan[]?; .level == "red") then "red" elif any(.coding_plan[]?; .level == "yellow") then "yellow" elif any(.coding_plan[]?; .level == "green") then "green" else "neutral" end')
if [ "$__lvl" = "red" ]; then
  __c="${ANSI_RED}"
  __rs=$(cat "$__gi" | jq -r '[.coding_plan[]? | select(.level == "red") | .reset_at // empty] | min // empty')
  if [ -n "$__rs" ] && [ "$__rs" != "null" ]; then
    __now=$(date +%s); __d=$((__rs - __now))
    if [ "$__d" -gt 0 ]; then
      __h=$((__d / 3600)); __m=$(((__d % 3600) / 60))
      __txt="$__txt (reset \${__h}h\${__m}m)"
    fi
  fi
elif [ "$__lvl" = "yellow" ]; then
  __c="${ANSI_AMBER}"
else
  __c="${ANSI_GREEN}"
fi
printf '\\033[38;2;%sm%s\\033[0m' "$__c" "$__txt"`;
}

/**
 * 余额段（第 3 行动态色）：余额平台展示，按后端 balance_level 上色——red→红 / yellow→黄
 * / green / neutral→绿（默认）。level 由后端按「剩余可用天数（动态窗口日速率）」算
 * （usage_color 唯一阈值源），statusline 只消费不重算阈值。直接输出 ANSI truecolor。
 */
function groupBalanceDynBash(prefix: string): string {
  const pfx = bashEscapeDq(prefix);
  // 余额段始终展示（不再与 coding_plan 互斥）——窗口预算由 coding 段展示，
  // 余额由 balance 段展示，两者可共存。
  return `${GROUP_GUARD}
__bal=$(cat "$__gi" | jq -r '.balance // 0 | (. * 100 | round) / 100')
__bal=$(printf '%g' "$__bal")
[ "$__bal" = "0" ] || [ "$__bal" = "0.0" ] && exit 0
__txt="${pfx}$__bal"
__lvl=$(cat "$__gi" | jq -r '.balance_level // "neutral"')
if [ "$__lvl" = "red" ]; then __c="${ANSI_RED}"
elif [ "$__lvl" = "yellow" ]; then __c="${ANSI_AMBER}"
else __c="${ANSI_GREEN}"; fi
printf '\\033[38;2;%sm%s\\033[0m' "$__c" "$__txt"`;
}

/**
 * Build a bash snippet for an atomic statusline segment that extracts a single
 * value from the stdin JSON (`$input`) and degrades to empty output when the
 * field is absent / null.
 *
 *  - `jqExpr` is passed verbatim to `jq` (may begin with flags like `-r`); it
 *    must use `// empty` (or equivalent) so missing fields yield no output.
 *  - On empty extraction the segment prints nothing — the generator's
 *    non-empty-only separator logic then leaves no orphaned separator.
 *  - `prefix` is a literal label prepended only when the value is non-empty.
 */
function atomSegBash(jqExpr: string, prefix = ""): string {
  const pfx = bashEscapeDq(prefix);
  return `__val=$(echo "$input" | jq ${jqExpr} 2>/dev/null)
[ -z "$__val" ] && exit 0
echo -n "${pfx}$__val"`;
}

/**
 * Wrap a segment body with a literal prefix / suffix that only appears when the
 * body produced non-empty output. Enables mixed in-row separators (`·` vs `|`,
 * `[cost]` hugging, conditional `·worktree`) without a global separator: the
 * affixes are part of the same empty-degrading, color-wrapped unit, so a segment
 * that exits empty leaves no orphaned separator char.
 *
 * `affixPre` / `affixSuf` are reserved option keys consumed here (not by the
 * segment's own `toBash`), kept distinct from user-facing `prefix` labels.
 */
function wrapAffix(body: string, affixPre: string, affixSuf: string): string {
  if (!affixPre && !affixSuf) return body;
  const pre = bashEscapeDq(affixPre);
  const suf = bashEscapeDq(affixSuf);
  // Real newlines (not `;`-collapsed) so multi-line bodies with `case`/`if`
  // control structures stay valid inside the `{ …; }` group. Brace `${__base}`
  // so an immediately-following multibyte affix (e.g. `·`) can't be swallowed
  // into the variable name under a UTF-8 locale (bash name-parsing quirk).
  return `__base="$({
${body}
})"
[ -z "$__base" ] && exit 0
printf '%s' "${pre}\${__base}${suf}"`;
}

/**
 * Prelude helper: fetch the group-info endpoint at most once per render and
 * cache the payload in a temp file shared by all group segments on the row.
 * Short `--max-time` + silent failure so an unreachable endpoint never stalls
 * the statusline. Emitted only when at least one group segment is active.
 */
function aidogFetchPrelude(): string {
  return `# ── aidog group-info (fetched once, shared by group-* segments) ──
__AIDOG_INFO_FILE=""
if [ -n "\${ANTHROPIC_BASE_URL:-}" ]; then
  __AIDOG_INFO_FILE="\${TMPDIR:-/tmp}/aidog_info_$$_\${RANDOM:-$$}"
  # Extract scheme://host:port (strip all path components) so any base_url
  # suffix (/v1, /proxy, /api/paas/v4, …) is removed.
  __no_scheme="\${ANTHROPIC_BASE_URL#*://}"
  __proxy_root="\${ANTHROPIC_BASE_URL%%://*}://\${__no_scheme%%/*}"
  curl -fsS --max-time 1 \\
    -X POST \\
    -H "Authorization: Bearer \${ANTHROPIC_AUTH_TOKEN:-}" \\
    "\${__proxy_root}/api/group-info" > "$__AIDOG_INFO_FILE" 2>/dev/null || : > "$__AIDOG_INFO_FILE"
fi
`;
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
    desc: "视觉分隔符（可插入到任意段之间）",
    defaultOptions: { char: "·" },
    toBash: (o) => `echo -n "${bashEscapeDq(typeof o.char === "string" ? o.char : "·")}"`,
    toPreview: (o) => (typeof o.char === "string" ? o.char : "·"),
    fields: [
      { key: "char", label: "分隔符字符", type: "string", placeholder: "·" },
    ],
  },
  {
    type: "group-balance",
    name: "分组余额",
    icon: "bolt",
    desc: "当前分组单平台预估剩余余额（动态色：<1天红 / <3天黄 / 否则绿）",
    defaultOptions: { prefix: "余额 ", dynamicColor: false },
    toBash: (o) => o.dynamicColor
      ? groupBalanceDynBash(o.prefix ?? "余额 ")
      : groupSegBash(`'.balance // 0 | (. * 100 | round) / 100' | awk '{printf "%.2f", $0}'`, o.prefix ?? "余额 "),
    toPreview: (o) => `${o.prefix ?? "余额 "}48.20`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "余额 " },
      { key: "dynamicColor", label: "动态色 (按可用天数)", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "group-spent",
    name: "分组花费",
    icon: "bolt",
    desc: "当前分组累计预估花费（仅单平台分组）",
    defaultOptions: { prefix: "$" },
    toBash: (o) => groupSegBash(`'.spent // 0 | (. * 100 | round) / 100' | awk '{printf "%.2f", $0}'`, o.prefix ?? "$"),
    toPreview: (o) => `${o.prefix ?? "$"}1.23`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "$" },
    ],
  },
  {
    type: "group-coding",
    name: "Coding Plan",
    icon: "permissions",
    desc: "Coding Plan 各档利用率（动态色：fast红 / normal黄 / busy绿，红时显重置）",
    defaultOptions: { dynamicColor: false },
    toBash: (o) => o.dynamicColor
      ? groupCodingDynBash()
      : groupSegBash(`-r '(.coding_plan // []) | map((if .name == "five_hour" then "5h" elif (.name == "seven_day" or .name == "weekly_limit") then "7d" else .name end) + " " + ((.utilization // 0) | round | tostring) + "%") | join("·")'`, ""),
    toPreview: () => "5h 23%·7d 41%",
    fields: [
      { key: "dynamicColor", label: "动态色 (按 pace)", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "group-requests",
    name: "请求·成功率",
    icon: "status",
    desc: "当前分组请求数 · 成功率（仅单平台分组）",
    defaultOptions: {},
    toBash: () => groupSegBash(`-r '"\\(.requests // 0)·\\((.success_rate // 0) | round)%"'`, ""),
    toPreview: () => "128·99%",
    fields: [],
  },
  {
    type: "group-cache",
    name: "缓存率",
    icon: "status",
    desc: "当前分组缓存命中率（仅单平台分组）",
    defaultOptions: { prefix: "缓存 " },
    toBash: (o) => groupSegBash(`-r '"\\((.cache_rate // 0) | round)%"'`, o.prefix ?? "缓存 "),
    toPreview: (o) => `${o.prefix ?? "缓存 "}37%`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "缓存 " },
    ],
  },
  {
    type: "group-tokens",
    name: "总 Tokens",
    icon: "core",
    desc: "当前分组已使用总 tokens（仅单平台分组）",
    defaultOptions: { prefix: "" },
    toBash: (o) => groupSegBash(`-r '(.total_tokens // 0) | if . >= 1000000 then ((. / 100000 | round) / 10 | tostring) + "M" elif . >= 1000 then ((. / 100 | round) / 10 | tostring) + "K" else tostring end'`, o.prefix ?? ""),
    toPreview: (o) => `${o.prefix ?? ""}1.2M`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "" },
    ],
  },
  // ── Atomic segments: one per raw statusline input field ──
  // Cost / execution
  {
    type: "cost-usd",
    name: "成本 ($)",
    icon: "bolt",
    desc: "cost.total_cost_usd — 累计预估成本",
    defaultOptions: { prefix: "$" },
    toBash: (o) => atomSegBash(
      `-r '(.cost.total_cost_usd // empty) | (. * 100 | round) / 100 | tostring'`,
      o.prefix ?? "$",
    ),
    toPreview: (o) => `${o.prefix ?? "$"}0.12`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "$" },
    ],
  },
  {
    type: "session-duration",
    name: "会话耗时",
    icon: "status",
    desc: "cost.total_duration_ms — 会话总耗时",
    defaultOptions: { format: "human" },
    toBash: (o) => o.format === "ms"
      ? atomSegBash(`-r '(.cost.total_duration_ms // empty) | tostring + "ms"'`)
      : atomSegBash(`-r '(.cost.total_duration_ms // empty) | (. / 1000) as $s | if $s >= 60 then ((($s / 60) | floor) | tostring) + "m" + (($s % 60 | round) | tostring) + "s" else ($s | round | tostring) + "s" end'`),
    toPreview: (o) => o.format === "ms" ? "285000ms" : "4m45s",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["human", "ms"] },
    ],
  },
  {
    type: "api-duration",
    name: "API 耗时",
    icon: "status",
    desc: "cost.total_api_duration_ms — API 等待时间",
    defaultOptions: { format: "human" },
    toBash: (o) => o.format === "ms"
      ? atomSegBash(`-r '(.cost.total_api_duration_ms // empty) | tostring + "ms"'`)
      : atomSegBash(`-r '(.cost.total_api_duration_ms // empty) | (. / 1000) as $s | if $s >= 60 then ((($s / 60) | floor) | tostring) + "m" + (($s % 60 | round) | tostring) + "s" else ($s | round | tostring) + "s" end'`),
    toPreview: (o) => o.format === "ms" ? "15300ms" : "15s",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["human", "ms"] },
    ],
  },
  {
    type: "lines-changed",
    name: "代码变更",
    icon: "core",
    desc: "cost.total_lines_added / removed — 新增/删除行",
    defaultOptions: {},
    toBash: () => atomSegBash(
      `-r '"+" + ((.cost.total_lines_added // 0) | tostring) + " -" + ((.cost.total_lines_removed // 0) | tostring)'`,
    ),
    toPreview: () => "+412 -87",
    fields: [],
  },
  // Context window
  {
    type: "context-tokens",
    name: "上下文 Tokens",
    icon: "core",
    desc: "输入/输出 token，或 session 合计（total_input + total_output）",
    defaultOptions: { abbrev: true, mode: "split" },
    toBash: (o) => {
      const fmt = o.abbrev
        ? `if . >= 1000000 then ((. / 100000 | round) / 10 | tostring) + "M" elif . >= 1000 then ((. / 100 | round) / 10 | tostring) + "K" else tostring end`
        : `tostring`;
      // sum 模式：当前 session tokens = total_input + total_output（PRD 第 1 行紫色段）。
      if (o.mode === "sum") {
        return atomSegBash(
          `-r 'if .context_window == null then empty else (((.context_window.total_input_tokens // 0) + (.context_window.total_output_tokens // 0)) | ${fmt}) end'`,
        );
      }
      return atomSegBash(
        `-r '((.context_window.total_input_tokens // empty) | ${fmt}) as $i | ((.context_window.total_output_tokens // 0) | ${fmt}) as $o | $i + "/" + $o'`,
      );
    },
    toPreview: (o) => o.mode === "sum"
      ? (o.abbrev ? "101.9K" : "101900")
      : (o.abbrev ? "89.5K/12.4K" : "89500/12400"),
    fields: [
      { key: "mode", label: "模式", type: "select", options: ["split", "sum"] },
      { key: "abbrev", label: "缩写 (K/M)", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "context-max",
    name: "上下文容量",
    icon: "status",
    desc: "context_window.context_window_size — 最大窗口",
    defaultOptions: { abbrev: true },
    toBash: (o) => o.abbrev
      ? atomSegBash(`-r '(.context_window.context_window_size // empty) | if . >= 1000000 then ((. / 100000 | round) / 10 | tostring) + "M" elif . >= 1000 then ((. / 1000 | round) | tostring) + "K" else tostring end'`)
      : atomSegBash(`-r '(.context_window.context_window_size // empty) | tostring'`),
    toPreview: (o) => o.abbrev ? "200K" : "200000",
    fields: [
      { key: "abbrev", label: "缩写 (K/M)", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "context-remaining",
    name: "上下文剩余",
    icon: "status",
    desc: "context_window.remaining_percentage — 剩余百分比",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '(.context_window.remaining_percentage // empty) | round | tostring + "%"'`),
    toPreview: () => "49%",
    fields: [],
  },
  {
    type: "context-cache",
    name: "缓存率",
    icon: "core",
    desc: "缓存写入/读取 token，或缓存命中率 %（≤4 位小数）",
    defaultOptions: { abbrev: true, mode: "tokens", prefix: "缓存 " },
    toBash: (o) => {
      // 命中率模式：cache_read / (input + cache_read) × 100，printf 控小数位 ≤4；current_usage==null 降级 0%。
      if (o.mode === "hitrate") {
        const pfx = bashEscapeDq(o.prefix ?? "缓存 ");
        return `__cu=$(echo "$input" | jq -r '.context_window.current_usage')
if [ -z "$__cu" ] || [ "$__cu" = "null" ]; then
  __rate=0
else
  __rate=$(echo "$input" | jq -r '.context_window.current_usage | (.cache_read_input_tokens // 0) as $r | (.input_tokens // 0) as $i | if ($i + $r) > 0 then ($r / ($i + $r) * 100) else 0 end')
fi
echo -n "${pfx}$(printf '%.4f' "\${__rate:-0}" | sed 's/0*$//; s/\\\\\\.$//')%"`;
      }
      const fmt = o.abbrev
        ? `if . >= 1000000 then ((. / 100000 | round) / 10 | tostring) + "M" elif . >= 1000 then ((. / 100 | round) / 10 | tostring) + "K" else tostring end`
        : `tostring`;
      return atomSegBash(
        `-r 'if .context_window.current_usage == null then empty else ((.context_window.current_usage.cache_creation_input_tokens // 0) | ${fmt}) as $w | ((.context_window.current_usage.cache_read_input_tokens // 0) | ${fmt}) as $r | "w" + $w + " r" + $r end'`,
      );
    },
    toPreview: (o) => o.mode === "hitrate"
      ? `${o.prefix ?? "缓存 "}13.3578%`
      : (o.abbrev ? "w20K r12.1K" : "w20000 r12100"),
    fields: [
      { key: "mode", label: "模式", type: "select", options: ["tokens", "hitrate"] },
      { key: "abbrev", label: "缩写 (K/M)", type: "select", options: ["true", "false"] },
      { key: "prefix", label: "命中率前缀", type: "string", placeholder: "缓存 " },
    ],
  },
  // Rate limits (per window)
  {
    type: "rate-limit-5h",
    name: "限制 5h",
    icon: "permissions",
    desc: "rate_limits.five_hour — 5 小时窗口使用率",
    defaultOptions: { showReset: false },
    toBash: (o) => o.showReset
      ? atomSegBash(`-r 'if .rate_limits.five_hour.used_percentage == null then empty else "5h:" + ((.rate_limits.five_hour.used_percentage) | round | tostring) + "%" + (if .rate_limits.five_hour.resets_at then " (" + (((.rate_limits.five_hour.resets_at - now) / 60 | floor) | tostring) + "m)" else "" end) end'`)
      : atomSegBash(`-r '(.rate_limits.five_hour.used_percentage // empty) | round | "5h:" + tostring + "%"'`),
    toPreview: (o) => o.showReset ? "5h:34% (128m)" : "5h:34%",
    fields: [
      { key: "showReset", label: "显示剩余重置时间", type: "select", options: ["false", "true"] },
    ],
  },
  {
    type: "rate-limit-7d",
    name: "限制 7d",
    icon: "permissions",
    desc: "rate_limits.seven_day — 7 天窗口使用率",
    defaultOptions: { showReset: false },
    toBash: (o) => o.showReset
      ? atomSegBash(`-r 'if .rate_limits.seven_day.used_percentage == null then empty else "7d:" + ((.rate_limits.seven_day.used_percentage) | round | tostring) + "%" + (if .rate_limits.seven_day.resets_at then " (" + (((.rate_limits.seven_day.resets_at - now) / 3600 | floor) | tostring) + "h)" else "" end) end'`)
      : atomSegBash(`-r '(.rate_limits.seven_day.used_percentage // empty) | round | "7d:" + tostring + "%"'`),
    toPreview: (o) => o.showReset ? "7d:62% (40h)" : "7d:62%",
    fields: [
      { key: "showReset", label: "显示剩余重置时间", type: "select", options: ["false", "true"] },
    ],
  },
  // Git
  {
    type: "git-branch",
    name: "Git 分支",
    icon: "folder",
    desc: "脚本内 git branch --show-current（非 git / 无分支降级空）",
    defaultOptions: {},
    // cwd 取自 workspace.current_dir，回退 .cwd，再回退当前目录；非 git 仓库 / 游离 HEAD → 空输出降级。
    toBash: () => `__cwd=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // "."')
__b=$(git -C "$__cwd" branch --show-current 2>/dev/null)
[ -z "$__b" ] && exit 0
echo -n "$__b"`,
    toPreview: () => "main",
    fields: [],
  },
  {
    type: "git-host",
    name: "Git 主机",
    icon: "folder",
    desc: "workspace.repo.host — Git 仓库主机",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.workspace.repo.host // empty'`),
    toPreview: () => "github.com",
    fields: [],
  },
  {
    type: "git-owner",
    name: "Git 所有者",
    icon: "folder",
    desc: "workspace.repo.owner — 仓库所有者",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.workspace.repo.owner // empty'`),
    toPreview: () => "anthropics",
    fields: [],
  },
  {
    type: "git-repo",
    name: "Git 仓库",
    icon: "folder",
    desc: "workspace.repo.name — 仓库名",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.workspace.repo.name // empty'`),
    toPreview: () => "claude-code",
    fields: [],
  },
  {
    type: "git-repo-full",
    name: "Git 全名",
    icon: "folder",
    desc: "owner/name — 仓库完整标识",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r 'if .workspace.repo.name then ((.workspace.repo.owner // "") + "/" + .workspace.repo.name) else empty end'`),
    toPreview: () => "anthropics/claude-code",
    fields: [],
  },
  {
    type: "git-worktree",
    name: "Git Worktree",
    icon: "folder",
    desc: "workspace.git_worktree — Git worktree 名称",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.workspace.git_worktree // empty'`),
    toPreview: () => "feature-xyz",
    fields: [],
  },
  // Directory / session
  {
    type: "cwd",
    name: "工作目录",
    icon: "folder",
    desc: "workspace.current_dir — 当前工作目录",
    defaultOptions: { format: "basename" },
    toBash: (o) => o.format === "full"
      ? atomSegBash(`-r '.workspace.current_dir // empty'`)
      : atomSegBash(`-r '(.workspace.current_dir // empty) | split("/") | last'`),
    toPreview: (o) => o.format === "full" ? "/Users/luoxin/persons/aidog" : "aidog",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["basename", "full"] },
    ],
  },
  {
    type: "project-dir",
    name: "项目目录",
    icon: "folder",
    desc: "workspace.project_dir — 项目启动目录",
    defaultOptions: { format: "basename" },
    toBash: (o) => o.format === "full"
      ? atomSegBash(`-r '.workspace.project_dir // empty'`)
      : atomSegBash(`-r '(.workspace.project_dir // empty) | split("/") | last'`),
    toPreview: (o) => o.format === "full" ? "/Users/luoxin/persons/aidog" : "aidog",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["basename", "full"] },
    ],
  },
  {
    type: "added-dirs",
    name: "附加目录",
    icon: "folder",
    desc: "workspace.added_dirs — /add-dir 添加的目录",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '(.workspace.added_dirs // []) | if length == 0 then empty else map(split("/") | last) | join(",") end'`),
    toPreview: () => "shared,web",
    fields: [],
  },
  {
    type: "session-id",
    name: "会话 ID",
    icon: "core",
    desc: "session_id — 会话标识符",
    defaultOptions: { truncate: true },
    toBash: (o) => o.truncate
      ? atomSegBash(`-r '(.session_id // empty) | .[0:8]'`)
      : atomSegBash(`-r '.session_id // empty'`),
    toPreview: (o) => o.truncate ? "abc123xy" : "abc123xyz789",
    fields: [
      { key: "truncate", label: "截断 (前8位)", type: "select", options: ["true", "false"] },
    ],
  },
  {
    type: "session-name",
    name: "会话名称",
    icon: "core",
    desc: "session_name — 自定义会话名（未设置时隐藏）",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.session_name // empty'`),
    toPreview: () => "statusline-atoms",
    fields: [],
  },
  {
    type: "transcript-path",
    name: "记录路径",
    icon: "folder",
    desc: "transcript_path — 会话记录文件",
    defaultOptions: { format: "basename" },
    toBash: (o) => o.format === "full"
      ? atomSegBash(`-r '.transcript_path // empty'`)
      : atomSegBash(`-r '(.transcript_path // empty) | split("/") | last'`),
    toPreview: (o) => o.format === "full" ? "/Users/luoxin/.claude/session.jsonl" : "session.jsonl",
    fields: [
      { key: "format", label: "格式", type: "select", options: ["basename", "full"] },
    ],
  },
  // Worktree
  {
    type: "worktree-name",
    name: "Worktree 名",
    icon: "folder",
    desc: "worktree.name — Worktree 标识",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.worktree.name // empty'`),
    toPreview: () => "feature-xyz",
    fields: [],
  },
  {
    type: "worktree-branch",
    name: "Worktree 分支",
    icon: "folder",
    desc: "worktree.branch — 当前工作分支",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.worktree.branch // empty'`),
    toPreview: () => "feat/atoms",
    fields: [],
  },
  {
    type: "worktree-original-branch",
    name: "Worktree 源分支",
    icon: "folder",
    desc: "worktree.original_branch — 回源分支",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.worktree.original_branch // empty'`),
    toPreview: () => "main",
    fields: [],
  },
  // PR
  {
    type: "pr-number",
    name: "PR 编号",
    icon: "status",
    desc: "pr.number — 开放 PR 编号",
    defaultOptions: { prefix: "#" },
    toBash: (o) => atomSegBash(`-r '(.pr.number // empty) | tostring'`, o.prefix ?? "#"),
    toPreview: (o) => `${o.prefix ?? "#"}123`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "#" },
    ],
  },
  {
    type: "pr-url",
    name: "PR 链接",
    icon: "status",
    desc: "pr.url — PR 链接",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.pr.url // empty'`),
    toPreview: () => "https://github.com/o/r/pull/123",
    fields: [],
  },
  {
    type: "pr-state",
    name: "PR 状态",
    icon: "status",
    desc: "pr.review_state — PR 审查状态",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.pr.review_state // empty'`),
    toPreview: () => "approved",
    fields: [],
  },
  // Other single fields
  {
    type: "version",
    name: "CC 版本",
    icon: "core",
    desc: "version — Claude Code 版本",
    defaultOptions: { prefix: "v" },
    toBash: (o) => atomSegBash(`-r '.version // empty'`, o.prefix ?? "v"),
    toPreview: (o) => `${o.prefix ?? "v"}2.1.90`,
    fields: [
      { key: "prefix", label: "前缀", type: "string", placeholder: "v" },
    ],
  },
  {
    type: "output-style",
    name: "输出风格",
    icon: "ui",
    desc: "output_style.name — 当前输出风格",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.output_style.name // empty'`),
    toPreview: () => "default",
    fields: [],
  },
  {
    type: "thinking",
    name: "思考模式",
    icon: "behavior",
    desc: "thinking.enabled — 扩展思考开启时显示",
    defaultOptions: { label: "thinking" },
    toBash: (o) => {
      const label = bashEscapeDq(o.label ?? "thinking");
      return atomSegBash(`-r 'if .thinking.enabled == true then "${label}" else empty end'`);
    },
    toPreview: (o) => o.label ?? "thinking",
    fields: [
      { key: "label", label: "文案", type: "string", placeholder: "thinking" },
    ],
  },
  {
    type: "token-warn",
    name: "Token 警示",
    icon: "permissions",
    desc: "exceeds_200k_tokens — 超 200k 时警示",
    defaultOptions: { label: "⚠200k" },
    toBash: (o) => {
      const label = bashEscapeDq(o.label ?? "⚠200k");
      return atomSegBash(`-r 'if .exceeds_200k_tokens == true then "${label}" else empty end'`);
    },
    toPreview: (o) => o.label ?? "⚠200k",
    fields: [
      { key: "label", label: "文案", type: "string", placeholder: "⚠200k" },
    ],
  },
  {
    type: "agent",
    name: "Agent 名称",
    icon: "team",
    desc: "agent.name — agent 名称（未配置时隐藏）",
    defaultOptions: {},
    toBash: () => atomSegBash(`-r '.agent.name // empty'`),
    toPreview: () => "reviewer",
    fields: [],
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

/**
 * Ordered segment categories for the add-segment picker. Each entry lists the
 * segment types under that group; the picker renders a labeled header per group.
 * i18n: `statusline.segCat.<id>`.
 */
const SEGMENT_CATEGORIES: { id: string; label: string; types: SegmentType[] }[] = [
  { id: "common", label: "常用", types: ["model", "context-bar", "context-pct", "git", "cost", "rate-limits", "effort", "vim", "separator"] },
  { id: "cost", label: "成本 / 执行", types: ["cost-usd", "session-duration", "api-duration", "lines-changed"] },
  { id: "context", label: "上下文", types: ["context-tokens", "context-max", "context-remaining", "context-cache"] },
  { id: "rate", label: "速率限制", types: ["rate-limit-5h", "rate-limit-7d"] },
  { id: "git", label: "Git", types: ["git-branch", "git-host", "git-owner", "git-repo", "git-repo-full", "git-worktree"] },
  { id: "session", label: "目录 / 会话", types: ["cwd", "project-dir", "added-dirs", "session-id", "session-name", "transcript-path"] },
  { id: "worktree", label: "Worktree", types: ["worktree-name", "worktree-branch", "worktree-original-branch"] },
  { id: "pr", label: "Pull Request", types: ["pr-number", "pr-url", "pr-state"] },
  { id: "other", label: "其他", types: ["version", "output-style", "thinking", "token-warn", "agent", "custom"] },
];

/**
 * Built-in default 3-line layout (PRD). Applied only when no `segments` exist
 * (first run) or on explicit reset — existing user layouts are never overwritten.
 *
 * Separators are now explicit `separator` segments inserted between stable items
 * (`·` row1/3). Conditional separators that must vanish when their neighbour
 * degrades to empty (`[cost]·`, `·worktree`, `coding · `/`balance · ` group
 * segments, `|pwd`) stay on per-segment reserved affix options (`affixPre` /
 * `affixSuf`) so an empty body leaves no orphaned separator char.
 *
 * Colors are fixed hex per PRD: model 蓝 / tokens 紫 / cost 灰 / ctx·cache 绿 /
 * branch 黄 / version 灰. Row 3 coding/balance self-color dynamically (no fixed
 * `color`) via group*DynBash. Separator segments inherit no color (terminal default).
 */
export const DEFAULT_SEGMENTS: StatusLineSegment[] = [
  // ── Row 1: model · tokens[cost]·ctx%·缓存 X% ──
  { id: "d-model", type: "model", enabled: true, newline: false, color: "#4A9EFF",
    options: { format: "short" } },
  { id: "d-sep1", type: "separator", enabled: true, newline: false,
    options: { char: " · " } },
  { id: "d-tokens", type: "context-tokens", enabled: true, newline: false, color: "#BF5AF2",
    options: { mode: "sum", abbrev: true } },
  // cost hugs brackets and trails its own `·` so it disappears cleanly when empty.
  { id: "d-cost", type: "cost-usd", enabled: true, newline: false, color: "#8E8E93",
    options: { prefix: "$", affixPre: "[", affixSuf: "]·" } },
  { id: "d-ctx", type: "context-pct", enabled: true, newline: false, color: "#34C759",
    options: {} },
  { id: "d-cache", type: "context-cache", enabled: true, newline: false, color: "#34C759",
    options: { mode: "hitrate", prefix: "缓存 ", affixPre: "·" } },
  // ── Row 2: branch[·worktree]|pwd ──
  { id: "d-branch", type: "git-branch", enabled: true, newline: true, color: "#FFD60A",
    options: {} },
  { id: "d-worktree", type: "worktree-name", enabled: true, newline: false,
    options: { affixPre: "·" } },
  { id: "d-cwd", type: "cwd", enabled: true, newline: false,
    options: { format: "full", affixPre: "|" } },
  // ── Row 3: coding-or-balance · version ──
  // version carries its own leading ` · ` affix; coding/balance are mutually
  // exclusive and concatenate directly. When both are empty the ` · ` still
  // prefixes version as a decorative bullet.
  { id: "d-coding", type: "group-coding", enabled: true, newline: true,
    options: { dynamicColor: true } },
  { id: "d-balance", type: "group-balance", enabled: true, newline: false,
    options: { dynamicColor: true, prefix: "$", affixPre: "·" } },
  { id: "d-version", type: "version", enabled: true, newline: false, color: "#8E8E93",
    options: { prefix: "v", affixPre: " · " } },
];

/**
 * Built-in default SubagentStatusLine layout. Subagent now shares the exact same
 * segment editor as the main statusline (no templates) — this is its first-run /
 * reset default. Renders a single line:
 *
 *   [Agent·●]<子代理名>·<ctx%>·<tokens>·<时长>
 *   e.g. [Agent·●]reviewer·48%·96K·6m40s
 *
 * `[Agent·●]` is a literal prefix (separator segment) hugging the name directly;
 * the name falls back `.agent.name → .session_name → "subagent"` so it never
 * disappears. Remaining metrics are `·`-separated and degrade to empty when the
 * underlying field is absent (leaving an orphan `·` only in the degenerate case,
 * acceptable per PRD readability tradeoff).
 */
export const DEFAULT_SUBAGENT_SEGMENTS: StatusLineSegment[] = [
  { id: "sa-prefix", type: "separator", enabled: true, newline: false, color: "#8E8E93",
    options: { char: "[Agent·●]" } },
  { id: "sa-name", type: "custom", enabled: true, newline: false, color: "#4A9EFF",
    options: { expr: ".agent.name // .session_name // \"subagent\"" } },
  { id: "sa-sep1", type: "separator", enabled: true, newline: false,
    options: { char: "·" } },
  { id: "sa-ctx", type: "context-pct", enabled: true, newline: false, color: "#34C759",
    options: {} },
  { id: "sa-sep2", type: "separator", enabled: true, newline: false,
    options: { char: "·" } },
  { id: "sa-tokens", type: "context-tokens", enabled: true, newline: false, color: "#BF5AF2",
    options: { mode: "sum", abbrev: true } },
  { id: "sa-sep3", type: "separator", enabled: true, newline: false,
    options: { char: "·" } },
  { id: "sa-dur", type: "session-duration", enabled: true, newline: false, color: "#8E8E93",
    options: { format: "human" } },
];

/**
 * Escape a literal string for safe inclusion inside a bash double-quoted string.
 * Backslash, double-quote, backtick and `$` must be escaped so the separator is
 * emitted verbatim and can never trigger expansion/command substitution.
 */
function bashEscapeDq(s: string): string {
  return s.replace(/[\\"`$]/g, m => "\\" + m);
}

// ── Available data fields reference ──

const STATUSLINE_DATA_FIELDS = [
  { id: "model", group: "模型", fields: [
    { key: "model.id", desc: "模型标识符" },
    { key: "model.display_name", desc: "模型显示名称" },
  ]},
  { id: "workspace", group: "工作区", fields: [
    { key: "workspace.current_dir", desc: "当前工作目录" },
    { key: "workspace.project_dir", desc: "项目启动目录" },
    { key: "workspace.repo.owner/name", desc: "Git 仓库标识" },
  ]},
  { id: "cost", group: "成本", fields: [
    { key: "cost.total_cost_usd", desc: "累计预估成本 ($)" },
    { key: "cost.total_duration_ms", desc: "总持续时间 (ms)" },
    { key: "cost.total_api_duration_ms", desc: "API 等待时间 (ms)" },
  ]},
  { id: "contextWindow", group: "上下文窗口", fields: [
    { key: "context_window.used_percentage", desc: "已使用百分比" },
    { key: "context_window.context_window_size", desc: "最大窗口大小" },
  ]},
  { id: "rateLimits", group: "速率限制", fields: [
    { key: "rate_limits.five_hour.used_percentage", desc: "5小时窗口使用 %" },
    { key: "rate_limits.seven_day.used_percentage", desc: "7天窗口使用 %" },
  ]},
  { id: "other", group: "其他", fields: [
    { key: "effort.level", desc: "推理工作量" },
    { key: "vim.mode", desc: "Vim 模式" },
    { key: "session_id", desc: "会话 ID" },
    { key: "version", desc: "Claude Code 版本" },
  ]},
];

// ── Script generation from segments ──

/** Group active segments into rows (split on `newline`). Returns rows with align. */
function groupRows(segments: StatusLineSegment[]): { align: RowAlign; segs: StatusLineSegment[] }[] {
  const rows: { align: RowAlign; segs: StatusLineSegment[] }[] = [];
  let cur: StatusLineSegment[] | null = null;
  for (const seg of segments) {
    if (cur === null || (seg.newline && cur.length > 0)) {
      cur = [];
      rows.push({ align: seg.align ?? "left", segs: cur });
    }
    cur.push(seg);
  }
  return rows;
}

/**
 * Re-derive `newline` flags so the row model stays self-consistent after any
 * structural mutation (drag-reorder, delete, enable-toggle).
 *
 * The row model is *derived* from `newline`: a row break is any segment with
 * `newline === true`, plus the implicit break before the first segment. Drag
 * reordering moves items in the flat array without touching `newline`, which can
 * leave the new first segment carrying `newline: true` (a redundant leading
 * break) or strand a row break inside the array in a way that silently merges
 * rows. Both make "this row" ambiguous and break per-row delete.
 *
 * Invariant enforced here: the first segment never carries `newline: true`
 * (its row break is implicit). All other `newline` flags are preserved, so the
 * visible row count and membership are stable across reorders.
 */
function normalizeSegments(segments: StatusLineSegment[]): StatusLineSegment[] {
  if (segments.length === 0) return segments;
  return segments.map((s, i) =>
    i === 0 ? (s.newline ? { ...s, newline: false } : s) : s,
  );
}

/** True when the segment starts a row (first active segment, or newline=true). */
function isRowLeaderSeg(segments: StatusLineSegment[], id: string): boolean {
  const active = segments.filter(s => s.enabled);
  const idx = active.findIndex(s => s.id === id);
  if (idx < 0) {
    // disabled segment — leads if it has explicit newline
    return !!segments.find(s => s.id === id)?.newline;
  }
  return idx === 0 || !!active[idx].newline;
}

/**
 * Auto-color bash snippet. Computes 38;2;R;G;B from a numeric value held in a
 * shell variable, by thresholds, then echoes the colored text. Self-contained.
 * `valueExpr` is a bash command substitution yielding an integer 0–100 (or cost*100).
 */
function autoColorBash(type: SegmentType, body: string): string {
  // body = the plain `echo -n "..."` snippet (single line). We wrap it.
  // Extract a numeric metric for the threshold; per type.
  let metric: string;
  let thresholds: string; // bash if/elif producing __r __g __b
  // Percentage-style thresholds: >80 red, >60 amber, else green.
  const pctThresholds =
    `if [ "$__m" -gt 80 ]; then __c="255;69;58"; elif [ "$__m" -gt 60 ]; then __c="255;159;10"; else __c="52;199;89"; fi`;
  if (type === "context-pct" || type === "context-bar") {
    metric = `__m=$(echo "$input" | jq -r '(.context_window.used_percentage // 0) | round')`;
    thresholds = pctThresholds;
  } else if (type === "context-remaining") {
    // Inverted: low remaining is bad.
    metric = `__m=$(echo "$input" | jq -r '(.context_window.remaining_percentage // 100) | round')`;
    thresholds =
      `if [ "$__m" -lt 20 ]; then __c="255;69;58"; elif [ "$__m" -lt 40 ]; then __c="255;159;10"; else __c="52;199;89"; fi`;
  } else if (type === "cost" || type === "cost-usd") {
    // cents
    metric = `__m=$(echo "$input" | jq -r '((.cost.total_cost_usd // 0) * 100) | round')`;
    thresholds =
      `if [ "$__m" -gt 1000 ]; then __c="255;69;58"; elif [ "$__m" -gt 100 ]; then __c="255;159;10"; else __c="52;199;89"; fi`;
  } else if (type === "rate-limit-5h") {
    metric = `__m=$(echo "$input" | jq -r '(.rate_limits.five_hour.used_percentage // 0) | round')`;
    thresholds = pctThresholds;
  } else if (type === "rate-limit-7d") {
    metric = `__m=$(echo "$input" | jq -r '(.rate_limits.seven_day.used_percentage // 0) | round')`;
    thresholds = pctThresholds;
  } else if (type === "session-duration" || type === "api-duration") {
    // Seconds: >300s red, >60s amber, else green.
    const field = type === "api-duration" ? "total_api_duration_ms" : "total_duration_ms";
    metric = `__m=$(echo "$input" | jq -r '((.cost.${field} // 0) / 1000) | round')`;
    thresholds =
      `if [ "$__m" -gt 300 ]; then __c="255;69;58"; elif [ "$__m" -gt 60 ]; then __c="255;159;10"; else __c="52;199;89"; fi`;
  } else { // rate-limits — use the higher of 5h/7d
    metric = `__m=$(echo "$input" | jq -r '[(.rate_limits.five_hour.used_percentage // 0), (.rate_limits.seven_day.used_percentage // 0)] | max | round')`;
    thresholds = pctThresholds;
  }
  // Capture the segment's stdout (body is one or more `echo -n` lines), then
  // emit it wrapped in ANSI truecolor. `{ … ; }` groups multi-line bodies.
  return `__t="$({\n${body}\n})"\n${metric}\n${thresholds}\nprintf '\\033[38;2;%sm%s\\033[0m' "$__c" "$__t"`;
}

/** Wrap an `echo -n "..."` snippet with fixed-color ANSI truecolor. */
function fixedColorBash(body: string, rgb: [number, number, number]): string {
  const [r, g, b] = rgb;
  return `__t="$({\n${body}\n})"\nprintf '\\033[38;2;${r};${g};${b}m%s\\033[0m' "$__t"`;
}

export function generateStatusLineScript(segments: StatusLineSegment[]): string {
  const active = segments.filter(s => s.enabled);
  if (active.length === 0) return "#!/usr/bin/env bash\necho ''\n";

  const lines: string[] = [
    "#!/usr/bin/env bash",
    "# Generated by aidog — do not edit manually",
    "input=$(cat)",
    `__cols=$(echo "$input" | jq -r '.terminal.width // 0')`,
    "",
  ];

  // Fetch the aidog group-info endpoint once (shared by all group-* segments)
  // before any row renders. Omitted entirely when no group segment is active.
  if (active.some(s => GROUP_SEG_TYPES.has(s.type))) {
    lines.push(aidogFetchPrelude());
  }

  const rows = groupRows(active);

  for (let i = 0; i < rows.length; i++) {
    const { align, segs } = rows[i];
    lines.push(`# ── row ${i + 1} (${align}) ──`);
    lines.push(`__line${i}=""`);
    for (const seg of segs) {
      const def = SEGMENT_DEF_MAP.get(seg.type);
      if (!def) continue;
      const opts = { ...def.defaultOptions, ...seg.options };
      // Reserved affix options drive mixed in-row separators (see wrapAffix).
      const affixPre = typeof opts.affixPre === "string" ? opts.affixPre : "";
      const affixSuf = typeof opts.affixSuf === "string" ? opts.affixSuf : "";
      const body = wrapAffix(def.toBash(opts), affixPre, affixSuf);
      let snippet: string;
      if (seg.autoColor && VALUE_COLORABLE.has(seg.type)) {
        snippet = autoColorBash(seg.type, body);
      } else {
        const rgb = hexToRgb(seg.color);
        snippet = rgb ? fixedColorBash(body, rgb) : body;
      }
      // Each segment runs in its own subshell; its full (possibly ANSI-wrapped)
      // output is captured as one unit so word-splitting never severs color codes.
      // Separators are now explicit `separator` segments inserted between items;
      // any segment (incl. separator) that degrades to empty simply appends "".
      lines.push(`__seg="$(\n${snippet}\n)"`);
      lines.push(`__line${i}+="$__seg"`);
    }
    if (align === "center" || align === "right") {
      // Strip ANSI for visible-width measurement, then pad with printf.
      lines.push(`__vis${i}=$(printf '%s' "$__line${i}" | sed 's/\\x1b\\[[0-9;]*m//g')`);
      lines.push(`__w${i}=\${#__vis${i}}`);
      lines.push(`if [ "$__cols" -gt 0 ] && [ "$__cols" -gt "$__w${i}" ]; then`);
      if (align === "center") {
        lines.push(`  __pad${i}=$(( (__cols - __w${i}) / 2 ))`);
      } else {
        lines.push(`  __pad${i}=$(( __cols - __w${i} ))`);
      }
      lines.push(`  printf '%*s%s\\n' "$__pad${i}" '' "$__line${i}"`);
      lines.push(`else`);
      lines.push(`  printf '%s\\n' "$__line${i}"`);
      lines.push(`fi`);
    } else {
      lines.push(`printf '%s\\n' "$__line${i}"`);
    }
    if (i < rows.length - 1) lines.push("");
  }

  return lines.join("\n");
}

/**
 * Resolved materialization for a statusLine / subagentStatusLine config block.
 * `scriptContent` is the bash script body to write (builtin mode) or `null`
 * (custom mode / disabled — nothing to generate). `customCommand` is the
 * user-supplied native command (custom mode only). `padding` is carried so the
 * caller can assemble the native field.
 */
export interface StatuslineMaterialization {
  enabled: boolean;
  mode: "builtin" | "custom";
  scriptContent: string | null;
  customCommand: string;
}

/**
 * Pure resolver: given a stored `_aidog_statusline` / `_aidog_subagent_statusline`
 * block and its scriptType, derive everything needed to materialize the native
 * `statusLine` / `subagentStatusLine` field — applying all default logic
 * (segments → DEFAULT_SEGMENTS, subagent template selection)
 * in one authoritative place. No side effects; the caller persists the result.
 *
 * Mirrors StatusLinePanel's in-component derivations so the on-save materializer
 * and the live UI agree byte-for-byte.
 */
export function materializeStatusline(
  stored: Record<string, any> | undefined,
  scriptType: "statusline" | "subagent",
): StatuslineMaterialization {
  const s = (stored ?? {}) as Record<string, any>;
  const isMain = scriptType === "statusline";
  const enabled = !!s.enabled;
  const mode: "builtin" | "custom" = s.mode === "custom" ? "custom" : "builtin";
    const customCommand = typeof s.customCommand === "string" ? s.customCommand : "";

  let scriptContent: string | null = null;
  if (enabled && mode === "builtin") {
    if (!isMain) {
      // Subagent statusline uses JSONL output (per-task overrides):
      //   stdin:  {tasks: [{id, name, type, status, …}]}
      //   stdout: {"id":"…","content":"…"} per task
      // The segment-based bash generator outputs plain text lines — incompatible.
      // Delegate to the dedicated Python script which handles JSONL correctly.
      const subagentScript = "python3 ~/persons/lyxamour/ccplugin/scripts/subagent_statusline.py";
      return { enabled: true, mode: "custom", scriptContent: null, customCommand: subagentScript };
    }
    // main statusline — segment-based bash generator.
    const segments: StatusLineSegment[] =
      (s.segments as StatusLineSegment[] | undefined) ?? DEFAULT_SEGMENTS.map(seg => ({ ...seg }));
    scriptContent = generateStatusLineScript(segments);
  }

  return { enabled, mode, scriptContent, customCommand };
}

/** Mock metric values used to drive autoColor preview (matches bash thresholds). */
const PREVIEW_METRIC: Record<string, number> = {
  "context-pct": 65,
  "context-bar": 65,
  "cost": 12,          // cents
  "cost-usd": 12,      // cents
  "rate-limits": 41,
  "rate-limit-5h": 34,
  "rate-limit-7d": 62,
  "context-remaining": 49,
  "session-duration": 285, // seconds
  "api-duration": 15,      // seconds
};

/** Map a mock metric to the same semantic color the bash thresholds produce. */
function autoColorPreviewHex(type: SegmentType): string {
  const m = PREVIEW_METRIC[type] ?? 0;
  if (type === "cost" || type === "cost-usd") {
    if (m > 1000) return "#ff453a";
    if (m > 100) return "#ff9f0a";
    return "#34c759";
  }
  if (type === "context-remaining") {
    if (m < 20) return "#ff453a";
    if (m < 40) return "#ff9f0a";
    return "#34c759";
  }
  if (type === "session-duration" || type === "api-duration") {
    if (m > 300) return "#ff453a";
    if (m > 60) return "#ff9f0a";
    return "#34c759";
  }
  if (m > 80) return "#ff453a";
  if (m > 60) return "#ff9f0a";
  return "#34c759";
}

/** Resolve the preview color for a segment (fixed hex or autoColor), or null. */
function previewColor(seg: StatusLineSegment): string | null {
  if (seg.autoColor && VALUE_COLORABLE.has(seg.type)) return autoColorPreviewHex(seg.type);
  const rgb = hexToRgb(seg.color);
  return rgb ? `#${rgb.map(v => v.toString(16).padStart(2, "0")).join("")}` : null;
}

/** Render a colored, row-grouped, aligned live preview of the segments. */
function StatusLinePreview({ segments, empty }: { segments: StatusLineSegment[]; empty: string }) {
  const active = segments.filter(s => s.enabled);
  if (active.length === 0) {
    return <span style={{ color: "var(--text-tertiary)" }}>{empty}</span>;
  }
  const rows = groupRows(active);
  return (
    <>
      {rows.map((row, ri) => (
        <div key={ri} style={{
          display: "flex",
          justifyContent: row.align === "center" ? "center" : row.align === "right" ? "flex-end" : "flex-start",
        }}>
          {row.segs.map((seg) => {
            const def = SEGMENT_DEF_MAP.get(seg.type);
            if (!def) return null;
            const color = previewColor(seg);
            const opts = { ...def.defaultOptions, ...seg.options };
            const affixPre = typeof opts.affixPre === "string" ? opts.affixPre : "";
            const affixSuf = typeof opts.affixSuf === "string" ? opts.affixSuf : "";
            return (
              <span key={seg.id} style={color ? { color } : undefined}>
                {affixPre}{def.toPreview(opts)}{affixSuf}
              </span>
            );
          })}
        </div>
      ))}
    </>
  );
}

// ── Segment Edit Modal ──

function SegmentEditModal({
  segment,
  isRowLeader,
  onSave,
  onClose,
  t,
}: {
  segment: StatusLineSegment;
  isRowLeader: boolean;
  onSave: (patch: Partial<StatusLineSegment>) => void;
  onClose: () => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const def = SEGMENT_DEF_MAP.get(segment.type);
  const [opts, setOpts] = useState({ ...(def?.defaultOptions ?? {}), ...segment.options });
  const [newline, setNewline] = useState(segment.newline);
  const [color, setColor] = useState<string>(segment.color ?? "");
  const [autoColor, setAutoColor] = useState<boolean>(!!segment.autoColor);
  const [align, setAlign] = useState<RowAlign>(segment.align ?? "left");
  if (!def) return null;
  const canAutoColor = VALUE_COLORABLE.has(segment.type);
  const validHex = hexToRgb(color) != null;
  const effectiveColor = autoColor && canAutoColor
    ? autoColorPreviewHex(segment.type)
    : (validHex ? color : null);

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
            {t(`statusline.seg.${def.type}.name`, def.name)}
          </div>
          <button type="button" className="btn btn-ghost btn-icon"
            style={{ width: 28, height: 28, fontSize: F.body }}
            onClick={onClose}>×</button>
        </div>
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 16 }}>{t(`statusline.seg.${def.type}.desc`, def.desc)}</div>

        {/* Newline toggle */}
        <label style={{
          display: "flex", alignItems: "center", gap: 8, marginBottom: 16,
          padding: "8px 12px", background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
          fontSize: F.body, color: "var(--text-primary)", cursor: "pointer",
        }}>
          <Toggle active={newline} onChange={setNewline} />
          {t("statusline.segNewline")}
        </label>

        {/* Row alignment (only meaningful when this segment leads a row) */}
        {(isRowLeader || newline) && (
          <div style={{ marginBottom: 16 }}>
            <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 6 }}>{t("statusline.rowAlign")}</div>
            <div style={{ display: "flex", gap: 6 }}>
              {(["left", "center", "right"] as RowAlign[]).map(a => {
                const active = align === a;
                return (
                  <button key={a} type="button"
                    style={{
                      flex: 1, padding: "6px 10px", fontSize: F.body,
                      fontWeight: active ? 600 : 400,
                      color: active ? "var(--accent)" : "var(--text-secondary)",
                      background: active ? "var(--accent-subtle, rgba(0,122,255,0.1))" : "transparent",
                      border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
                      borderRadius: "var(--radius-sm)", cursor: "pointer",
                    }}
                    onClick={() => setAlign(a)}>
                    {t(`statusline.align.${a}`)}
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {/* Color controls */}
        <div style={{ marginBottom: 16 }}>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 6 }}>{t("statusline.color")}</div>
          {canAutoColor && (
            <label style={{
              display: "flex", alignItems: "center", gap: 8, marginBottom: 10,
              fontSize: F.body, color: "var(--text-primary)", cursor: "pointer",
            }}>
              <Toggle active={autoColor} onChange={setAutoColor} />
              {t("statusline.autoColor")}
            </label>
          )}
          <div style={{
            display: "flex", alignItems: "center", gap: 8,
            opacity: autoColor && canAutoColor ? 0.45 : 1,
            pointerEvents: autoColor && canAutoColor ? "none" : "auto",
          }}>
            <input
              type="color"
              value={validHex ? `#${hexToRgb(color)!.map(v => v.toString(16).padStart(2, "0")).join("")}` : "#4a9eff"}
              onChange={(e) => setColor(e.target.value)}
              style={{
                width: 36, height: 30, padding: 0, border: "1px solid var(--border)",
                borderRadius: "var(--radius-sm)", background: "transparent", cursor: "pointer", flexShrink: 0,
              }}
            />
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
              value={color} placeholder="#4A9EFF"
              onChange={(e) => setColor(e.target.value)} />
            <button type="button" className="btn btn-ghost"
              style={{ fontSize: F.hint, padding: "4px 10px", color: "var(--text-tertiary)" }}
              onClick={() => setColor("")}>
              {t("statusline.clearColor")}
            </button>
          </div>
        </div>

        {/* Type-specific fields */}
        {def.fields.length > 0 && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12, marginBottom: 16 }}>
            {def.fields.map(f => (
              <div key={f.key} style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <label style={{ fontSize: F.body, color: "var(--text-secondary)", minWidth: 100, flexShrink: 0 }}>
                  {t(`statusline.seg.${def.type}.field.${f.key}`, f.label)}
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
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 4 }}>{t("statusline.preview")}</div>
          <div style={{
            padding: "8px 14px", background: "var(--bg-surface)",
            borderRadius: "var(--radius-sm)", fontSize: F.body,
            fontFamily: '"SF Mono", "Fira Code", monospace',
            color: "var(--text-primary)",
          }}>
            <span style={effectiveColor ? { color: effectiveColor } : undefined}>
              {def.toPreview(opts)}
            </span>
          </div>
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="btn btn-ghost" style={{ fontSize: F.body, padding: S.btnPad }} onClick={onClose}>
            {t("statusline.cancel")}
          </button>
          <button className="btn btn-primary" style={{ fontSize: F.body, padding: S.btnPad }}
            onClick={() => {
              onSave({
                options: opts,
                newline,
                color: validHex ? color : undefined,
                autoColor: canAutoColor ? autoColor : undefined,
                align: (isRowLeader || newline) ? align : undefined,
              });
              onClose();
            }}>
            {t("statusline.save")}
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
  t,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
  scriptType: "statusline" | "subagent";
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const isMain = scriptType === "statusline";
  const aidogKey = isMain ? "_aidog_statusline" : "_aidog_subagent_statusline";
  const fieldName = isMain ? "statusLine" : "subagentStatusLine";

  const stored = (config[aidogKey] ?? {}) as Record<string, any>;
  const enabled = !!stored.enabled;
  // Generation mode: "builtin" → aidog structured segments; "custom" → user-supplied
  // native statusLine command (no aidog script generated). Back-compat: default builtin.
  const mode: "builtin" | "custom" = stored.mode === "custom" ? "custom" : "builtin";
  const customCommand: string = typeof stored.customCommand === "string" ? stored.customCommand : "";

  // Segments — main and subagent share the same editor; only the first-run /
  // reset default layout differs.
  const defaultSegments = isMain ? DEFAULT_SEGMENTS : DEFAULT_SUBAGENT_SEGMENTS;
  const segments: StatusLineSegment[] =
    stored.segments ?? defaultSegments.map(s => ({ ...s }));

  const [showScript, setShowScript] = useState(false);
  const [saving, setSaving] = useState(false);
  const [editSeg, setEditSeg] = useState<StatusLineSegment | null>(null);
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

  const updateSegments = (next: StatusLineSegment[]) => setStored({ segments: normalizeSegments(next) });

  /**
   * Delete an entire row by its leader segment id. Resolves the row membership
   * from the *current* derived grouping (over ALL segments, enabled or not, so
   * the visual row and the deleted set always match), then removes exactly those
   * segment ids. Fixes the bug where, after dragging a segment into another row,
   * deleting "that row" removed the wrong segment set and dropped moved content.
   */
  const deleteRow = (leaderId: string) => {
    // Derive rows over the full segment list (matches the rendered grouping,
    // which keys off `newline` regardless of enabled state).
    const rows: StatusLineSegment[][] = [];
    let cur: StatusLineSegment[] | null = null;
    for (const seg of segments) {
      if (cur === null || (seg.newline && cur.length > 0)) {
        cur = [];
        rows.push(cur);
      }
      cur.push(seg);
    }
    const row = rows.find(r => r.some(s => s.id === leaderId));
    if (!row) return;
    const ids = new Set(row.map(s => s.id));
    updateSegments(segments.filter(s => !ids.has(s.id)));
  };

  // Generate script
  const scriptPreview = generateStatusLineScript(segments);


  const handleSave = async () => {
    setSaving(true);
    try {
      const path = await statuslineApi.generate(scriptType, scriptPreview);
      const value: Record<string, any> = { type: "command", command: path };
      updateField(fieldName, value);
    } catch (e: any) {
      console.error("generate_statusline_script:", e);
    }
    setSaving(false);
  };

  // Live-preview convenience only: keep the native `statusLine` / `subagentStatusLine`
  // draft field roughly in sync while the user edits builtin segments, so the JSON
  // view reflects changes without a save round-trip. This is NO LONGER the
  // persistence path — Settings.handleSave → materializeStatuslineFields is the
  // authoritative, race-free materializer (covers disabled/custom/subagent too,
  // which this effect deliberately does not touch). The effect keys off the *real
  // inputs* (scriptPreview / padding / hideVim / enabled), NOT the generated path,
  // and skips the write when the value is unchanged — keeping `updateField`
  // idempotent so the dirty state never thrashes / loops. Because the save writes
  // the same stable command path, the two never conflict.
  const lastWrittenRef = useRef<string>("");
  useEffect(() => {
    if (!enabled || mode !== "builtin") return;
    let cancelled = false;
    const timer = setTimeout(async () => {
      try {
        const path = await statuslineApi.generate(scriptType, scriptPreview);
        if (cancelled) return;
        const value: Record<string, any> = { type: "command", command: path };
  
        const signature = JSON.stringify(value);
        // Skip when the field already holds this exact value → no spurious dirty.
        const current = config[fieldName];
        if (signature === lastWrittenRef.current && JSON.stringify(current) === signature) return;
        lastWrittenRef.current = signature;
        updateField(fieldName, value);
      } catch (e: any) {
        console.error("auto generate_statusline_script:", e);
      }
    }, 500);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // Depends on real inputs only (scriptPreview captures segments/template).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, mode, scriptPreview, scriptType, isMain, fieldName]);

  // Apply custom mode: write the native Claude Code statusLine command directly,
  // bypassing aidog script generation. Empty command clears the field.
  const handleApplyCustom = () => {
    const cmd = customCommand.trim();
    if (!cmd) {
      updateField(fieldName, undefined);
      return;
    }
    const value: Record<string, any> = { type: "command", command: cmd };
    updateField(fieldName, value);
  };

  // Switch generation mode. Clears the live native field so the two modes never
  // leave a stale config behind (user re-applies in the newly selected mode).
  const switchMode = (next: "builtin" | "custom") => {
    if (next === mode) return;
    updateField(fieldName, undefined);
    setStored({ mode: next });
  };

  const addSegment = (type: SegmentType, newline = false) => {
    const def = SEGMENT_DEF_MAP.get(type);
    if (!def) return;
    const newSeg: StatusLineSegment = {
      id: `s${Date.now()}`,
      type,
      enabled: true,
      newline,
      options: { ...def.defaultOptions },
    };
    updateSegments([...segments, newSeg]);
    setShowAddMenu(false);
  };

  // Add a brand-new row: append a model segment that starts a new line.
  const addRow = () => {
    addSegment("model", segments.length > 0);
  };

  // Restore the built-in default 3-line layout (segments + empty affix-carried
  // separator). Explicit user action only — never auto-applied over a saved layout.
  const resetToDefaultLayout = () => {
    setStored({
      segments: defaultSegments.map(s => ({ ...s, options: { ...s.options } })),
    });
  };

  // Toggle alignment on the row that owns the given segment (set on its leader).
  const cycleRowAlign = (segId: string) => {
    const active = segments.filter(s => s.enabled);
    const idx = active.findIndex(s => s.id === segId);
    if (idx < 0) return;
    // Walk back to the row leader (first seg or newline=true).
    let leaderIdx = idx;
    while (leaderIdx > 0 && !active[leaderIdx].newline) leaderIdx--;
    const leaderId = active[leaderIdx].id;
    const order: RowAlign[] = ["left", "center", "right"];
    const cur = active[leaderIdx].align ?? "left";
    const nextAlign = order[(order.indexOf(cur) + 1) % order.length];
    updateSegments(segments.map(s => s.id === leaderId ? { ...s, align: nextAlign } : s));
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
            {isMain ? t("statusline.useBuiltin", "使用内置状态栏") : t("statusline.useBuiltinSubagent", "使用内置子代理状态栏")}
          </div>
          <Hint>{isMain
            ? t("statusline.builtinDesc", "开启后 aidog 生成脚本到 ~/.aidog/aidog-statusline.sh")
            : t("statusline.builtinSubagentDesc", "开启后 aidog 生成脚本到 ~/.aidog/aidog-subagent-statusline.sh")}</Hint>
        </div>
        {enabled && (
          <span style={{
            fontSize: F.small, fontWeight: 600, color: "#34c759",
            padding: "2px 8px", background: "rgba(52,199,89,0.12)", borderRadius: "var(--radius-sm)",
          }}>● {t("statusline.enabled", "已启用")}</span>
        )}
      </div>

      {enabled && (
        <>
          {/* Mode selector: builtin structured segments vs custom native command */}
          <div style={{ display: "flex", gap: 6 }}>
            {(["builtin", "custom"] as const).map(m => {
              const active = mode === m;
              return (
                <button key={m} type="button"
                  style={{
                    flex: 1, padding: "8px 12px", fontSize: F.body, fontWeight: active ? 600 : 400,
                    color: active ? "var(--accent)" : "var(--text-secondary)",
                    background: active ? "var(--accent-subtle, rgba(0,122,255,0.1))" : "transparent",
                    border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
                    borderRadius: "var(--radius-sm)", cursor: "pointer",
                  }}
                  onClick={() => switchMode(m)}>
                  {m === "builtin"
                    ? t("statusline.modeBuiltin", "内置结构化")
                    : t("statusline.modeCustom", "自定义脚本")}
                </button>
              );
            })}
          </div>
        </>
      )}

      {enabled && mode === "custom" && (
        <div style={{
          padding: "12px 16px", background: "var(--bg-surface)", borderRadius: "var(--radius-md)",
          border: "1px solid var(--border)", display: "flex", flexDirection: "column", gap: 12,
        }}>
          <Hint>{t("statusline.customDesc", "按原生 statusLine 格式分字段填写，写入 settings 的 command 字段，不生成 aidog 脚本")}</Hint>
          {/* type — 固定 command，只读展示 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <label style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("statusline.customType", "类型")}</label>
            <input className="input" readOnly value="command"
              style={{ fontSize: F.body, padding: S.inputPad, width: 140, opacity: 0.7, fontFamily: '"SF Mono", "Fira Code", monospace' }} />
          </div>
          {/* command — 脚本路径 / 命令 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <label style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("statusline.customCommand", "命令 / 脚本路径")}</label>
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad }}
              value={customCommand}
              placeholder={t("statusline.customPlaceholder", "~/.claude/my-statusline.sh 或 inline 命令")}
              onChange={(e) => setStored({ customCommand: e.target.value })} />
            <Hint>{t("statusline.customCommandDesc", "支持绝对路径、~ 路径或内联命令")}</Hint>
          </div>
          <div style={{ display: "flex", justifyContent: "flex-end" }}>
            <button className="btn btn-primary" style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={handleApplyCustom}>
              {t("statusline.applyCustom", "应用自定义脚本")}
            </button>
          </div>
        </div>
      )}

      {enabled && mode === "builtin" && (
        <>
          {/* Preview */}
          <div style={{
            padding: "12px 16px", background: "var(--bg-surface)", borderRadius: "var(--radius-md)",
            border: "1px solid var(--border)",
          }}>
            <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 6 }}>{t("statusline.preview")}</div>
            <div style={{
              fontFamily: '"SF Mono", "Fira Code", monospace', fontSize: F.body,
              color: "var(--text-primary)", lineHeight: 1.6,
            }}>
              <StatusLinePreview segments={segments} empty={t("statusline.previewEmpty")} />
            </div>
          </div>

          {/* ── Drag-sortable segment list (shared by main & subagent) ── */}
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <SortableList
                items={segments}
                onReorder={updateSegments}
                renderItem={(seg, handle) => {
                  const def = SEGMENT_DEF_MAP.get(seg.type);
                  if (!def) return null;
                  const leader = isRowLeaderSeg(segments, seg.id);
                  const segColor = previewColor(seg);
                  return (
                    <div style={{ marginBottom: 6 }}>
                    {/* Row-leader bar: new-line marker + alignment */}
                    {leader && (
                      <div style={{
                        display: "flex", alignItems: "center", gap: 8,
                        padding: "2px 4px 4px", fontSize: F.hint, color: "var(--text-tertiary)",
                      }}>
                        <span style={{ fontWeight: 600 }}>{t("statusline.rowLabel")}</span>
                        <button type="button" className="btn btn-ghost"
                          style={{ fontSize: F.hint, padding: "2px 8px", color: "var(--accent)" }}
                          onClick={() => cycleRowAlign(seg.id)}
                          title={t("statusline.rowAlign")}>
                          {t(`statusline.align.${seg.align ?? "left"}`)}
                        </button>
                        <button type="button" className="btn btn-ghost"
                          style={{ fontSize: F.hint, padding: "2px 8px", color: "var(--text-tertiary)" }}
                          onClick={() => deleteRow(seg.id)}
                          title={t("statusline.deleteRow", "删除整行")}>
                          {t("statusline.deleteRow", "删除整行")}
                        </button>
                      </div>
                    )}
                    <div className="glass-surface" style={{
                      display: "flex", alignItems: "center", gap: 10,
                      padding: "10px 12px",
                      borderRadius: "var(--radius-md)",
                      opacity: seg.enabled ? 1 : 0.45,
                      border: handle.isDragging ? "1px solid var(--accent)" : "1px solid var(--border)",
                      boxShadow: handle.isDragging ? "0 6px 20px rgba(0,0,0,0.18)" : "none",
                      transition: "opacity 150ms, border-color 150ms",
                    }}>
                      {/* Drag handle (only this element starts the drag) */}
                      <span
                        ref={handle.ref}
                        {...handle.attributes}
                        {...handle.listeners}
                        style={{
                          color: "var(--text-tertiary)", fontSize: F.body,
                          cursor: handle.isDragging ? "grabbing" : "grab",
                          userSelect: "none", touchAction: "none",
                          padding: "0 2px", lineHeight: 1,
                        }}
                        title={t("statusline.dragSort", "拖动排序")}
                      ><IconMenu size={15} /></span>
                      {/* Toggle */}
                      <Toggle active={seg.enabled} onChange={(v) => {
                        const next = segments.map(s => s.id === seg.id ? { ...s, enabled: v } : s);
                        updateSegments(next);
                      }} />
                      {/* Name */}
                      <span style={{ fontSize: F.body, fontWeight: 600, color: "var(--text-primary)", flexShrink: 0 }}>
                        {t(`statusline.seg.${def.type}.name`, def.name)}
                      </span>
                      {/* Inline preview (colored) */}
                      <span style={{
                        flex: 1, fontSize: F.hint,
                        color: segColor ?? "var(--text-tertiary)",
                        fontFamily: '"SF Mono", "Fira Code", monospace',
                        overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                      }}>
                        {def.toPreview({ ...def.defaultOptions, ...seg.options })}
                      </span>
                      {/* Break-to-new-line toggle (moves segment between rows) */}
                      <button type="button" className="btn btn-ghost btn-icon"
                        style={{
                          width: 24, height: 24, minWidth: 24, fontSize: F.hint,
                          color: seg.newline ? "var(--accent)" : "var(--text-tertiary)",
                        }}
                        title={t("statusline.toggleNewline")}
                        onClick={() => updateSegments(segments.map(s => s.id === seg.id ? { ...s, newline: !s.newline } : s))}>↵</button>
                      {/* Edit button */}
                      <button type="button" className="btn btn-ghost"
                        style={{ fontSize: F.hint, padding: "2px 8px", color: "var(--accent)" }}
                        onClick={() => setEditSeg({ ...seg })}>
                        <IconEdit size={13} />
                      </button>
                      {/* Delete */}
                      <button type="button" className="btn btn-ghost btn-icon"
                        style={{ width: 24, height: 24, minWidth: 24, fontSize: F.hint, color: "var(--text-tertiary)" }}
                        onClick={() => updateSegments(segments.filter((s) => s.id !== seg.id))}>
                        <IconClose size={13} />
                      </button>
                    </div>
                    </div>
                  );
                }}
              />

              {/* Add segment / row */}
              <div style={{ position: "relative", display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
                <button className="btn btn-ghost" style={{ fontSize: F.body, padding: "6px 14px", marginRight: "auto", color: "var(--text-tertiary)" }}
                  onClick={resetToDefaultLayout}
                  title={t("statusline.resetLayoutHint", "恢复内置默认 3 行布局")}>
                  {t("statusline.resetLayout", "恢复默认布局")}
                </button>
                <button className="btn btn-ghost" style={{ fontSize: F.body, padding: "6px 14px" }}
                  onClick={addRow}>
                  {t("statusline.addRow")}
                </button>
                <button className="btn btn-ghost" style={{ fontSize: F.body, padding: "6px 14px" }}
                  onClick={() => setShowAddMenu(!showAddMenu)}>
                  {t("statusline.addSegment")}
                </button>
                {showAddMenu && (
                  <div style={{
                    position: "absolute", bottom: "100%", right: 0, zIndex: 100,
                    background: "var(--bg-surface)", border: "1px solid var(--border)",
                    borderRadius: "var(--radius-md)", padding: 4,
                    maxHeight: 360, overflow: "auto", minWidth: 280,
                    boxShadow: "0 8px 32px rgba(0,0,0,0.2)",
                  }}>
                    {SEGMENT_CATEGORIES.map(cat => (
                      <div key={cat.id}>
                        <div style={{
                          padding: "6px 12px 2px", fontSize: F.small, fontWeight: 600,
                          color: "var(--text-tertiary)", textTransform: "uppercase", letterSpacing: 0.4,
                        }}>{t(`statusline.segCat.${cat.id}`, cat.label)}</div>
                        {cat.types.map(type => {
                          const d = SEGMENT_DEF_MAP.get(type);
                          if (!d) return null;
                          return (
                            <button key={d.type} type="button" style={{
                              display: "block", width: "100%", textAlign: "left",
                              padding: "6px 12px", fontSize: F.body,
                              background: "transparent", border: "none", borderRadius: "var(--radius-sm)",
                              cursor: "pointer", color: "var(--text-primary)",
                            }}
                              onMouseEnter={(e) => { e.currentTarget.style.background = "var(--bg-glass)"; }}
                              onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
                              onClick={() => addSegment(d.type)}>
                              <span style={{ fontWeight: 500 }}>{t(`statusline.seg.${d.type}.name`, d.name)}</span>
                              <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginLeft: 8 }}>{t(`statusline.seg.${d.type}.desc`, d.desc)}</span>
                            </button>
                          );
                        })}
                      </div>
                    ))}
                  </div>
                )}
              </div>
          </div>


          {/* Script preview (collapsible) */}
          <div style={{
            padding: "10px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
          }}>
            <button type="button" className="btn btn-ghost"
              style={{ fontSize: F.body, padding: "4px 8px", display: "flex", alignItems: "center", gap: 4, width: "100%", justifyContent: "space-between" }}
              onClick={() => setShowScript(!showScript)}>
              <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                <span style={{ transform: showScript ? "rotate(90deg)" : "rotate(0deg)", transition: "transform 150ms", display: "inline-block" }}>▶</span>
                {t("statusline.scriptPreview", "脚本预览")}
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
              {saving ? t("statusline.generating", "生成中…") : t("statusline.applyGenerate", "应用并生成脚本")}
            </button>
          </div>
        </>
      )}

      {/* Edit modal */}
      {editSeg && (
        <SegmentEditModal
          segment={editSeg}
          isRowLeader={isRowLeaderSeg(segments, editSeg.id)}
          t={t}
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
export function StatusLineSection({
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
        <StatusLinePanel config={config} updateField={updateField} scriptType="statusline" t={t} />
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
        <StatusLinePanel config={config} updateField={updateField} scriptType="subagent" t={t} />
      </div>

      {/* FileSuggestion (keep existing behavior) */}
      {(() => {
        const field: SettingField = {
          key: "fileSuggestion",
          label: "File Suggestion",
          type: "string",
          description: t("statusline.fileSuggestionDesc", "自定义文件建议脚本路径"),
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
          {t("statusline.dataFieldsRef", "可用数据字段参考")}
        </button>
        {showDataRef && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 12 }}>
            <Hint>{t("statusline.dataFieldsHint", "Claude Code 通过 stdin 注入以下 JSON 字段，可在脚本中用 jq 提取")}</Hint>
            {STATUSLINE_DATA_FIELDS.map(group => (
              <div key={group.id}>
                <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 6 }}>
                  {t(`statusline.dataGroup.${group.id}`, group.group)}
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
 * Build the diff tree between `current` config and `incoming` source.
 * Skips internal `_aidog_` keys. Object top-level keys expand to child entries.
 * TODO: only one level of nesting is expanded (covers permissions/env/hooks);
 * deeper objects are diffed as a single leaf.
 */
export function buildImportDiffTree(
  current: Record<string, any>,
  incoming: Record<string, any>,
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
        if (JSON.stringify(curObj[ck]) === JSON.stringify(incObj[ck])) continue;
        children.push({
          path: `${key}.${ck}`,
          label: ck,
          current: curObj[ck],
          incoming: incObj[ck],
        });
      }
      if (children.length > 0) {
        nodes.push({ path: key, label: key, current: cur, incoming: inc, children });
        continue;
      }
    }
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
    const bgColor = changeType === "added" ? "rgba(52,199,89,0.06)"
      : changeType === "removed" ? "rgba(255,69,58,0.06)"
      : "var(--bg-glass)";
    const labelColor = changeType === "added" ? "#34c759"
      : changeType === "removed" ? "#ff453a"
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
                    color: state === "partial" ? "#ff9f0a" : "var(--accent)",
                    padding: "1px 6px",
                    background: state === "partial" ? "rgba(255,159,10,0.12)" : "rgba(0,122,255,0.12)",
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
  const { t } = useTranslation();
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
          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("settings.plugins.skipLfs", "跳过 LFS 下载")}</span>
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
              }}><IconClose size={12} /></button>
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
        <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("settings.plugins.autoRefresh", "启动时自动刷新")}</span>
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
  const { t } = useTranslation();
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
        <Hint>{t("settings.plugins.enabledHint", "格式: plugin-name@marketplace → 启用/禁用")}</Hint>
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
              }}><IconClose size={12} /></button>
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
        <Hint>{t("settings.plugins.marketplacesHint", "命名市场源定义（github / git / directory / settings）")}</Hint>
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
                }}><IconClose size={12} /></button>
              </div>
              <MarketplaceSourceEditor
                source={mktConfig.source ?? { source: "github" }}
                onChange={(s) => updateMarketplace(name, { ...mktConfig, source: s })}
              />
              {/* Path field — local installation path */}
              <div style={{ display: "flex", gap: 6, alignItems: "center", marginTop: 2 }}>
                <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: 80, flexShrink: 0, whiteSpace: "nowrap" }}>Path</span>
                <input className="input" style={{ fontSize: F.hint, padding: "6px 10px", flex: 1 }}
                  placeholder={t("settings.plugins.localPathPh", "本地安装路径（留空自动管理）")}
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
export function PluginsSection({
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
export function PluginsSectionInline({ config, updateField }: {
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

export type HooksConfig = Record<string, MatcherGroup[]>;

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

export function HooksSection({
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
          <option value="">{t("settings.hooks.addEvent", "+ 添加 Hook 事件…")}</option>
          {HOOK_EVENTS.map(ev => (
            <option key={ev.id} value={ev.id}>
              {ev.id} — {t(`settings.hooks.event.${ev.id}.desc`, ev.desc)}
            </option>
          ))}
        </select>
      </div>

      {/* Hint */}
      {totalHooks === 0 && (
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
          {t("settings.hooks.introLine1", "Hooks 在 Claude Code 生命周期的特定点自动执行命令/HTTP请求/LLM提示。")}
          <br />{t("settings.hooks.introLine2", "选择事件类型开始配置。")}
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
                  — {t(`settings.hooks.event.${eventMeta.id}.desc`, eventMeta.desc)}
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
                title={t("settings.hooks.deleteEvent", "删除此事件所有 hooks")}
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
                    {t("settings.hooks.matcher", "匹配器")}
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
                          {t("settings.hooks.customMatcher", "+ 自定义")}: {matcherTags.filter(t => !eventMeta.matcherOptions.includes(t)).join(", ")}
                        </span>
                      )}
                    </>
                  ) : eventMeta?.matcherFreeform ? (
                    <input
                      className="input"
                      style={{ ...inputStyle, flex: 1 }}
                      placeholder={eventMeta?.id === "FileChanged" ? t("settings.hooks.matcherFilePh", "文件名，如 .envrc|.env") : t("settings.hooks.matcherToolPh", "工具名称或正则，多个用 | 分隔")}
                      value={group.matcher}
                      onChange={(e) => updateMatcher(eventId, gi, e.target.value)}
                    />
                  ) : (
                    <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("settings.hooks.matchAll", "匹配所有")}</span>
                  )}
                  <button
                    type="button"
                    className="btn btn-ghost btn-icon"
                    style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)" }}
                    onClick={() => removeMatcherGroup(eventId, gi)}
                    title={t("settings.hooks.deleteMatcherGroup", "删除此匹配器组")}
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
                        {t(`settings.hooks.handler.${handler.type}`, HANDLER_LABELS[handler.type])}
                      </span>
                      <select
                        className="input"
                        style={{ ...inputStyle, width: 130, flexShrink: 0 }}
                        value={handler.type}
                        onChange={(e) => updateHandler(eventId, gi, hi, { type: e.target.value as HandlerType })}
                      >
                        {HANDLER_TYPES.map(ht => (
                          <option key={ht} value={ht}>{t(`settings.hooks.handler.${ht}`, HANDLER_LABELS[ht])}</option>
                        ))}
                      </select>
                      <button
                        type="button"
                        className="btn btn-ghost btn-icon"
                        style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)", marginLeft: "auto" }}
                        onClick={() => removeHandler(eventId, gi, hi)}
                        title={t("settings.hooks.deleteHandler", "删除此处理器")}
                      >
                        ×
                      </button>
                    </div>

                    {/* Command — textarea + shell selector on own row */}
                    {handler.type === "command" && (
                      <>
                        <FieldRow label={t("settings.hooks.fieldCommand", "命令")} icon={<SectionIcon name="bolt" size={13} />}>
                          <textarea
                            className="input"
                            style={{
                              flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0,
                              fontFamily: '"SF Mono", "Fira Code", monospace', lineHeight: 1.5,
                              minHeight: 56, resize: "vertical",
                            }}
                            placeholder={t("settings.hooks.commandPh", "命令或脚本路径，如 ./scripts/check.sh&#10;支持多行命令，每行独立执行")}
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
                          placeholder={t("settings.hooks.urlPh", "HTTP URL，如 http://localhost:8080/hooks/pre-tool-use")}
                          value={handler.url ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { url: e.target.value || undefined })}
                        />
                      </FieldRow>
                    )}
                    {/* MCP Tool — server + tool each on own row */}
                    {handler.type === "mcp_tool" && (
                      <>
                        <FieldRow label={t("settings.hooks.fieldServer", "服务器")} icon={<SectionIcon name="network" size={13} />}>
                          <input
                            className="input"
                            style={{ ...inputStyle, flex: 1 }}
                            placeholder={t("settings.hooks.serverPh", "MCP 服务器名称")}
                            value={handler.server ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { server: e.target.value || undefined })}
                          />
                        </FieldRow>
                        <FieldRow label={t("settings.hooks.fieldTool", "工具")} icon={<SectionIcon name="advanced" size={13} />}>
                          <input
                            className="input"
                            style={{ ...inputStyle, flex: 1 }}
                            placeholder={t("settings.hooks.toolPh", "工具名称")}
                            value={handler.tool ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { tool: e.target.value || undefined })}
                          />
                        </FieldRow>
                      </>
                    )}
                    {/* Prompt / Agent — textarea */}
                    {(handler.type === "prompt" || handler.type === "agent") && (
                      <FieldRow label={t("settings.hooks.fieldPrompt", "提示")} icon={<SectionIcon name="behavior" size={13} />}>
                        <textarea
                          className="input"
                          style={{
                            flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0,
                            fontFamily: '"SF Mono", "Fira Code", monospace', lineHeight: 1.5,
                            minHeight: 56, resize: "vertical",
                          }}
                          placeholder={t("settings.hooks.promptPh", "提示文本，用 $ARGUMENTS 插入 hook 输入数据&#10;支持多行提示内容")}
                          value={handler.prompt ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { prompt: e.target.value || undefined })}
                        />
                      </FieldRow>
                    )}

                    {/* ── Auxiliary options, each on its own row ── */}
                    {eventMeta?.hasMatcher && (
                      <FieldRow label={t("settings.hooks.fieldIf", "条件 if")} icon={<SectionIcon name="permissions" size={13} />}>
                        <input
                          className="input"
                          style={{ ...inputStyle, flex: 1, fontSize: F.hint }}
                          placeholder={t("settings.hooks.ifPh", "匹配条件，如 Bash(rm *)")}
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
                    <FieldRow label={t("settings.hooks.fieldTimeout", "超时")} icon={<SectionIcon name="status" size={13} />}>
                      <input
                        className="input"
                        style={{ ...inputStyle, width: 80, fontSize: F.hint }}
                        type="number"
                        placeholder="600"
                        value={handler.timeout ?? ""}
                        onChange={(e) => updateHandler(eventId, gi, hi, { timeout: e.target.value ? Number(e.target.value) : undefined })}
                      />
                      <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("settings.hooks.seconds", "秒")}</span>
                    </FieldRow>
                    {handler.type === "command" && (
                      <FieldRow label="async" icon={<SectionIcon name="ui" size={13} />}>
                        <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: F.hint, color: "var(--text-tertiary)", cursor: "pointer" }}>
                          <Toggle active={!!handler.async} onChange={(v) => updateHandler(eventId, gi, hi, { async: v || undefined })} />
                          {t("settings.hooks.asyncDesc", "后台运行（不阻塞主流程）")}
                        </label>
                      </FieldRow>
                    )}
                    <FieldRow label={t("settings.hooks.fieldStatus", "状态")} icon={<SectionIcon name="status" size={13} />}>
                      <input
                        className="input"
                        style={{ ...inputStyle, flex: 1, fontSize: F.hint }}
                        placeholder={t("settings.hooks.statusPh", "运行时显示的状态消息")}
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
                  {t("settings.hooks.addHandler", "+ 处理器")}
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
                {t("settings.hooks.addMatcherGroup", "+ 匹配器组")}
              </button>
            )}
          </div>
        );
      })}
    </Section>
  );
}

/** Hooks without Section wrapper — for tab content pane */
export function HooksSectionInline(props: {
  hooksValue: HooksConfig | undefined;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  // Reuse same logic but render flat — extract hooks data from props
  const { hooksValue, updateField, t } = props;
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
          <option value="">{t("settings.hooks.addEvent", "+ 添加 Hook 事件…")}</option>
          {HOOK_EVENTS.map(ev => (
            <option key={ev.id} value={ev.id}>{ev.id} — {t(`settings.hooks.event.${ev.id}.desc`, ev.desc)}</option>
          ))}
        </select>
      </div>

      {totalHooks === 0 && (
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", lineHeight: 1.5 }}>
          {t("settings.hooks.introLine1", "Hooks 在 Claude Code 生命周期的特定点自动执行命令/HTTP请求/LLM提示。")}
          <br />{t("settings.hooks.introLine2", "选择事件类型开始配置。")}
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
              {eventMeta && <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>— {t(`settings.hooks.event.${eventMeta.id}.desc`, eventMeta.desc)}</span>}
              <span style={{ fontSize: 12, fontWeight: 600, padding: "2px 10px", borderRadius: 10,
                background: "var(--accent-subtle)", color: "var(--accent)", marginLeft: "auto" }}>
                {count} handler{count !== 1 ? "s" : ""}
              </span>
              <button type="button" className="btn btn-ghost btn-icon"
                style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)" }}
                onClick={() => { const u = { ...hooks }; delete u[eventId]; syncHooks(u); }} title={t("action.delete", "删除")}>×
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
                    <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0, fontWeight: 500 }}>{t("settings.hooks.matcher", "匹配器")}</span>
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
                        placeholder={eventMeta?.id === "FileChanged" ? t("settings.hooks.matcherFilePh", "文件名，如 .envrc|.env") : t("settings.hooks.matcherToolPh", "工具名称或正则，多个用 | 分隔")}
                        value={group.matcher} onChange={(e) => updateMatcher(eventId, gi, e.target.value)} />
                    ) : (
                      <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("settings.hooks.matchAll", "匹配所有")}</span>
                    )}
                    <button type="button" className="btn btn-ghost btn-icon"
                      style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)" }}
                      onClick={() => removeMatcherGroup(eventId, gi)} title={t("action.delete", "删除")}>×
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
                          {t(`settings.hooks.handler.${handler.type}`, HANDLER_LABELS[handler.type])}
                        </span>
                        <select className="input" style={{ ...inputStyle, width: 130, flexShrink: 0 }}
                          value={handler.type} onChange={(e) => updateHandler(eventId, gi, hi, { type: e.target.value as HandlerType })}>
                          {HANDLER_TYPES.map(ht => <option key={ht} value={ht}>{t(`settings.hooks.handler.${ht}`, HANDLER_LABELS[ht])}</option>)}
                        </select>
                        <button type="button" className="btn btn-ghost btn-icon"
                          style={{ width: 26, height: 26, minWidth: 26, fontSize: 14, padding: 0, color: "var(--text-tertiary)", marginLeft: "auto" }}
                          onClick={() => removeHandler(eventId, gi, hi)} title={t("action.delete", "删除")}>×
                        </button>
                      </div>

                      {handler.type === "command" && (
                        <>
                          <FieldRow label={t("settings.hooks.fieldCommand", "命令")} icon={<SectionIcon name="bolt" size={13} />}>
                            <textarea
                              className="input"
                              style={{
                                flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0,
                                fontFamily: '"SF Mono", "Fira Code", monospace', lineHeight: 1.5,
                                minHeight: 56, resize: "vertical",
                              }}
                              placeholder={t("settings.hooks.commandPh", "命令或脚本路径，如 ./scripts/check.sh&#10;支持多行命令，每行独立执行")}
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
                          <input className="input" style={{ ...inputStyle, flex: 1 }} placeholder={t("settings.hooks.urlPh", "HTTP URL，如 http://localhost:8080/hooks/pre-tool-use")}
                            value={handler.url ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { url: e.target.value || undefined })} />
                        </FieldRow>
                      )}
                      {handler.type === "mcp_tool" && (
                        <>
                          <FieldRow label={t("settings.hooks.fieldServer", "服务器")} icon={<SectionIcon name="network" size={13} />}>
                            <input className="input" style={{ ...inputStyle, flex: 1 }} placeholder={t("settings.hooks.serverPh", "MCP 服务器名称")}
                              value={handler.server ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { server: e.target.value || undefined })} />
                          </FieldRow>
                          <FieldRow label={t("settings.hooks.fieldTool", "工具")} icon={<SectionIcon name="advanced" size={13} />}>
                            <input className="input" style={{ ...inputStyle, flex: 1 }} placeholder={t("settings.hooks.toolPh", "工具名称")}
                              value={handler.tool ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { tool: e.target.value || undefined })} />
                          </FieldRow>
                        </>
                      )}
                      {(handler.type === "prompt" || handler.type === "agent") && (
                        <FieldRow label={t("settings.hooks.fieldPrompt", "提示")} icon={<SectionIcon name="behavior" size={13} />}>
                          <textarea
                            className="input"
                            style={{
                              flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0,
                              fontFamily: '"SF Mono", "Fira Code", monospace', lineHeight: 1.5,
                              minHeight: 56, resize: "vertical",
                            }}
                            placeholder={t("settings.hooks.promptPh", "提示文本，用 $ARGUMENTS 插入 hook 输入数据&#10;支持多行提示内容")}
                            value={handler.prompt ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { prompt: e.target.value || undefined })}
                          />
                        </FieldRow>
                      )}

                      {eventMeta?.hasMatcher && (
                        <FieldRow label={t("settings.hooks.fieldIf", "条件 if")} icon={<SectionIcon name="permissions" size={13} />}>
                          <input className="input" style={{ ...inputStyle, flex: 1, fontSize: F.hint }} placeholder={t("settings.hooks.ifPh", "匹配条件，如 Bash(rm *)")}
                            value={handler["if"] ?? ""} onChange={(e) => {
                              const patch: Partial<HookHandler> = {};
                              if (e.target.value) (patch as any)["if"] = e.target.value;
                              else (patch as any)["if"] = undefined;
                              updateHandler(eventId, gi, hi, patch);
                            }} />
                        </FieldRow>
                      )}
                      <FieldRow label={t("settings.hooks.fieldTimeout", "超时")} icon={<SectionIcon name="status" size={13} />}>
                        <input className="input" style={{ ...inputStyle, width: 80, fontSize: F.hint }} type="number" placeholder="600"
                          value={handler.timeout ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { timeout: e.target.value ? Number(e.target.value) : undefined })} />
                        <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>{t("settings.hooks.seconds", "秒")}</span>
                      </FieldRow>
                      {handler.type === "command" && (
                        <FieldRow label="async" icon={<SectionIcon name="ui" size={13} />}>
                          <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: F.hint, color: "var(--text-tertiary)", cursor: "pointer" }}>
                            <Toggle active={!!handler.async} onChange={(v) => updateHandler(eventId, gi, hi, { async: v || undefined })} />
                            {t("settings.hooks.asyncDesc", "后台运行（不阻塞主流程）")}
                          </label>
                        </FieldRow>
                      )}
                      <FieldRow label={t("settings.hooks.fieldStatus", "状态")} icon={<SectionIcon name="status" size={13} />}>
                        <input className="input" style={{ ...inputStyle, flex: 1, fontSize: F.hint }}
                          placeholder={t("settings.hooks.statusPh", "运行时显示的状态消息")} value={handler.statusMessage ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { statusMessage: e.target.value || undefined })} />
                      </FieldRow>
                    </div>
                  ))}

                  <button type="button" className="btn btn-ghost"
                    style={{ fontSize: F.hint, padding: "6px 14px", alignSelf: "flex-start", marginLeft: 72 }}
                    onClick={() => addHandler(eventId, gi)}>{t("settings.hooks.addHandler", "+ 处理器")}</button>
                </div>
              );
            })}

            {isExpanded && (
              <button type="button" className="btn btn-ghost"
                style={{ fontSize: F.hint, padding: "6px 14px", alignSelf: "flex-start" }}
                onClick={() => addMatcherGroup(eventId)}>{t("settings.hooks.addMatcherGroup", "+ 匹配器组")}</button>
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
  const { t } = useTranslation();
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
    const timeoutId = setTimeout(async () => {
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
    setTimer(timeoutId);
  }, [timer]);

  const pick = async () => {
    try {
      const selected = await open({
        directory: pathType === "directory",
        multiple: false,
        title: pathType === "directory" ? t("settings.editor.chooseDir", "选择目录") : t("settings.editor.chooseFile", "选择文件"),
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
      return diffH === 0 ? t("settings.editor.justNow", "刚刚") : `${diffH}${t("settings.editor.hoursAgo", "小时前")}`;
    }
    if (diffDays < 30) return `${diffDays}${t("settings.editor.daysAgo", "天前")}`;
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4, position: "relative" }}>
      <div style={{ display: "flex", gap: 6 }}>
        <input
          className="input"
          style={{ flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0 }}
          placeholder={placeholder ?? (pathType === "directory" ? t("settings.editor.dirOrInputPh", "选择目录或直接输入路径…") : t("settings.editor.fileOrInputPh", "选择文件或直接输入路径…"))}
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
          title={pathType === "directory" ? t("settings.editor.chooseDir", "选择目录") : t("settings.editor.chooseFile", "选择文件")}
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
            ? t("settings.editor.dirHint", "输入 ~/ 浏览主目录，支持 Tab 补全")
            : t("settings.editor.fileHint", "输入路径浏览文件，如 ~/ 或 ./")}
        </span>
      )}
    </div>
  );
}

// ─── Field Renderer ────────────────────────────────────────

/** Order-insensitive deep equality for default-value comparison (R10). */
function stableEq(a: any, b: any): boolean {
  if (a === b) return true;
  if (a == null || b == null) return a === b;
  if (typeof a !== "object" || typeof b !== "object") return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a)) {
    if (a.length !== b.length) return false;
    return a.every((v, i) => stableEq(v, b[i]));
  }
  const ka = Object.keys(a), kb = Object.keys(b);
  if (ka.length !== kb.length) return false;
  return ka.every((k) => Object.prototype.hasOwnProperty.call(b, k) && stableEq(a[k], b[k]));
}

export function FieldRenderer({
  field,
  value,
  onChange,
  t,
  defaultValue,
  onReset,
  highlight,
}: {
  field: SettingField;
  value: any;
  onChange: (v: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
  /** R10: recommended default for this field, if any (undefined → no default known) */
  defaultValue?: any;
  /** R10: reset this field to defaultValue */
  onReset?: () => void;
  /** R8: search query to highlight within the field label/key */
  highlight?: string;
}) {
  // Shared left-right row style
  const rowStyle: React.CSSProperties = {
    display: "flex",
    alignItems: "flex-start",
    gap: 12,
  };

  // R10: a reset marker is shown only when a default exists AND the current
  // value diverges from it. Fields absent from RECOMMENDED_CONFIG have no default.
  const hasDefault = defaultValue !== undefined;
  const nonDefault = hasDefault && !stableEq(value, defaultValue);
  const label = (style?: React.CSSProperties) => (
    <FieldLabel field={field} t={t} style={style} nonDefault={nonDefault} onReset={onReset} highlight={highlight} />
  );

  switch (field.type) {
    case "boolean":
      return (
        <div style={{ ...rowStyle, alignItems: "center" }}>
          {label({ paddingTop: 0 })}
          <div style={{ flex: 1, minWidth: 0, display: "flex", justifyContent: "flex-end", paddingTop: 2 }}>
            <Toggle active={!!value} onChange={(v) => onChange(v || undefined)} />
          </div>
        </div>
      );

    case "select":
      return (
        <div style={rowStyle}>
          {label()}
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
          {label()}
          <div style={{ flex: 1, minWidth: 0 }}>
            <JsonEditor value={value} onChange={onChange} placeholder="{}" />
          </div>
        </div>
      );

    case "kv":
      return (
        <div style={rowStyle}>
          {label()}
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
          {label()}
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
            {label()}
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
          {label()}
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
