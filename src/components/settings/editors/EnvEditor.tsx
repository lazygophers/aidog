// ─── Env Var Editor (structured) ────────────────────────────
// Extracted verbatim from editors.tsx (arch-redesign phase 3).

import { useState, useMemo, useCallback } from "react";
import {
  ENV_VAR_DEFS,
  ENV_VAR_GROUP_ORDER,
  type EnvVarDef,
} from "../../../services/claude-settings-schema";
import { F, S } from "./tokens";
import { SvgIcon } from "./icons";
import { Toggle } from "./_shared";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

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
    <Button variant="ghost" type="button" 
      style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.small, color: "var(--text-tertiary)" }}
      onClick={() => onChange(undefined)} title={t("action.remove", "Remove")}>×</Button>
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
            <Select  
              value={value ?? ""} onValueChange={(v) => onChange(v || undefined)}>
<SelectTrigger style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}><SelectValue/></SelectTrigger>
<SelectContent>
              <SelectItem value="">—</SelectItem>
              {opts.map((o) => <SelectItem key={o} value={o}>{o}</SelectItem>)}
            </SelectContent>
</Select>
            {isSet && removeBtn}
          </div>
        );
      }
      case "number":
        return (
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <Input  type="number"
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
            <Input  type={show ? "text" : "password"}
              style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
              placeholder={placeholder} value={value ?? ""}
              onChange={(e) => onChange(e.target.value || undefined)} />
            <Button variant="ghost" type="button" 
              style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon }}
              onClick={() => setShow(!show)}>
              <SvgIcon d={show
                ? "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8Z M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z"
                : "M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19M1 1l22 22"} size={14} />
            </Button>
            {isSet && removeBtn}
          </div>
        );
      }
      case "string":
      default:
        return (
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <Input  style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
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
          <Input  style={{ fontSize: F.body, padding: S.inputPad, paddingLeft: 32, width: "100%" }}
            placeholder={t("env.searchPlaceholder", "Search environment variables…")}
            value={search} onChange={(e) => setSearch(e.target.value)} />
          {search && (
            <Button variant="outline" type="button" style={{
              position: "absolute", right: 6, top: "50%", transform: "translateY(-50%)",
              background: "none", border: "none", cursor: "pointer", color: "var(--text-tertiary)", fontSize: 14,
            }} onClick={() => setSearch("")}>×</Button>
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
                <Input  style={{ fontSize: F.body, padding: S.inputPad }} value={k} readOnly />
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <Input  style={{ flex: 1, fontSize: F.body, padding: S.inputPad }} value={v}
                    onChange={(e) => updateEnv(k, e.target.value)} />
                  <Button variant="ghost" type="button" 
                    style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
                    onClick={() => updateEnv(k, undefined)}>×</Button>
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
            <Input  style={{ fontSize: F.body, padding: S.inputPad, width: 120 }}
              placeholder="KEY" value={customKey} onChange={(e) => setCustomKey(e.target.value)} />
            <Input  style={{ fontSize: F.body, padding: S.inputPad, width: 120 }}
              placeholder="VALUE" value={customVal} onChange={(e) => setCustomVal(e.target.value)} />
            <Button variant="ghost"  style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={() => {
                if (customKey.trim()) { updateEnv(customKey.trim(), customVal); setCustomKey(""); setCustomVal(""); }
              }}>
              {t("env.addCustom", "+ Custom")}
            </Button>
          </div>

          {/* Add known dropdown */}
          {addableDefs.length > 0 && (
            <div style={{ position: "relative" }}>
              <Button variant="ghost"  style={{ fontSize: F.body, padding: S.btnPad }}
                onClick={() => setShowAddMenu(!showAddMenu)}>
                {t("env.addKnown", "+ Add Known")}
              </Button>
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
                          <Button variant="outline" key={d.key} style={{
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
                          </Button>
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
