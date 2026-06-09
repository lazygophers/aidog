import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useApp } from "../context/AppContext";
import { ALL_LOCALES } from "../locales";
import type { ThemeName } from "../themes";

// ── SVG Icons ──

const icons = {
  proxy: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 9h12M7 5l-4 4 4 4M13 5l4 4-4 4" />
    </svg>
  ),
  platforms: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="2" width="6" height="6" rx="1.5" />
      <rect x="10" y="2" width="6" height="6" rx="1.5" />
      <rect x="2" y="10" width="6" height="6" rx="1.5" />
      <rect x="10" y="10" width="6" height="6" rx="1.5" />
    </svg>
  ),
  groups: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M6 3h10M6 9h10M6 15h10M2 3h.01M2 9h.01M2 15h.01" />
    </svg>
  ),
  settings: (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M7.2 1.5h3.6l.6 2.2a6 6 0 012 1.15l2.2-.55 1.8 3.12-1.8 1.48a6 6 0 010 2.4l1.8 1.48-1.8 3.12-2.2-.55a6 6 0 01-2 1.15l-.6 2.2H7.2l-.6-2.2a6 6 0 01-2-1.15l-2.2.55-1.8-3.12 1.8-1.48a6 6 0 010-2.4L.6 7.42l1.8-3.12 2.2.55a6 6 0 012-1.15z" />
      <circle cx="9" cy="9" r="2.5" />
    </svg>
  ),
  chevron: (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 4.5L6 7.5L9 4.5" />
    </svg>
  ),
  sun: (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <circle cx="8" cy="8" r="3" />
      <path d="M8 1v2M8 13v2M1 8h2M13 8h2M3.05 3.05l1.41 1.41M11.54 11.54l1.41 1.41M3.05 12.95l1.41-1.41M11.54 4.46l1.41-1.41" />
    </svg>
  ),
  moon: (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14 8.5A6 6 0 018.5 2 6.5 6.5 0 1014 8.5z" />
    </svg>
  ),
};

// ── Types ──

export interface NavItem {
  id: string;
  icon: string;
  labelKey: string;
}

interface SidebarProps {
  navItems: NavItem[];
  activeId: string;
  onNavigate: (id: string) => void;
}

// ── Dropdown Component ──

function Dropdown({
  trigger,
  children,
  open,
  onToggle,
  align = "left",
}: {
  trigger: React.ReactNode;
  children: React.ReactNode;
  open: boolean;
  onToggle: () => void;
  align?: "left" | "right";
}) {
  return (
    <div style={{ position: "relative" }}>
      <div onClick={onToggle} style={{ cursor: "pointer" }}>
        {trigger}
      </div>
      {open && (
        <>
          <div
            style={{ position: "fixed", inset: 0, zIndex: 99 }}
            onClick={onToggle}
          />
          <div
            className="glass-elevated"
            style={{
              position: "absolute",
              bottom: "100%",
              [align]: 0,
              marginBottom: 6,
              minWidth: 180,
              padding: 6,
              zIndex: 100,
              animation: "fadeIn 200ms ease both",
            }}
          >
            {children}
          </div>
        </>
      )}
    </div>
  );
}

function DropdownItem({
  active,
  children,
  onClick,
}: {
  active?: boolean;
  children: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      className="btn btn-ghost"
      style={{
        width: "100%",
        justifyContent: "flex-start",
        gap: 8,
        padding: "8px 10px",
        fontSize: 13,
        fontWeight: active ? 600 : 400,
        color: active ? "var(--accent)" : "var(--text-primary)",
        background: active ? "var(--accent-subtle)" : "transparent",
        borderRadius: "var(--radius-sm)",
      }}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

// ── Main Sidebar ──

export function Sidebar({ navItems, activeId, onNavigate }: SidebarProps) {
  const { t } = useTranslation();
  const {
    locale, setLocale,
    themeName, setThemeName,
    themeMode, toggleMode,
    availableThemes,
  } = useApp();
  const [themeOpen, setThemeOpen] = useState(false);
  const [langOpen, setLangOpen] = useState(false);

  return (
    <aside
      className="glass glass-highlight"
      style={{
        width: 200,
        minWidth: 200,
        height: "100%",
        display: "flex",
        flexDirection: "column",
        padding: "16px 10px",
        gap: 4,
      }}
    >
      {/* Logo */}
      <div style={{
        padding: "10px 12px 20px",
        fontSize: 17,
        fontWeight: 700,
        letterSpacing: "-0.3px",
        display: "flex",
        alignItems: "center",
        gap: 8,
      }}>
        <span style={{
          fontSize: 22,
          filter: "drop-shadow(0 2px 4px rgba(0,0,0,0.1))",
        }}>🐕</span>
        <span>{t("app.title")}</span>
      </div>

      {/* Navigation */}
      <nav style={{ flex: 1, display: "flex", flexDirection: "column", gap: 2 }}>
        {navItems.map((item) => {
          const isActive = item.id === activeId;
          return (
            <button
              key={item.id}
              className="btn btn-ghost"
              style={{
                justifyContent: "flex-start",
                gap: 10,
                padding: "10px 12px",
                fontWeight: isActive ? 600 : 400,
                color: isActive
                  ? "var(--accent)"
                  : "var(--text-secondary)",
                background: isActive ? "var(--accent-subtle)" : "transparent",
                borderRadius: "var(--radius-sm)",
                fontSize: 13,
              }}
              onClick={() => onNavigate(item.id)}
            >
              <span style={{
                display: "flex",
                alignItems: "center",
                opacity: isActive ? 1 : 0.6,
                transition: "opacity 200ms",
              }}>
                {(icons as Record<string, React.ReactNode>)[item.icon]}
              </span>
              <span>{t(item.labelKey)}</span>
            </button>
          );
        })}
      </nav>

      {/* Bottom Controls */}
      <div style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        paddingTop: 12,
        borderTop: "1px solid var(--border)",
      }}>
        {/* Theme Picker */}
        <Dropdown
          open={themeOpen}
          onToggle={() => setThemeOpen((v) => !v)}
          trigger={
            <button className="btn btn-ghost" style={{
              width: "100%",
              justifyContent: "space-between",
              fontSize: 12,
              padding: "7px 10px",
              color: "var(--text-secondary)",
            }}>
              <span>🎨 {t(`theme.${themeName}`)}</span>
              <span style={{ opacity: 0.4 }}>{icons.chevron}</span>
            </button>
          }
        >
          {availableThemes.map((th) => (
            <DropdownItem
              key={th.name}
              active={th.name === themeName}
              onClick={() => {
                setThemeName(th.name as ThemeName);
                setThemeOpen(false);
              }}
            >
              {t(th.label)}
            </DropdownItem>
          ))}
        </Dropdown>

        {/* Language Picker */}
        <Dropdown
          open={langOpen}
          onToggle={() => setLangOpen((v) => !v)}
          trigger={
            <button className="btn btn-ghost" style={{
              width: "100%",
              justifyContent: "space-between",
              fontSize: 12,
              padding: "7px 10px",
              color: "var(--text-secondary)",
            }}>
              <span>🌐 {t(`lang.${locale}`)}</span>
              <span style={{ opacity: 0.4 }}>{icons.chevron}</span>
            </button>
          }
        >
          {ALL_LOCALES.map((loc) => (
            <DropdownItem
              key={loc}
              active={loc === locale}
              onClick={() => {
                setLocale(loc);
                setLangOpen(false);
              }}
            >
              {t(`lang.${loc}`)}
            </DropdownItem>
          ))}
        </Dropdown>

        {/* Dark/Light Toggle */}
        <button
          className="btn btn-ghost"
          style={{
            width: "100%",
            justifyContent: "center",
            fontSize: 12,
            padding: "7px 10px",
            gap: 6,
            color: "var(--text-secondary)",
          }}
          onClick={toggleMode}
        >
          {themeMode === "light" ? icons.moon : icons.sun}
          <span>{themeMode === "light" ? t("theme.dark") : t("theme.light")}</span>
        </button>
      </div>
    </aside>
  );
}
