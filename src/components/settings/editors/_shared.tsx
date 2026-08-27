// ─── Shared UI primitives (cross-section) ─────────────────
// Extracted verbatim from editors.tsx (arch-redesign phase 3).
// Consumed by FieldRenderer / PermissionsSection / SandboxSection / HooksSectionInline.

import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { type SettingField, type ObjectSubField } from "../../../services/claude-settings-schema";
import { F, S } from "./tokens";
import { SectionIcon } from "./icons";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useReveal, makeRipple, JsonCodeEditor } from "../../shared";

/** Toggle switch — shadcn Switch under the hood, legacy `active`/`onChange` props kept for callers */
export function Toggle({
  active,
  onChange,
  disabled,
}: {
  active: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <Switch
      checked={active}
      onCheckedChange={onChange}
      disabled={disabled}
      className="shrink-0"
    />
  );
}

/** Collapsible section — own card */
export function Section({
  title,
  defaultOpen = false,
  children,
}: {
  title: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  // ponytail: reveal 一次 — Section 挂载即触发，进入视口淡入上移（萤火虫入场签名）。
  const { ref: revealRef, shown: revealShown } = useReveal<HTMLDivElement>(0);
  return (
    <div
      ref={revealRef}
      className={`glass-surface hover-lift${revealShown ? " reveal in" : " reveal"}`}
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
export function Highlighted({ text, query }: { text: string; query?: string }) {
  if (!query) return <>{text}</>;
  const idx = text.toLowerCase().indexOf(query.toLowerCase());
  if (idx < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, idx)}
      <mark style={{ background: "var(--primary)", color: "var(--primary-foreground)", borderRadius: 3, padding: "0 2px" }}>
        {text.slice(idx, idx + query.length)}
      </mark>
      {text.slice(idx + query.length)}
    </>
  );
}

/** Label cell for left-right layout */
export function FieldLabel({ field, t, style, nonDefault, onReset, highlight }: {
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
          <Button variant="outline"
            type="button"
            className="ripple"
            onClick={(e) => { e.preventDefault(); makeRipple(e); onReset(); }}
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
          </Button>
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

/** JSON 字段编辑器（对象值）：内部走统一的 JsonCodeEditor —— 搜索/替换、高亮、行内标红、折叠行号。
 *  对外仍收发**解析后的对象**（调用方 schema 驱动，不感知文本），文本态由本组件持有。 */
export function JsonEditor({
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
    <JsonCodeEditor
      value={text}
      placeholder={placeholder}
      minHeight={rows * 24}
      error={error}
      onChange={(next) => {
        setText(next);
        setError("");
        try {
          if (next.trim()) {
            onChange(JSON.parse(next));
          } else {
            onChange(undefined);
          }
        } catch {
          setError("Invalid JSON");
        }
      }}
    />
  );
}

/** Key-value editor (for env) */
export function KvEditor({
  items,
  onChange,
  keyPlaceholder = "KEY",
}: {
  items: Record<string, string>;
  onChange: (items: Record<string, string>) => void;
  keyPlaceholder?: string;
}) {
  const [newKey, setNewKey] = useState("");
  const [newVal, setNewVal] = useState("");
  const entries = Object.entries(items);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.row }}>
      {entries.map(([k, v]) => (
        <div key={k} style={{ display: "flex", gap: 6 }}>
          <Input
            
            style={{ flex: 2, fontSize: F.body, padding: S.inputPad }}
            value={k}
            readOnly
          />
          <Input
            
            style={{ flex: 3, fontSize: F.body, padding: S.inputPad }}
            value={v}
            onChange={(e) => onChange({ ...items, [k]: e.target.value })}
          />
          <Button variant="ghost"
            type="button"
            
            style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
            onClick={() => {
              const next = { ...items };
              delete next[k];
              onChange(next);
            }}
          >
            ×
          </Button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6 }}>
        <Input
          
          style={{ flex: 2, fontSize: F.body, padding: S.inputPad }}
          placeholder={keyPlaceholder}
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
        />
        <Input
          
          style={{ flex: 3, fontSize: F.body, padding: S.inputPad }}
          placeholder="VALUE"
          value={newVal}
          onChange={(e) => setNewVal(e.target.value)}
        />
        <Button variant="ghost"
          type="button"
          
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
        </Button>
      </div>
    </div>
  );
}

/** String list editor (for permissions allow/ask/deny) */
export function StringListEditor({
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
          <Input
            
            style={{ flex: 1, fontSize: F.body, padding: S.inputPad }}
            value={item}
            onChange={(e) => {
              const next = [...items];
              next[i] = e.target.value;
              onChange(next);
            }}
          />
          <Button variant="ghost"
            type="button"
            
            style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
            onClick={() => onChange(items.filter((_, j) => j !== i))}
          >
            ×
          </Button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6 }}>
        <Input
          
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
        <Button variant="ghost"
          type="button"
          
          style={{ fontSize: F.body, padding: S.btnPad }}
          onClick={() => {
            if (draft.trim()) {
              onChange([...items, draft.trim()]);
              setDraft("");
            }
          }}
        >
          +
        </Button>
      </div>
    </div>
  );
}

/** Key → enum-value editor (skillOverrides: skill name → on/off/...) */
export function KvSelectEditor({
  items,
  onChange,
  valueOptions,
  keyPlaceholder = "KEY",
}: {
  items: Record<string, string>;
  onChange: (items: Record<string, string>) => void;
  valueOptions: string[];
  keyPlaceholder?: string;
}) {
  const [newKey, setNewKey] = useState("");
  const entries = Object.entries(items);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.row }}>
      {entries.map(([k, v]) => (
        <div key={k} style={{ display: "flex", gap: 6 }}>
          <Input style={{ flex: 2, fontSize: F.body, padding: S.inputPad }} value={k} readOnly />
          <Select value={v} onValueChange={(nv) => onChange({ ...items, [k]: nv })}>
            <SelectTrigger style={{ flex: 3, fontSize: F.body, padding: S.inputPad }}><SelectValue /></SelectTrigger>
            <SelectContent>
              {valueOptions.map((opt) => (
                <SelectItem key={opt} value={opt}>{opt}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button variant="ghost" type="button"
            style={{ width: S.btnIcon, height: S.btnIcon, minWidth: S.btnIcon, fontSize: F.body }}
            onClick={() => {
              const next = { ...items };
              delete next[k];
              onChange(next);
            }}
          >
            ×
          </Button>
        </div>
      ))}
      <div style={{ display: "flex", gap: 6 }}>
        <Input
          style={{ flex: 2, fontSize: F.body, padding: S.inputPad }}
          placeholder={keyPlaceholder}
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
        />
        <Button variant="ghost" type="button"
          style={{ fontSize: F.body, padding: S.btnPad }}
          onClick={() => {
            if (newKey.trim()) {
              onChange({ ...items, [newKey.trim()]: valueOptions[0] });
              setNewKey("");
            }
          }}
        >
          +
        </Button>
      </div>
    </div>
  );
}

/** Fixed-shape object editor — renders declared sub-fields inline (voice, spinnerVerbs, ...) */
export function ObjectEditor({
  value,
  onChange,
  fields,
  addLabel,
}: {
  value: Record<string, any>;
  onChange: (v: Record<string, any> | undefined) => void;
  fields: ObjectSubField[];
  addLabel: string;
}) {
  const setKey = (k: string, v: any) => {
    const next = { ...value };
    if (v === undefined || v === "" || (Array.isArray(v) && v.length === 0)) delete next[k];
    else next[k] = v;
    onChange(Object.keys(next).length > 0 ? next : undefined);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.row, paddingLeft: 8, borderLeft: "2px solid var(--border)" }}>
      {fields.map((f) => (
        <div key={f.key} style={{ display: "flex", gap: 8, alignItems: f.type === "string[]" ? "flex-start" : "center" }}>
          <span style={{ fontSize: F.hint, color: "var(--text-secondary)", minWidth: 110, flexShrink: 0, paddingTop: f.type === "string[]" ? 6 : 0 }}>
            {f.label}
          </span>
          <div style={{ flex: 1, minWidth: 0 }}>
            {f.type === "boolean" && (
              <Toggle active={!!value[f.key]} onChange={(v) => setKey(f.key, v || undefined)} />
            )}
            {f.type === "select" && (
              <Select value={value[f.key] ?? "__none__"} onValueChange={(v) => setKey(f.key, v === "__none__" ? undefined : v)}>
                <SelectTrigger style={{ fontSize: F.body, padding: S.inputPad, width: "100%" }}><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="__none__">—</SelectItem>
                  {f.options?.map((opt) => (
                    <SelectItem key={opt} value={opt}>{opt}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
            {f.type === "string" && (
              <Input style={{ fontSize: F.body, padding: S.inputPad, width: "100%" }}
                placeholder={f.placeholder}
                value={value[f.key] ?? ""}
                onChange={(e) => setKey(f.key, e.target.value || undefined)} />
            )}
            {f.type === "string[]" && (
              <StringListEditor
                items={Array.isArray(value[f.key]) ? value[f.key] : []}
                onChange={(list) => setKey(f.key, list)}
                addLabel={addLabel}
              />
            )}
          </div>
        </div>
      ))}
    </div>
  );
}

/** Reusable field-row with inline label for handler cards */
export function FieldRow({ label, icon, children }: {
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

/** Section sub-heading with bottom border */
export function SubHeading({ children }: { children: React.ReactNode }) {
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
export function Hint({ children }: { children: React.ReactNode }) {
  return <span style={{ fontSize: F.small, color: "var(--text-tertiary)", lineHeight: 1.4 }}>{children}</span>;
}

// ─── Path Input (text + system picker + autocomplete) ─────

export interface PathSuggestion {
  name: string;
  full_path: string;
  is_dir: boolean;
  modified: number;
}

export function PathInput({
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
        <Input
          
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
        <Button variant="ghost"
          type="button"
          
          style={{ fontSize: F.body, padding: S.inputPad, flexShrink: 0 }}
          onClick={pick}
          title={pathType === "directory" ? t("settings.editor.chooseDir", "选择目录") : t("settings.editor.chooseFile", "选择文件")}
        >
          <SectionIcon name="folder" size={15} />
        </Button>
      </div>

      {/* Autocomplete dropdown */}
      {showSugg && suggestions.length > 0 && (
        <div
          className="glass-elevated animate-fade-in-fast"
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
          }}
        >
          {suggestions.map((s, i) => (
            <Button variant="ghost"
              key={s.full_path}
              type="button"
              
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
            </Button>
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
