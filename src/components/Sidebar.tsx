import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useApp } from "../context/AppContext";
import { ALL_LOCALES } from "../locales";
import type { ThemeName } from "../themes";

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
              marginBottom: 4,
              minWidth: 170,
              padding: 4,
              zIndex: 100,
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
      className="btn"
      style={{
        width: "100%",
        justifyContent: "flex-start",
        background: active ? "var(--accent-subtle)" : undefined,
        fontWeight: active ? 600 : 400,
        border: "none",
        borderRadius: "var(--radius-sm)",
        fontSize: 12,
      }}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

export function Sidebar({ navItems, activeId, onNavigate }: SidebarProps) {
  const { t } = useTranslation();
  const { locale, setLocale, themeName, setThemeName, themeMode, toggleMode, availableThemes } =
    useApp();
  const [themeOpen, setThemeOpen] = useState(false);
  const [langOpen, setLangOpen] = useState(false);

  return (
    <aside
      className="glass"
      style={{
        width: 220,
        minWidth: 220,
        height: "100%",
        display: "flex",
        flexDirection: "column",
        padding: "16px 12px",
        gap: 4,
        borderRadius: 0,
        borderLeft: "none",
        borderTop: "none",
        borderBottom: "none",
      }}
    >
      {/* Logo */}
      <div
        style={{
          padding: "8px 12px 20px",
          fontSize: 18,
          fontWeight: 700,
          letterSpacing: "-0.3px",
        }}
      >
        🐕 {t("app.title")}
      </div>

      {/* Navigation */}
      <nav style={{ flex: 1, display: "flex", flexDirection: "column", gap: 2 }}>
        {navItems.map((item) => (
          <button
            key={item.id}
            className="btn"
            style={{
              justifyContent: "flex-start",
              gap: 10,
              padding: "10px 12px",
              background:
                item.id === activeId ? "var(--accent-subtle)" : "transparent",
              fontWeight: item.id === activeId ? 600 : 400,
              border: "none",
              borderRadius: "var(--radius-sm)",
              color:
                item.id === activeId
                  ? "var(--accent)"
                  : "var(--text-primary)",
            }}
            onClick={() => onNavigate(item.id)}
          >
            <span style={{ fontSize: 16 }}>{item.icon}</span>
            <span>{t(item.labelKey)}</span>
          </button>
        ))}
      </nav>

      {/* Bottom controls */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 6,
          paddingTop: 12,
          borderTop: "1px solid var(--border)",
        }}
      >
        {/* Theme picker */}
        <Dropdown
          open={themeOpen}
          onToggle={() => setThemeOpen((v) => !v)}
          trigger={
            <div
              className="btn"
              style={{
                width: "100%",
                justifyContent: "space-between",
                fontSize: 12,
                padding: "6px 10px",
              }}
            >
              <span>🎨 {t(`theme.${themeName}`)}</span>
              <span style={{ opacity: 0.4 }}>▴</span>
            </div>
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

        {/* Language picker */}
        <Dropdown
          open={langOpen}
          onToggle={() => setLangOpen((v) => !v)}
          trigger={
            <div
              className="btn"
              style={{
                width: "100%",
                justifyContent: "space-between",
                fontSize: 12,
                padding: "6px 10px",
              }}
            >
              <span>🌐 {t(`lang.${locale}`)}</span>
              <span style={{ opacity: 0.4 }}>▴</span>
            </div>
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

        {/* Mode toggle */}
        <button
          className="btn"
          style={{
            width: "100%",
            justifyContent: "center",
            fontSize: 12,
            padding: "6px 10px",
          }}
          onClick={toggleMode}
        >
          {themeMode === "light" ? `🌙 ${t("theme.dark")}` : `☀️ ${t("theme.light")}`}
        </button>
      </div>
    </aside>
  );
}
