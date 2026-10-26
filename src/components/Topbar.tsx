// Topbar — G3 全局骨架顶栏（整体风格重设计票 02）。
// 面包屑（section / page）+ ⌘K 搜索占位 + 通知入口 + 主题切换。Props 驱动（可测），
// 主题切换经 useApp().toggleMode（与侧栏旧实现同一入口，data-mode 由 applyTheme 写）。
import { useTranslation } from "react-i18next";
import { useApp } from "../context/AppContext";
import { Button } from "@/components/ui/button";

export interface TopbarProps {
  /** 面包屑：[sectionLabel, pageLabel]；空串 section 不渲染首段。 */
  crumb: [string, string];
  onOpenNotifications?: () => void;
  notifHidden?: boolean;
}

export function Topbar({ crumb, onOpenNotifications, notifHidden }: TopbarProps) {
  const { t } = useTranslation();
  const { themeMode, toggleMode } = useApp();
  return (
    <header
      id="g3-topbar"
      style={{
        height: 52,
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "0 18px",
        borderBottom: "1px solid var(--border)",
        background: "var(--bg-chrome)",
        flexShrink: 0,
      }}
    >
      {/* 面包屑：section · page（mono 字族，page 段用界面字族加粗） */}
      <nav id="g3-topbar-crumb" style={{ fontSize: 12, color: "var(--muted-foreground)", fontFamily: "var(--font-mono, monospace)", display: "flex", gap: 6, minWidth: 0 }}>
        {crumb[0] && <span style={{ whiteSpace: "nowrap" }}>{crumb[0]}</span>}
        {crumb[0] && <span style={{ opacity: 0.5 }}>/</span>}
        <b id="g3-topbar-crumb-page" style={{ color: "var(--foreground)", fontFamily: "inherit", fontWeight: 600, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{crumb[1]}</b>
      </nav>

      <div style={{ flex: 1 }} />

      {/* ⌘K 搜索占位（票 02 只落骨架：真实命令面板后续票接） */}
      <button
        type="button"
        id="g3-topbar-search"
        title={t("topbar.search", "搜索（⌘K）")}
        style={{
          all: "unset", cursor: "pointer",
          display: "inline-flex", alignItems: "center", gap: 8,
          fontSize: 12, color: "var(--muted-foreground)",
          border: "1px solid var(--border)", borderRadius: "var(--radius-sm)",
          padding: "4px 10px", minWidth: 160,
        }}
      >
        <span style={{ fontSize: 11 }}>⌕</span>
        <span style={{ flex: 1, textAlign: "start" }}>{t("topbar.search", "搜索（⌘K）")}</span>
        <kbd style={{
          fontFamily: "var(--font-mono, monospace)", fontSize: 11,
          border: "1px solid var(--border)", borderBottomWidth: 2, borderRadius: 5,
          padding: "0 6px", color: "var(--muted-foreground)",
          background: "var(--card)",
        }}>⌘K</kbd>
      </button>

      {!notifHidden && (
        <Button
          id="g3-topbar-bell"
          variant="ghost" size="sm" title={t("nav.notifications", "通知")}
          style={{ fontSize: 14, padding: 4, width: 28, height: 28, minWidth: 0 }}
          onClick={onOpenNotifications}
        >
          🔔
        </Button>
      )}

      <Button
        id="g3-topbar-theme"
        data-mode={themeMode}
        variant="ghost" size="sm"
        title={themeMode === "light" ? t("theme.dark", "深色") : t("theme.light", "浅色")}
        style={{ fontSize: 14, padding: 4, width: 28, height: 28, minWidth: 0 }}
        onClick={toggleMode}
      >
        {themeMode === "light" ? "☀" : "☾"}
      </Button>
    </header>
  );
}
