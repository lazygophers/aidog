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

/** Label cell for left-right layout */
function FieldLabel({ field, t, style }: { field: SettingField; t: ReturnType<typeof useTranslation>["t"]; style?: React.CSSProperties }) {
  const translated = t(`settings.f_${field.key}`, field.label);
  return (
    <label
      style={{
        flexShrink: 0,
        width: 180,
        fontSize: F.label,
        fontWeight: 500,
        color: "var(--text-secondary)",
        lineHeight: 1.4,
        paddingTop: 8,
        ...style,
      }}
    >
      {translated}
      <span style={{ display: "block", fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginTop: 2 }}>
        {field.key}
      </span>
      {field.description && (
        <span style={{ display: "block", fontWeight: 400, fontSize: F.hint, color: "var(--text-tertiary)", marginTop: 2, lineHeight: 1.4 }}>
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
      <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
        <label style={{
          flexShrink: 0, width: 180, fontSize: F.label, fontWeight: 500,
          color: "var(--text-secondary)", lineHeight: 1.4, paddingTop: 8,
        }}>
          {t("settings.permissionsDefaultMode")}
          <span style={{ display: "block", fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginTop: 2 }}>
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
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginTop: 4, lineHeight: 1.5 }}>
            规则优先级: deny → ask → allow。第一个匹配的规则生效。
          </div>
        </div>
      </div>

      {/* Existing rules */}
      {rules.length > 0 && (
        <div style={{ paddingLeft: 192, display: "flex", flexDirection: "column", gap: S.row }}>
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
      <div style={{ paddingLeft: 192, display: "flex", gap: 6, alignItems: "center" }}>
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
            ⚡
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

// ─── Hooks Section (friendly editor) ────────────────────────

const HOOK_EVENTS: { id: string; label: string; desc: string; hasMatcher: boolean; matcherHint: string }[] = [
  { id: "SessionStart", label: "Session Start", desc: "会话启动或恢复时", hasMatcher: true, matcherHint: "startup | resume | clear | compact" },
  { id: "UserPromptSubmit", label: "Prompt Submit", desc: "用户提交提示时", hasMatcher: false, matcherHint: "" },
  { id: "PreToolUse", label: "Pre Tool Use", desc: "工具调用前，可阻止", hasMatcher: true, matcherHint: "Bash | Edit | Write | Read | mcp__*" },
  { id: "PostToolUse", label: "Post Tool Use", desc: "工具调用成功后", hasMatcher: true, matcherHint: "Bash | Edit | Write | mcp__*" },
  { id: "Notification", label: "Notification", desc: "发送通知时", hasMatcher: true, matcherHint: "permission_prompt | idle_prompt | auth_success" },
  { id: "Stop", label: "Stop", desc: "Claude 完成响应时", hasMatcher: false, matcherHint: "" },
  { id: "SubagentStop", label: "Subagent Stop", desc: "子代理完成时", hasMatcher: true, matcherHint: "Explore | Plan | general-purpose" },
  { id: "ConfigChange", label: "Config Change", desc: "配置文件变更时", hasMatcher: true, matcherHint: "user_settings | project_settings | local_settings" },
  { id: "FileChanged", label: "File Changed", desc: "监视文件变更时", hasMatcher: true, matcherHint: ".envrc|.env (文字文件名)" },
  { id: "CwdChanged", label: "CWD Changed", desc: "工作目录切换时", hasMatcher: false, matcherHint: "" },
  { id: "PreCompact", label: "Pre Compact", desc: "上下文压缩前", hasMatcher: true, matcherHint: "manual | auto" },
  { id: "SessionEnd", label: "Session End", desc: "会话结束时", hasMatcher: true, matcherHint: "clear | resume | logout | other" },
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
            className="glass-surface"
            style={{ padding: 12, display: "flex", flexDirection: "column", gap: 10 }}
          >
            {/* Event header */}
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <span
                style={{ cursor: "pointer", userSelect: "none", fontSize: F.small, color: "var(--text-tertiary)",
                  transition: "transform 0.15s", transform: isExpanded ? "rotate(90deg)" : "rotate(0deg)"
                }}
                onClick={() => setExpandedEvent(isExpanded ? null : eventId)}
              >
                ▶
              </span>
              <span style={{ fontSize: F.body, fontWeight: 600, color: "var(--accent)" }}>
                {eventId}
              </span>
              {eventMeta && (
                <span style={{ fontSize: F.hint, color: "var(--text-tertiary)" }}>
                  — {eventMeta.desc}
                </span>
              )}
              <span style={{
                fontSize: 12, fontWeight: 600, padding: "1px 8px", borderRadius: 10,
                background: "var(--accent-subtle)", color: "var(--accent)", marginLeft: "auto",
              }}>
                {count}
              </span>
              <button
                type="button"
                className="btn btn-ghost btn-icon"
                style={{ width: 22, height: 22, minWidth: 22, fontSize: 13, padding: 0, color: "var(--text-tertiary)" }}
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
            {isExpanded && groups.map((group, gi) => (
              <div
                key={gi}
                style={{
                  borderLeft: "3px solid var(--accent)",
                  paddingLeft: 12,
                  display: "flex",
                  flexDirection: "column",
                  gap: 8,
                }}
              >
                {/* Matcher input */}
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0, width: 60 }}>
                    Matcher
                  </span>
                  <input
                    className="input"
                    style={{ ...inputStyle, flex: 1 }}
                    placeholder={eventMeta?.matcherHint ?? "留空匹配所有"}
                    value={group.matcher}
                    onChange={(e) => updateMatcher(eventId, gi, e.target.value)}
                  />
                  <button
                    type="button"
                    className="btn btn-ghost btn-icon"
                    style={{ width: 22, height: 22, minWidth: 22, fontSize: 13, padding: 0, color: "var(--text-tertiary)" }}
                    onClick={() => removeMatcherGroup(eventId, gi)}
                    title="删除此匹配器组"
                  >
                    ×
                  </button>
                </div>

                {/* Handlers */}
                {group.hooks.map((handler, hi) => (
                  <div key={hi} style={{ display: "flex", flexDirection: "column", gap: 6, paddingLeft: 66 }}>
                    {/* Handler type selector + delete */}
                    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                      <select
                        className="input"
                        style={{ ...inputStyle, width: 110, flexShrink: 0 }}
                        value={handler.type}
                        onChange={(e) => updateHandler(eventId, gi, hi, { type: e.target.value as HandlerType })}
                      >
                        {HANDLER_TYPES.map(ht => (
                          <option key={ht} value={ht}>{HANDLER_LABELS[ht]}</option>
                        ))}
                      </select>

                      {/* Common: timeout */}
                      <input
                        className="input"
                        style={{ ...inputStyle, width: 80, flexShrink: 0 }}
                        type="number"
                        placeholder="超时(秒)"
                        value={handler.timeout ?? ""}
                        onChange={(e) => updateHandler(eventId, gi, hi, { timeout: e.target.value ? Number(e.target.value) : undefined })}
                      />

                      {/* Command-specific: async toggle */}
                      {handler.type === "command" && (
                        <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: F.hint, color: "var(--text-tertiary)", flexShrink: 0, cursor: "pointer" }}>
                          <Toggle active={!!handler.async} onChange={(v) => updateHandler(eventId, gi, hi, { async: v || undefined })} />
                          async
                        </label>
                      )}

                      {/* if condition (tool events only) */}
                      {eventMeta?.hasMatcher && (
                        <input
                          className="input"
                          style={{ ...inputStyle, width: 140, flexShrink: 0 }}
                          placeholder="if: Bash(rm *)"
                          value={handler["if"] ?? ""}
                          onChange={(e) => {
                            const patch: Partial<HookHandler> = {};
                            if (e.target.value) (patch as any)["if"] = e.target.value;
                            else (patch as any)["if"] = undefined;
                            updateHandler(eventId, gi, hi, patch);
                          }}
                        />
                      )}

                      <button
                        type="button"
                        className="btn btn-ghost btn-icon"
                        style={{ width: 22, height: 22, minWidth: 22, fontSize: 13, padding: 0, color: "var(--text-tertiary)", marginLeft: "auto" }}
                        onClick={() => removeHandler(eventId, gi, hi)}
                        title="删除此处理器"
                      >
                        ×
                      </button>
                    </div>

                    {/* Type-specific fields */}
                    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                      {handler.type === "command" && (
                        <>
                          <PathInput
                            value={handler.command}
                            onChange={(v) => updateHandler(eventId, gi, hi, { command: v })}
                            pathType="file"
                            placeholder="命令或脚本路径，如 ./scripts/check.sh"
                          />
                          <select
                            className="input"
                            style={{ ...inputStyle, width: 90, flexShrink: 0 }}
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
                          placeholder="http://localhost:8080/hooks/pre-tool-use"
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
                          placeholder="提示文本，用 $ARGUMENTS 插入 hook 输入"
                          value={handler.prompt ?? ""}
                          onChange={(e) => updateHandler(eventId, gi, hi, { prompt: e.target.value || undefined })}
                        />
                      )}
                    </div>

                    {/* Status message (optional, all types) */}
                    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                      <input
                        className="input"
                        style={{ ...inputStyle, flex: 1 }}
                        placeholder="statusMessage (可选，hook 运行时显示的消息)"
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
                  style={{ fontSize: F.hint, padding: "4px 10px", alignSelf: "flex-start", marginLeft: 66 }}
                  onClick={() => addHandler(eventId, gi)}
                >
                  + 处理器
                </button>
              </div>
            ))}

            {/* Add matcher group to existing event */}
            {isExpanded && (
              <button
                type="button"
                className="btn btn-ghost"
                style={{ fontSize: F.hint, padding: "4px 10px", alignSelf: "flex-start" }}
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
          placeholder={placeholder ?? (pathType === "directory" ? "点击 📁 选择或直接输入路径…" : "点击 📁 选择或直接输入文件路径…")}
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
          📁
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
                  {s.is_dir ? "📁" : "📄"}
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
          <div style={{ flex: 1, minWidth: 0, display: "flex", justifyContent: "flex-end" }}>
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
              // Special handling for permissions section: unified rule manager
              if (section.id === "permissions") {
                return (
                  <PermissionsSection
                    key={section.id}
                    perms={perms}
                    updateField={updateField}
                    t={t}
                  />
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
                    <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
                      <label style={{
                        flexShrink: 0, width: 180, fontSize: F.label, fontWeight: 500,
                        color: "var(--text-secondary)", lineHeight: 1.4, paddingTop: 8,
                      }}>
                        {t("settings.f_env", "Environment Variables")}
                        <span style={{ display: "block", fontSize: F.hint, color: "var(--text-tertiary)", fontWeight: 400, marginTop: 2 }}>
                          env
                        </span>
                      </label>
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <KvEditor
                          items={(config.env ?? {}) as Record<string, string>}
                          onChange={(newEnv) =>
                            updateField(
                              "env",
                              Object.keys(newEnv).length > 0 ? newEnv : undefined,
                            )
                          }
                        />
                      </div>
                    </div>
                  </Section>
                );
              }

              // Special handling for hooks: friendly editor
              if (section.id === "hooks") {
                return (
                  <HooksSection
                    key={section.id}
                    hooksValue={config.hooks as HooksConfig | undefined}
                    updateField={updateField}
                    t={t}
                  />
                );
              }

              // Default: render each field in section
              return (
                <Section
                  key={section.id}
                  title={t(section.labelKey)}
                  defaultOpen={section.id === "core"}
                >
                  {section.fields
                    .filter((field) => !field.skipGui)
                    .map((field) => (
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
