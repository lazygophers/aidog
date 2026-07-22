// Extracted verbatim from editors.tsx (arch-redesign phase 3).

import React from "react";
import { useTranslation } from "react-i18next";
import { type SettingField } from "../../../services/claude-settings-schema";
import { F, S } from "./tokens";
import { Toggle, FieldLabel, JsonEditor, KvEditor, StringListEditor, PathInput } from "./_shared";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

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
          <Select
            
            
            value={value ?? ""}
            onValueChange={(v) => onChange(v || undefined)}
          >
<SelectTrigger style={{ fontSize: F.body, padding: S.inputPad, flex: 1, minWidth: 0 }}><SelectValue/></SelectTrigger>
<SelectContent>
            <SelectItem value="">—</SelectItem>
            {field.options?.map((opt) => (
              <SelectItem key={opt} value={opt}>
                {opt}
              </SelectItem>
            ))}
          </SelectContent>
</Select>
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
            <Input
              
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
