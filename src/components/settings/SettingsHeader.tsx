// ─── Settings sticky header ─────────────────────────────────
// Mode switch (GUI/JSON) + search + import + recommended + save.
// Search box is a placeholder pass-through for D1; wired to global search in D2.

import { useTranslation } from "react-i18next";
import { F, S, SectionIcon, SvgIcon } from "./editors";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { makeRipple } from "../../components/shared";

// ponytail: 萤火虫 tab — 激活态 accent-subtle bg + primary color + 底部 2px 流光边;
//           非激活 hover bg-glass-hover 柔和。底层 variant 仍走 shadcn ring。
function ModeTab({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <Button
      variant={active ? "default" : "ghost"}
      className="ripple"
      style={{
        fontSize: F.body,
        padding: S.btnPad,
        position: "relative",
        background: active ? "var(--accent-subtle)" : "transparent",
        color: active ? "var(--primary)" : "var(--text-secondary)",
        boxShadow: active ? "inset 0 -2px 0 0 var(--primary), 0 0 6px var(--accent-subtle)" : "none",
        transition: "all 200ms cubic-bezier(0.4, 0, 0.2, 1)",
      }}
      onClick={(e) => { makeRipple(e); onClick(); }}
    >
      {children}
    </Button>
  );
}

export interface SettingsHeaderProps {
  mode: "json" | "gui";
  onModeChange: (mode: "json" | "gui") => void;
  search: string;
  onSearchChange: (v: string) => void;
  onLoadRecommended: () => void;
  onImport: () => void;
  onSave: () => void;
  saving: boolean;
  toast: string;
  /** Draft differs from last-saved baseline. */
  dirty: boolean;
  /** callback ref on the sticky root — parent measures actual (possibly multi-row) height */
  rootRef?: (el: HTMLDivElement | null) => void;
}

export function SettingsHeader({
  mode,
  onModeChange,
  search,
  onSearchChange,
  onLoadRecommended,
  onImport,
  onSave,
  saving,
  toast,
  dirty,
  rootRef,
}: SettingsHeaderProps) {
  const { t } = useTranslation();
  return (
    <div
      ref={rootRef}
      className="settings-sticky-bar"
      style={{
        position: "sticky",
        top: 0,
        zIndex: 30,
        display: "flex",
        alignItems: "center",
        gap: 8,
        flexWrap: "wrap",
        padding: "12px 4px",
        background: "var(--bg-glass)",
        // ponytail: backdrop-filter 已删，理由同 SectionAnchorNav.tsx（var(--bg-glass) 全不透明）
      }}
    >
      {/* Mode switch — GUI/JSON 萤火虫 tab */}
      <ModeTab active={mode === "gui"} onClick={() => onModeChange("gui")}>
        {t("settings.guiMode")}
      </ModeTab>
      <ModeTab active={mode === "json"} onClick={() => onModeChange("json")}>
        {t("settings.jsonMode")}
      </ModeTab>

      {/* Global search (placeholder for D1) */}
      <div style={{ position: "relative", flex: 1, minWidth: 180, maxWidth: 360 }}>
        <SvgIcon
          d="M11 3a8 8 0 1 0 0 16 8 8 0 0 0 0-16Z M21 21l-4.35-4.35"
          size={14}
          style={{ position: "absolute", left: 10, top: "50%", transform: "translateY(-50%)", color: "var(--text-tertiary)" }}
        />
        <Input
          
          style={{ fontSize: F.hint, padding: "6px 10px 6px 30px", width: "100%" }}
          placeholder={t("settings.search", "搜索设置…")}
          value={search}
          onChange={(e) => onSearchChange(e.target.value)}
        />
        {search && (
          <Button variant="outline"
            type="button"
            style={{
              position: "absolute", right: 6, top: "50%", transform: "translateY(-50%)",
              background: "none", border: "none", cursor: "pointer", color: "var(--text-tertiary)", fontSize: 12,
            }}
            onClick={() => onSearchChange("")}
          >
            ×
          </Button>
        )}
      </div>

      <div style={{ width: 1, height: 20, background: "var(--border)", margin: "0 4px" }} />

      <Button variant="ghost"
        
        style={{ fontSize: F.hint, padding: "6px 14px" }}
        onClick={onLoadRecommended}
      >
        <SectionIcon name="bolt" size={14} /> {t("settings.loadRecommended")}
      </Button>
      <Button variant="ghost"
        
        style={{ fontSize: F.hint, padding: "6px 14px" }}
        onClick={onImport}
      >
        <SectionIcon name="folder" size={14} /> {t("settings.importFromClaudeCode", "从 Claude Code 导入")}
      </Button>

      {toast && <span style={{ fontSize: F.body, color: "var(--color-success)" }}>{toast}</span>}

      {/* Dirty / saved status indicator */}
      {!toast && (
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 6,
            fontSize: F.hint,
            color: dirty ? "var(--color-warning)" : "var(--text-tertiary)",
          }}
        >
          <span
            style={{
              width: 7,
              height: 7,
              borderRadius: "50%",
              background: dirty ? "var(--color-warning)" : "var(--text-tertiary)",
              opacity: dirty ? 1 : 0.5,
              flexShrink: 0,
            }}
          />
          {dirty
            ? t("settings.unsavedChanges", "未保存更改")
            : t("settings.allSaved", "已保存")}
        </span>
      )}

      <Button variant={dirty ? "default" : "ghost"}
        className="ripple"
        style={{ fontSize: F.body, padding: S.btnPad, minWidth: 80 }}
        onClick={(e) => { makeRipple(e); onSave(); }}
        disabled={saving || !dirty}
      >
        {saving ? t("status.loading") : t("action.save")}
      </Button>
    </div>
  );
}
