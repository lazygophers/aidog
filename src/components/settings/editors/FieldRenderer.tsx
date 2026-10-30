// Extracted verbatim from editors.tsx (arch-redesign phase 3).

import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { type SettingField } from "../../../services/claude-settings-schema";
import { F, S } from "./tokens";
import { Toggle, FieldLabel, JsonEditor, KvEditor, KvSelectEditor, ObjectEditor, StringListEditor, PathInput } from "./_shared";
import { Input } from "@/components/ui/input";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
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

/** 可输入 + 总是列出全部候选的下拉（不按输入过滤）；值不在列表里照常保存。 */
function OptionCombobox({ id, value, onChange, options, placeholder }: {
  id: string;
  value: string | undefined;
  onChange: (v: string | undefined) => void;
  options: string[];
  placeholder?: string;
}) {
  const [open, setOpen] = useState(false);
  const [hi, setHi] = useState(-1);
  const pick = (v: string) => { onChange(v || undefined); setOpen(false); };
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const d = e.key === "ArrowDown" ? 1 : -1;
      if (!open) { setOpen(true); setHi(Math.max(0, options.indexOf(value ?? ""))); return; }
      setHi((i) => (i + d + options.length) % options.length);
    } else if (e.key === "Enter" && open && hi >= 0) {
      e.preventDefault();
      pick(options[hi]);
    } else if (e.key === "Escape" && open) {
      e.stopPropagation();
      setOpen(false);
    }
  };
  return (
    <Popover open={open} onOpenChange={(o) => { if (!o) setOpen(false); }}>
      <PopoverAnchor asChild>
        <div style={{ position: "relative", width: "100%" }}>
          <Input
            id={`cb-${id}`}
            role="combobox"
            aria-expanded={open}
            aria-controls={`cb-list-${id}`}
            style={{ fontSize: F.body, padding: S.inputPad, width: "100%", paddingInlineEnd: 28 }}
            placeholder={placeholder}
            value={value ?? ""}
            onChange={(e) => onChange(e.target.value || undefined)}
            onFocus={() => setOpen(true)}
            onKeyDown={onKeyDown}
          />
          <button
            type="button"
            tabIndex={-1}
            data-testid={`cb-toggle-${id}`}
            // mousedown 阻止抢焦点，避免 input blur 后立刻重开
            onMouseDown={(e) => { e.preventDefault(); setOpen((o) => !o); }}
            style={{ position: "absolute", insetInlineEnd: 4, top: "50%", transform: "translateY(-50%)", color: "var(--text-tertiary)", cursor: "pointer", background: "transparent", border: 0 }}
          >
            ▾
          </button>
        </div>
      </PopoverAnchor>
      <PopoverContent
        id={`cb-list-${id}`}
        role="listbox"
        align="start"
        className="glass-elevated"
        style={{ width: "var(--radix-popover-trigger-width)", maxHeight: 240, overflowY: "auto", padding: 4 }}
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        {options.map((o, i) => {
          const selected = value === o;
          return (
            <div
              key={o}
              role="option"
              aria-selected={selected}
              data-testid={`cb-opt-${id}-${o}`}
              data-state={selected ? "selected" : i === hi ? "active" : "idle"}
              onMouseDown={(e) => { e.preventDefault(); pick(o); }}
              onMouseEnter={() => setHi(i)}
              style={{
                padding: "6px 12px", fontSize: F.body, cursor: "pointer", borderRadius: "var(--radius-sm)",
                fontWeight: selected ? 600 : 400,
                color: selected ? "var(--accent)" : "var(--text-primary)",
                background: selected || i === hi ? "var(--accent-subtle)" : "transparent",
              }}
            >
              {o}
            </div>
          );
        })}
      </PopoverContent>
    </Popover>
  );
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



            value={!value ? "__none__" : value}
            onValueChange={(v) => onChange(v === "__none__" ? undefined : v)}
          >
<SelectTrigger style={{ fontSize: F.body, padding: S.inputPad, flex: 1, minWidth: 0 }}><SelectValue/></SelectTrigger>
<SelectContent>
            <SelectItem value="__none__">—</SelectItem>
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
              keyPlaceholder={field.keyPlaceholder}
            />
          </div>
        </div>
      );

    case "kv-select":
      return (
        <div style={rowStyle}>
          {label()}
          <div style={{ flex: 1, minWidth: 0 }}>
            <KvSelectEditor
              items={(value && typeof value === "object" && !Array.isArray(value)) ? value as Record<string, string> : {}}
              onChange={(kv) => onChange(Object.keys(kv).length > 0 ? kv : undefined)}
              valueOptions={field.valueOptions ?? []}
              keyPlaceholder={field.keyPlaceholder}
            />
          </div>
        </div>
      );

    case "object":
      return (
        <div style={rowStyle}>
          {label()}
          <div style={{ flex: 1, minWidth: 0 }}>
            <ObjectEditor
              value={(value && typeof value === "object" && !Array.isArray(value)) ? value as Record<string, any> : {}}
              onChange={onChange}
              fields={field.objectFields ?? []}
              addLabel={t("settings.addRule")}
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
            {field.options?.length ? (
              <OptionCombobox
                id={field.key}
                value={value}
                onChange={onChange}
                options={field.options}
                placeholder={field.placeholder}
              />
            ) : (
              <Input
                style={{ fontSize: F.body, padding: S.inputPad, width: "100%" }}
                placeholder={field.placeholder}
                value={value ?? ""}
                onChange={(e) => onChange(e.target.value || undefined)}
              />
            )}
          </div>
        </div>
      );
  }
}
