// ─── Section anchor chip nav ────────────────────────────────
// Sticky row of section chips (icon + i18n label). Click → smooth scroll.
// Active chip is driven by scroll-spy (IntersectionObserver) in the parent.

import { useTranslation } from "react-i18next";
import { SECTIONS } from "../../services/claude-settings-schema";
import { F, SectionIcon } from "./editors";

export interface SectionAnchorNavProps {
  activeId: string;
  onJump: (id: string) => void;
  /** sticky top offset (px) — sits below the header */
  top: number;
  /** callback ref on the sticky root — parent measures actual (possibly multi-row) height */
  rootRef?: (el: HTMLDivElement | null) => void;
}

export function SectionAnchorNav({ activeId, onJump, top, rootRef }: SectionAnchorNavProps) {
  const { t } = useTranslation();
  return (
    <div
      ref={rootRef}
      className="settings-sticky-bar"
      style={{
        position: "sticky",
        top,
        zIndex: 20,
        display: "flex",
        gap: 6,
        flexWrap: "nowrap",
        padding: "10px 4px",
        overflowX: "auto",
        background: "var(--bg-glass)",
        backdropFilter: "blur(20px)",
        WebkitBackdropFilter: "blur(20px)",
      }}
    >
      {SECTIONS.map((section) => {
        const active = activeId === section.id;
        return (
          <button
            key={section.id}
            type="button"
            onClick={() => onJump(section.id)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              padding: "5px 12px",
              fontSize: F.hint,
              fontWeight: active ? 600 : 400,
              color: active ? "#fff" : "var(--text-secondary)",
              background: active ? "var(--accent)" : "var(--bg-glass)",
              border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
              borderRadius: 999,
              cursor: "pointer",
              whiteSpace: "nowrap",
              transition: "all 120ms ease",
            }}
          >
            <SectionIcon name={section.id} size={14} />
            {t(section.labelKey)}
          </button>
        );
      })}
    </div>
  );
}
