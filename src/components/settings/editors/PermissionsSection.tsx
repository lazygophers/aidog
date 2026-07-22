// ─── Permissions Section ───────────────────────────────────
// Extracted verbatim from editors.tsx (arch-redesign phase 3).

import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { IconClose, IconCheck } from "../../icons";
import { F, S } from "./tokens";
import { SectionIcon } from "./icons";
import { Section, FieldRow, JsonEditor } from "./_shared";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

type RuleMode = "allow" | "ask" | "deny";

const MODE_COLORS: Record<RuleMode, string> = {
  allow: "var(--color-success)",
  ask: "var(--color-warning)",
  deny: "var(--color-danger)",
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
    <Select
      
      value={mode}
      onValueChange={(v) => onChange(v as RuleMode)}
      
    >
<SelectTrigger style={{
        fontSize: F.small, fontWeight: 600, minWidth: 72,
        padding: "4px 8px", borderRadius: "var(--radius-sm)",
        background: `${MODE_COLORS[mode]}12`,
        color: MODE_COLORS[mode],
        border: `1px solid ${MODE_COLORS[mode]}35`,
        cursor: "pointer", outline: "none",
      }}><SelectValue/></SelectTrigger>
<SelectContent>
      {ALL_MODES.map(m => (
        <SelectItem key={m} value={m}>{modeLabel(m)}</SelectItem>
      ))}
    </SelectContent>
</Select>
  );

  const toolGroup = TOOL_GROUPS.find(g => g.tool === activeToolGroup) ?? TOOL_GROUPS[0];

  /** Segmented control: visual list editor ↔ raw JSON fallback */
  const ViewToggle = (
    <div style={{ display: "flex", justifyContent: "flex-end" }}>
      <div style={{ display: "inline-flex", gap: 2, padding: 2, background: "var(--bg-glass)", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)" }}>
        {(["visual", "json"] as const).map((m) => {
          const active = viewMode === m;
          return (
            <Button variant="outline" key={m} type="button"
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
            </Button>
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
        <Select
          
          
          value={perms.defaultMode ?? ""}
          onValueChange={(v) => updatePermKey("defaultMode", v || undefined)}
        >
<SelectTrigger style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}><SelectValue/></SelectTrigger>
<SelectContent>
          <SelectItem value="">—</SelectItem>
          {PERMISSION_MODES.map(m => (
            <SelectItem key={m.value} value={m.value}>{t(`settings.perm.mode_${m.value}`, m.desc)} — {t(`settings.perm.mode_${m.value}_desc`, m.hint)}</SelectItem>
          ))}
        </SelectContent>
</Select>
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
            <Button variant="outline" key={g.tool} type="button"
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
            </Button>
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
                <Input
                  
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
                <Button variant="ghost" type="button" 
                  style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
                  onClick={() => syncRules(rules.filter((_, j) => j !== rule.idx))}
                >
                  ×
                </Button>
              </div>
            ))}
          </div>
        );
      })()}

      {/* ── Add Rule ── */}
      <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
        <div style={{ position: "relative", flex: 1 }}>
          <Input
            
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
          <Button variant="ghost" type="button" 
            style={{
              position: "absolute", right: 2, top: "50%", transform: "translateY(-50%)",
              width: 24, height: 24, minWidth: 24, padding: 0,
              color: showTemplates ? "var(--accent)" : "var(--text-tertiary)",
            }}
            onClick={() => setShowTemplates(!showTemplates)}
            title={t("settings.perm.ruleTemplates", "规则模板")}
          >
            <SectionIcon name="bolt" size={14} />
          </Button>
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
                        <Button variant="ghost" key={ex} type="button" 
                          style={{
                            padding: "3px 8px", fontSize: 13, fontWeight: 400,
                            color: "var(--text-primary)", borderRadius: "var(--radius-sm)",
                            fontFamily: '"SF Mono", "Fira Code", monospace',
                          }}
                          onClick={() => { setDraftRule(ex); setShowTemplates(false); }}
                        >
                          {ex}
                        </Button>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
        <ModeSelect mode={draftMode} onChange={setDraftMode} />
        <Button variant="ghost" type="button" 
          style={{ fontSize: F.body, padding: S.btnPad, width: S.btnIcon, minWidth: S.btnIcon }}
          onClick={() => {
            if (draftRule.trim()) {
              syncRules([...rules, { pattern: draftRule.trim(), mode: draftMode }]);
              setDraftRule("");
            }
          }}
        >
          +
        </Button>
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
