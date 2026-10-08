// SectionTabs — 编辑页「常用平铺 + 高级 tab」的左侧菜单 tab 列（2026-10-08 改版二轮：
// 由顶部 sticky 分段条改为左侧竖排 rail，用户裁决；方向 B 原型 .scratch 已删，结构同其左栏）。
// sticky 吸在 <main> 滚动容器顶部，父级用 flex row 包裹（rail 左、panels 右）。
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
        top: 12,
        zIndex: 20,
        display: "flex",
        flexDirection: "column",
        alignItems: "stretch",
        gap: 2,
        flexShrink: 0,
        width: 148,
        padding: "8px 4px",
        background: "var(--bg-glass)",
        borderRadius: "var(--radius-md)",
      }}
      role="tablist"
      aria-orientation="vertical"
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
              border: 0,
              background: on ? "var(--accent-subtle)" : "transparent",
              color: on ? "var(--text-primary)" : "var(--text-secondary)",
              fontSize: 13,
              fontWeight: on ? 600 : 500,
              padding: "7px 12px",
              borderRadius: "var(--radius-sm)",
              cursor: "pointer",
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
              textAlign: "left",
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
