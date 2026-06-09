import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { settingsApi } from "../services/api";
import {
  SECTIONS,
  RECOMMENDED_CONFIG,
  type SettingField,
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

const PERMISSION_MODES: { value: string; desc: string }[] = [
  { value: "default", desc: "首次使用每个工具时提示" },
  { value: "acceptEdits", desc: "自动接受工作目录内的文件编辑" },
  { value: "plan", desc: "只读模式，不编辑源文件" },
  { value: "auto", desc: "自动批准，后台安全检查（预览）" },
  { value: "dontAsk", desc: "未预先批准的工具自动拒绝" },
  { value: "bypassPermissions", desc: "跳过所有权限提示" },
];

const TOOL_TEMPLATES: { tool: string; examples: string[] }[] = [
  { tool: "Bash", examples: ["Bash(npm run build)", "Bash(git commit *)", "Bash(docker *)"] },
  { tool: "Read", examples: ["Read(./.env)", "Read(//**/*.key)", "Read(~/.ssh/**)"] },
  { tool: "Edit", examples: ["Edit(/src/**/*.ts)", "Edit(./config.json)"] },
  { tool: "WebFetch", examples: ["WebFetch(domain:example.com)"] },
  { tool: "MCP", examples: ["mcp__puppeteer__*"] },
  { tool: "Agent", examples: ["Agent(Explore)", "Agent(Plan)"] },
];

function PermissionsSection({
  perms,
  updateField,
  t,
}: {
  perms: Record<string, string[]>;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const [draftRule, setDraftRule] = useState("");
  const [draftMode, setDraftMode] = useState<RuleMode>("allow");
  const [showTemplates, setShowTemplates] = useState(false);

  // Flatten allow/ask/deny into unified rule list
  const rules: { pattern: string; mode: RuleMode }[] = [
    ...(perms.allow ?? []).map(p => ({ pattern: p, mode: "allow" as RuleMode })),
    ...(perms.ask ?? []).map(p => ({ pattern: p, mode: "ask" as RuleMode })),
    ...(perms.deny ?? []).map(p => ({ pattern: p, mode: "deny" as RuleMode })),
  ];

  const syncRules = (updated: { pattern: string; mode: RuleMode }[]) => {
    const next: Record<string, any> = {};
    if (perms.defaultMode) next.defaultMode = perms.defaultMode;
    const allow = updated.filter(r => r.mode === "allow").map(r => r.pattern);
    const ask = updated.filter(r => r.mode === "ask").map(r => r.pattern);
    const deny = updated.filter(r => r.mode === "deny").map(r => r.pattern);
    if (allow.length) next.allow = allow;
    if (ask.length) next.ask = ask;
    if (deny.length) next.deny = deny;
    updateField("permissions", Object.keys(next).length > 0 ? next : undefined);
  };

  const modeLabel = (m: RuleMode) =>
    t(`settings.permissions${m.charAt(0).toUpperCase() + m.slice(1)}`);
  const modeIcon = (m: RuleMode) => m === "allow" ? "✓" : m === "ask" ? "?" : "✗";

  const RuleBadge = ({ mode, onClick }: { mode: RuleMode; onClick: () => void }) => (
    <span
      style={{
        display: "inline-flex", alignItems: "center", gap: 4,
        fontSize: 14, fontWeight: 600, width: 80, justifyContent: "center",
        padding: "6px 0", borderRadius: "var(--radius-sm)",
        background: `${MODE_COLORS[mode]}18`,
        color: MODE_COLORS[mode],
        cursor: "pointer",
        userSelect: "none",
      }}
      onClick={onClick}
    >
      {modeIcon(mode)} {modeLabel(mode)}
    </span>
  );

  // Detect tool name from rule pattern for grouping badge
  const toolBadge = (pattern: string) => {
    const m = pattern.match(/^([A-Za-z_]+|mcp__[a-z_]+)/);
    if (!m) return null;
    return m[1];
  };

  return (
    <Section title={t("settings.sectionPermissions")} defaultOpen>
      {/* Default Mode — left-right with descriptions */}
      <div style={{ display: "flex", alignItems: "flex-start", gap: 16 }}>
        <label style={{
          flexShrink: 0, width: 200, fontSize: F.label, fontWeight: 500,
          color: "var(--text-primary)", lineHeight: 1.5, paddingTop: 10,
        }}>
          {t("settings.permissionsDefaultMode")}
          <span style={{ display: "block", fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginTop: 3, fontFamily: '"SF Mono", "Fira Code", monospace' }}>
            permissions.defaultMode
          </span>
        </label>
        <div style={{ flex: 1, minWidth: 0 }}>
          <select
            className="input"
            style={{ fontSize: F.body, padding: S.inputPad, width: "100%" }}
            value={perms.defaultMode ?? ""}
            onChange={(e) => {
              const next: Record<string, any> = {};
              if (perms.allow?.length) next.allow = perms.allow;
              if (perms.ask?.length) next.ask = perms.ask;
              if (perms.deny?.length) next.deny = perms.deny;
              if (e.target.value) next.defaultMode = e.target.value;
              updateField("permissions", Object.keys(next).length > 0 ? next : undefined);
            }}
          >
            <option value="">—</option>
            {PERMISSION_MODES.map(m => (
              <option key={m.value} value={m.value}>{m.value} — {m.desc}</option>
            ))}
          </select>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: 6, lineHeight: 1.5 }}>
            规则优先级: deny → ask → allow。第一个匹配的规则生效。
          </div>
        </div>
      </div>

      {/* Existing rules */}
      {rules.length > 0 && (
        <div style={{ paddingLeft: 216, display: "flex", flexDirection: "column", gap: S.row }}>
          {rules.map((rule, i) => {
            const tool = toolBadge(rule.pattern);
            return (
              <div key={i} style={{ display: "flex", gap: 6, alignItems: "center" }}>
                {tool && (
                  <span style={{
                    fontSize: 12, fontWeight: 600, padding: "2px 8px", borderRadius: 4,
                    background: "var(--bg-glass)", color: "var(--accent)", flexShrink: 0,
                    border: "1px solid var(--border)",
                  }}>
                    {tool}
                  </span>
                )}
                <input
                  className="input"
                  style={{ flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0 }}
                  value={rule.pattern}
                  onChange={(e) => {
                    const updated = [...rules];
                    updated[i] = { ...updated[i], pattern: e.target.value };
                    syncRules(updated);
                  }}
                />
                <RuleBadge
                  mode={rule.mode}
                  onClick={() => {
                    const modes: RuleMode[] = ["allow", "ask", "deny"];
                    const updated = [...rules];
                    updated[i] = { ...updated[i], mode: modes[(modes.indexOf(rule.mode) + 1) % 3] };
                    syncRules(updated);
                  }}
                />
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
                  onClick={() => syncRules(rules.filter((_, j) => j !== i))}
                >
                  ×
                </button>
              </div>
            );
          })}
        </div>
      )}

      {/* Add rule */}
      <div style={{ paddingLeft: 216, display: "flex", gap: 6, alignItems: "center" }}>
        <div style={{ position: "relative", flex: 1 }}>
          <input
            className="input"
            style={{ fontSize: F.body, padding: S.inputPad, width: "100%", paddingRight: 28 }}
            placeholder="Bash(npm run *) 或 Edit(/src/**)"
            value={draftRule}
            onChange={(e) => setDraftRule(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && draftRule.trim()) {
                syncRules([...rules, { pattern: draftRule.trim(), mode: draftMode }]);
                setDraftRule("");
              }
            }}
          />
          <button
            type="button"
            className="btn btn-ghost btn-icon"
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
              <div
                className="glass-elevated"
                style={{
                  position: "absolute", top: "100%", left: 0, right: 0,
                  marginTop: 4, maxHeight: 260, overflowY: "auto",
                  zIndex: 100, padding: 8, animation: "fadeIn 150ms ease both",
                }}
              >
                {TOOL_TEMPLATES.map(group => (
                  <div key={group.tool} style={{ marginBottom: 8 }}>
                    <div style={{ fontSize: 13, fontWeight: 600, color: "var(--accent)", marginBottom: 4 }}>
                      {group.tool}
                    </div>
                    {group.examples.map(ex => (
                      <button
                        key={ex}
                        type="button"
                        className="btn btn-ghost"
                        style={{
                          width: "100%", justifyContent: "flex-start",
                          padding: "5px 10px", fontSize: 14, fontWeight: 400,
                          color: "var(--text-primary)", borderRadius: "var(--radius-sm)",
                        }}
                        onClick={() => { setDraftRule(ex); setShowTemplates(false); }}
                      >
                        {ex}
                      </button>
                    ))}
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
        <RuleBadge
          mode={draftMode}
          onClick={() => {
            const modes: RuleMode[] = ["allow", "ask", "deny"];
            setDraftMode(modes[(modes.indexOf(draftMode) + 1) % 3]);
          }}
        />
        <button
          type="button"
          className="btn btn-ghost"
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
    </Section>
  );
}

/** Permissions without Section wrapper — for tab content pane */
function PermissionsSectionInline({ perms, updateField, t }: {
  perms: Record<string, string[]>;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  // Same logic as PermissionsSection but renders flat (no Section card)
  const [draftRule, setDraftRule] = useState("");
  const [draftMode, setDraftMode] = useState<RuleMode>("allow");
  const [showTemplates, setShowTemplates] = useState(false);

  const rules: { pattern: string; mode: RuleMode }[] = [
    ...(perms.allow ?? []).map(p => ({ pattern: p, mode: "allow" as RuleMode })),
    ...(perms.ask ?? []).map(p => ({ pattern: p, mode: "ask" as RuleMode })),
    ...(perms.deny ?? []).map(p => ({ pattern: p, mode: "deny" as RuleMode })),
  ];

  const syncRules = (updated: { pattern: string; mode: RuleMode }[]) => {
    const next: Record<string, any> = {};
    if (perms.defaultMode) next.defaultMode = perms.defaultMode;
    const allow = updated.filter(r => r.mode === "allow").map(r => r.pattern);
    const ask = updated.filter(r => r.mode === "ask").map(r => r.pattern);
    const deny = updated.filter(r => r.mode === "deny").map(r => r.pattern);
    if (allow.length) next.allow = allow;
    if (ask.length) next.ask = ask;
    if (deny.length) next.deny = deny;
    updateField("permissions", Object.keys(next).length > 0 ? next : undefined);
  };

  const modeLabel = (m: RuleMode) => t(`settings.permissions${m.charAt(0).toUpperCase() + m.slice(1)}`);
  const modeIcon = (m: RuleMode) => m === "allow" ? "✓" : m === "ask" ? "?" : "✗";

  const RuleBadge = ({ mode, onClick }: { mode: RuleMode; onClick: () => void }) => (
    <span
      style={{
        display: "inline-flex", alignItems: "center", gap: 4,
        fontSize: 14, fontWeight: 600, width: 80, justifyContent: "center",
        padding: "6px 0", borderRadius: "var(--radius-sm)",
        background: `${MODE_COLORS[mode]}18`, color: MODE_COLORS[mode],
        cursor: "pointer", userSelect: "none",
      }}
      onClick={onClick}
    >
      {modeIcon(mode)} {modeLabel(mode)}
    </span>
  );

  const toolBadge = (pattern: string) => {
    const m = pattern.match(/^([A-Za-z_]+|mcp__[a-z_]+)/);
    return m ? m[1] : null;
  };

  return (
    <>
      {/* Default Mode */}
      <div style={{ display: "flex", alignItems: "flex-start", gap: 16 }}>
        <label style={{
          flexShrink: 0, width: 200, fontSize: F.label, fontWeight: 500,
          color: "var(--text-primary)", lineHeight: 1.5, paddingTop: 10,
        }}>
          {t("settings.permissionsDefaultMode")}
          <span style={{ display: "block", fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginTop: 3, fontFamily: '"SF Mono", "Fira Code", monospace' }}>
            permissions.defaultMode
          </span>
        </label>
        <div style={{ flex: 1, minWidth: 0 }}>
          <select
            className="input"
            style={{ fontSize: F.body, padding: S.inputPad, width: "100%" }}
            value={perms.defaultMode ?? ""}
            onChange={(e) => {
              const next: Record<string, any> = {};
              if (perms.allow?.length) next.allow = perms.allow;
              if (perms.ask?.length) next.ask = perms.ask;
              if (perms.deny?.length) next.deny = perms.deny;
              if (e.target.value) next.defaultMode = e.target.value;
              updateField("permissions", Object.keys(next).length > 0 ? next : undefined);
            }}
          >
            <option value="">—</option>
            {PERMISSION_MODES.map(m => (
              <option key={m.value} value={m.value}>{m.value} — {m.desc}</option>
            ))}
          </select>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: 6, lineHeight: 1.5 }}>
            规则优先级: deny → ask → allow。第一个匹配的规则生效。
          </div>
        </div>
      </div>

      {/* Rules list */}
      {rules.length > 0 && (
        <div style={{ paddingLeft: 216, display: "flex", flexDirection: "column", gap: S.row }}>
          {rules.map((rule, i) => {
            const tool = toolBadge(rule.pattern);
            return (
              <div key={i} style={{ display: "flex", gap: 6, alignItems: "center" }}>
                {tool && (
                  <span style={{
                    fontSize: 12, fontWeight: 600, padding: "2px 8px", borderRadius: 4,
                    background: "var(--bg-glass)", color: "var(--accent)", flexShrink: 0,
                    border: "1px solid var(--border)",
                  }}>
                    {tool}
                  </span>
                )}
                <input
                  className="input"
                  style={{ flex: 1, fontSize: F.body, padding: S.inputPad, minWidth: 0 }}
                  value={rule.pattern}
                  onChange={(e) => {
                    const updated = [...rules];
                    updated[i] = { ...updated[i], pattern: e.target.value };
                    syncRules(updated);
                  }}
                />
                <RuleBadge mode={rule.mode} onClick={() => {
                  const modes: RuleMode[] = ["allow", "ask", "deny"];
                  const updated = [...rules];
                  updated[i] = { ...updated[i], mode: modes[(modes.indexOf(rule.mode) + 1) % 3] };
                  syncRules(updated);
                }} />
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
                  onClick={() => syncRules(rules.filter((_, j) => j !== i))}
                >
                  ×
                </button>
              </div>
            );
          })}
        </div>
      )}

      {/* Add rule */}
      <div style={{ paddingLeft: 216, display: "flex", gap: 6, alignItems: "center" }}>
        <div style={{ position: "relative", flex: 1 }}>
          <input
            className="input"
            style={{ fontSize: F.body, padding: S.inputPad, width: "100%", paddingRight: 28 }}
            placeholder="Bash(npm run *) 或 Edit(/src/**)"
            value={draftRule}
            onChange={(e) => setDraftRule(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && draftRule.trim()) {
                syncRules([...rules, { pattern: draftRule.trim(), mode: draftMode }]);
                setDraftRule("");
              }
            }}
          />
          <button
            type="button"
            className="btn btn-ghost btn-icon"
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
              <div className="glass-elevated" style={{
                position: "absolute", top: "100%", left: 0, right: 0,
                marginTop: 4, maxHeight: 260, overflowY: "auto",
                zIndex: 100, padding: 8, animation: "fadeIn 150ms ease both",
              }}>
                {TOOL_TEMPLATES.map(group => (
                  <div key={group.tool} style={{ marginBottom: 8 }}>
                    <div style={{ fontSize: 13, fontWeight: 600, color: "var(--accent)", marginBottom: 4 }}>
                      {group.tool}
                    </div>
                    {group.examples.map(ex => (
                      <button key={ex} type="button" className="btn btn-ghost" style={{
                        width: "100%", justifyContent: "flex-start",
                        padding: "5px 10px", fontSize: 14, fontWeight: 400,
                        color: "var(--text-primary)", borderRadius: "var(--radius-sm)",
                      }} onClick={() => { setDraftRule(ex); setShowTemplates(false); }}>
                        {ex}
                      </button>
                    ))}
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
        <RuleBadge mode={draftMode} onClick={() => {
          const modes: RuleMode[] = ["allow", "ask", "deny"];
          setDraftMode(modes[(modes.indexOf(draftMode) + 1) % 3]);
        }} />
        <button type="button" className="btn btn-ghost" style={{ fontSize: F.body, padding: S.btnPad, width: S.btnIcon, minWidth: S.btnIcon }} onClick={() => {
          if (draftRule.trim()) { syncRules([...rules, { pattern: draftRule.trim(), mode: draftMode }]); setDraftRule(""); }
        }}>
          +
        </button>
      </div>
    </>
  );
}

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

                    {/* Main field: command / URL / prompt (full width, prominent) */}
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      {handler.type === "command" && (
                        <>
                          <div style={{ flex: 1, minWidth: 0 }}>
                            <PathInput
                              value={handler.command}
                              onChange={(v) => updateHandler(eventId, gi, hi, { command: v })}
                              pathType="file"
                              placeholder="命令或脚本路径，如 ./scripts/check.sh"
                            />
                          </div>
                          <select
                            className="input"
                            style={{ ...inputStyle, width: 100, flexShrink: 0 }}
                            value={handler.shell ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { shell: e.target.value || undefined })}
                          >
                            <option value="">bash</option>
                            <option value="powershell">powershell</option>
                          </select>
                        </>
                      )}
                      {handler.type === "http" && (
                        <input
                          className="input"
                          style={{ ...inputStyle, flex: 1 }}
                          placeholder="HTTP URL，如 http://localhost:8080/hooks/pre-tool-use"
                          value={handler.url ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { url: e.target.value || undefined })}
                        />
                      )}
                      {handler.type === "mcp_tool" && (
                        <>
                          <input
                            className="input"
                            style={{ ...inputStyle, flex: 1 }}
                            placeholder="MCP 服务器名称"
                            value={handler.server ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { server: e.target.value || undefined })}
                          />
                          <input
                            className="input"
                            style={{ ...inputStyle, flex: 1 }}
                            placeholder="工具名称"
                            value={handler.tool ?? ""}
                            onChange={(e) => updateHandler(eventId, gi, hi, { tool: e.target.value || undefined })}
                          />
                        </>
                      )}
                      {(handler.type === "prompt" || handler.type === "agent") && (
                        <input
                          className="input"
                          style={{ ...inputStyle, flex: 1 }}
                          placeholder="提示文本，用 $ARGUMENTS 插入 hook 输入数据"
                          value={handler.prompt ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { prompt: e.target.value || undefined })}
                        />
                      )}
                    </div>

                    {/* Auxiliary options row (subtle, smaller) */}
                    <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap", paddingTop: 2 }}>
                      {eventMeta?.hasMatcher && (
                        <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0 }}>条件 if</span>
                          <input
                            className="input"
                            style={{ ...inputStyle, width: 160, fontSize: F.hint }}
                            placeholder="Bash(rm *)"
                            value={handler["if"] ?? ""}
                            onChange={(e) => {
                              const patch: Partial<HookHandler> = {};
                              if (e.target.value) (patch as any)["if"] = e.target.value;
                              else (patch as any)["if"] = undefined;
                              updateHandler(eventId, gi, hi, patch);
                            }}
                          />
                        </div>
                      )}
                      <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                        <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0 }}>超时</span>
                        <input
                          className="input"
                          style={{ ...inputStyle, width: 64, fontSize: F.hint }}
                          type="number"
                          placeholder="600"
                          value={handler.timeout ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { timeout: e.target.value ? Number(e.target.value) : undefined })}
                        />
                        <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>秒</span>
                      </div>
                      {handler.type === "command" && (
                        <label style={{ display: "flex", alignItems: "center", gap: 5, fontSize: F.hint, color: "var(--text-tertiary)", cursor: "pointer" }}>
                          <Toggle active={!!handler.async} onChange={(v) => updateHandler(eventId, gi, hi, { async: v || undefined })} />
                          后台运行 (async)
                        </label>
                      )}
                      <input
                        className="input"
                        style={{ ...inputStyle, flex: "1 1 180px", minWidth: 140, fontSize: F.hint }}
                        placeholder="状态消息 (运行时显示)"
                        value={handler.statusMessage ?? ""}
                        onChange={(e) => updateHandler(eventId, gi, hi, { statusMessage: e.target.value || undefined })}
                      />
                    </div>
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

                      {/* Main field */}
                      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                        {handler.type === "command" && (
                          <>
                            <div style={{ flex: 1, minWidth: 0 }}>
                              <PathInput value={handler.command} onChange={(v) => updateHandler(eventId, gi, hi, { command: v })} pathType="file" placeholder="命令或脚本路径，如 ./scripts/check.sh" />
                            </div>
                            <select className="input" style={{ ...inputStyle, width: 100, flexShrink: 0 }}
                              value={handler.shell ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { shell: e.target.value || undefined })}>
                              <option value="">bash</option><option value="powershell">powershell</option>
                            </select>
                          </>
                        )}
                        {handler.type === "http" && (
                          <input className="input" style={{ ...inputStyle, flex: 1 }} placeholder="HTTP URL，如 http://localhost:8080/hooks/pre-tool-use"
                            value={handler.url ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { url: e.target.value || undefined })} />
                        )}
                        {handler.type === "mcp_tool" && (
                          <>
                            <input className="input" style={{ ...inputStyle, flex: 1 }} placeholder="MCP 服务器名称"
                              value={handler.server ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { server: e.target.value || undefined })} />
                            <input className="input" style={{ ...inputStyle, flex: 1 }} placeholder="工具名称"
                              value={handler.tool ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { tool: e.target.value || undefined })} />
                          </>
                        )}
                        {(handler.type === "prompt" || handler.type === "agent") && (
                          <input className="input" style={{ ...inputStyle, flex: 1 }} placeholder="提示文本，用 $ARGUMENTS 插入 hook 输入数据"
                            value={handler.prompt ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { prompt: e.target.value || undefined })} />
                        )}
                      </div>

                      {/* Auxiliary options */}
                      <div style={{ display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap", paddingTop: 2 }}>
                        {eventMeta?.hasMatcher && (
                          <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                            <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0 }}>条件 if</span>
                            <input className="input" style={{ ...inputStyle, width: 160, fontSize: F.hint }} placeholder="Bash(rm *)"
                              value={handler["if"] ?? ""} onChange={(e) => {
                                const patch: Partial<HookHandler> = {};
                                if (e.target.value) (patch as any)["if"] = e.target.value;
                                else (patch as any)["if"] = undefined;
                                updateHandler(eventId, gi, hi, patch);
                              }} />
                          </div>
                        )}
                        <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0 }}>超时</span>
                          <input className="input" style={{ ...inputStyle, width: 64, fontSize: F.hint }} type="number" placeholder="600"
                            value={handler.timeout ?? ""} onChange={(e) => updateHandler(eventId, gi, hi, { timeout: e.target.value ? Number(e.target.value) : undefined })} />
                          <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>秒</span>
                        </div>
                        {handler.type === "command" && (
                          <label style={{ display: "flex", alignItems: "center", gap: 5, fontSize: F.hint, color: "var(--text-tertiary)", cursor: "pointer" }}>
                            <Toggle active={!!handler.async} onChange={(v) => updateHandler(eventId, gi, hi, { async: v || undefined })} />
                            后台运行 (async)
                          </label>
                        )}
                        <input className="input" style={{ ...inputStyle, flex: "1 1 180px", minWidth: 140, fontSize: F.hint }}
                          placeholder="状态消息 (运行时显示)" value={handler.statusMessage ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { statusMessage: e.target.value || undefined })} />
                      </div>
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

  // Active section tab
  const [activeTab, setActiveTab] = useState("core");

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
        <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
          <div style={{ display: "flex", alignItems: "flex-start", gap: 16 }}>
            <label style={{
              flexShrink: 0, width: 200, fontSize: F.label, fontWeight: 500,
              color: "var(--text-primary)", lineHeight: 1.5, paddingTop: 10,
            }}>
              {t("settings.f_env", "Environment Variables")}
              <span style={{ display: "block", fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginTop: 3, fontFamily: '"SF Mono", "Fira Code", monospace' }}>
                env
              </span>
            </label>
            <div style={{ flex: 1, minWidth: 0 }}>
              <KvEditor
                items={(config.env ?? {}) as Record<string, string>}
                onChange={(newEnv) =>
                  updateField("env", Object.keys(newEnv).length > 0 ? newEnv : undefined)
                }
              />
            </div>
          </div>
        </div>
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
      </div>
    );
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", width: "100%" }}>
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
          style={{ flex: 1, padding: S.pad, borderRadius: "var(--radius-lg)", overflow: "auto" }}
        >
          <textarea
            className="input"
            style={{
              fontFamily: '"SF Mono", "Fira Code", monospace',
              fontSize: F.body,
              lineHeight: 1.7,
              minHeight: "100%",
              resize: "vertical",
              whiteSpace: "pre",
              padding: S.inputPad,
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

      {/* GUI mode — sidebar tabs + content */}
      {mode === "gui" && (
        <div style={{ display: "flex", gap: 20, flex: 1, minHeight: 0 }}>
          {/* Sidebar */}
          <nav
            style={{
              width: 200,
              flexShrink: 0,
              display: "flex",
              flexDirection: "column",
              gap: 2,
              overflowY: "auto",
              paddingRight: 4,
            }}
          >
            {SECTIONS.map((section) => {
              const visibleFields = section.fields.filter((f) => !f.skipGui);
              if (visibleFields.length === 0 && section.id !== "hooks") return null;
              const isActive = activeTab === section.id;
              return (
                <button
                  key={section.id}
                  type="button"
                  className="btn btn-ghost"
                  style={{
                    justifyContent: "flex-start",
                    gap: 10,
                    padding: "10px 14px",
                    fontSize: F.body,
                    fontWeight: isActive ? 600 : 400,
                    color: isActive ? "var(--accent)" : "var(--text-secondary)",
                    background: isActive ? "var(--accent-subtle)" : "transparent",
                    borderRadius: "var(--radius-sm)",
                    textAlign: "left",
                    transition: "all 150ms",
                    border: "none",
                  }}
                  onClick={() => setActiveTab(section.id)}
                >
                  <span style={{ fontSize: 16, flexShrink: 0, display: "flex", alignItems: "center" }}><SectionIcon name={section.id} size={16} /></span>
                  <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {t(section.labelKey)}
                  </span>
                </button>
              );
            })}
          </nav>

          {/* Content pane */}
          <div
            className="glass-surface"
            style={{
              flex: 1,
              minWidth: 0,
              padding: S.pad,
              borderRadius: "var(--radius-lg)",
              overflowY: "auto",
            }}
          >
            {(() => {
              const section = SECTIONS.find((s) => s.id === activeTab);
              if (!section) return null;

              // Section heading inside content pane
              const heading = (
                <div style={{ marginBottom: S.gap + 4 }}>
                  <div style={{ fontSize: F.title, fontWeight: 600, color: "var(--text-primary)", letterSpacing: "-0.01em" }}>
                    <SectionIcon name={section.id} size={20} style={{ marginRight: 8, verticalAlign: "middle" }} />{t(section.labelKey)}
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

              if (section.id === "hooks") {
                return (
                  <div>
                    {heading}
                    <HooksSectionInline hooksValue={config.hooks as HooksConfig | undefined} updateField={updateField} t={t} />
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
          </div>
        </div>
      )}
    </div>
  );
}
