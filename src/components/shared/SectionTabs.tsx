// SectionTabs — 编辑页「常用平铺 + 高级 tab」的分段 tab 条（2026-10-08 编辑页改版，方向 A）。
// sticky 吸在 <main> 滚动容器顶部（复用 .settings-sticky-bar 玻璃分隔 idiom，同 SettingsHeader）。
import { useTranslation } from "react-i18next";

export interface SectionTab {
  id: string;
  label: string;
}

export function SectionTabs({ tabs, active, onChange }: {
  tabs: SectionTab[];
  active: string;
  onChange: (id: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className="settings-sticky-bar"
      style={{
        position: "sticky",
        top: 0,
        zIndex: 20,
        display: "flex",
        alignItems: "center",
        gap: 2,
        flexWrap: "nowrap",
        padding: "10px 4px",
        overflowX: "auto",
        background: "var(--bg-glass)",
      }}
      role="tablist"
      aria-label={t("common.sectionTabs", "设置分区")}
    >
      {tabs.map(tab => {
        const on = tab.id === active;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={on}
            onClick={() => onChange(tab.id)}
            style={{
              border: on ? "1px solid var(--border)" : "1px solid transparent",
              background: on ? "var(--bg-glass)" : "transparent",
              color: on ? "var(--text-primary)" : "var(--text-secondary)",
              fontSize: 13,
              fontWeight: 600,
              padding: "6px 14px",
              borderRadius: "var(--radius-sm)",
              cursor: "pointer",
              whiteSpace: "nowrap",
              transition: "all var(--transition)",
            }}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
