// ── CompactCard ──
// 可折叠卡片壳：header（常显关键指标区）+ 可展开二级明细（expandable children）。
// 默认只显 header；点击展开区切换二级内容。供 Platforms / Groups 列表卡片复用。
// 外观走 Liquid Glass（glass-surface），全 CSS 变量。
//
// 受控 / 非受控双模式：
// - 传 `expanded` + `onToggle` → 受控（父管理展开态，适合「全展开/全折叠」批量控制）。
// - 不传 → 内部 useState 自管理。
// children 为空时不渲染展开触发器（无二级内容则纯 header 卡片）。

import { useState, type ReactNode } from "react";

export interface CompactCardProps {
  /** 常显关键指标区（名称 / 状态 / 余额 / 核心统计 / 快操作）。 */
  header: ReactNode;
  /** 展开后的二级明细（endpoints / 模型映射等）；省略则无展开能力。 */
  children?: ReactNode;
  /** 受控展开态；传则父接管，需配合 onToggle。 */
  expanded?: boolean;
  /** 受控切换回调；收到目标展开态。 */
  onToggle?: (next: boolean) => void;
  /** 非受控初始展开态，默认 false。 */
  defaultExpanded?: boolean;
  /** 展开触发器无障碍文案（i18n 文本，由调用方传入）。 */
  toggleLabel?: string;
  /** 额外外层样式（如拖拽时的 transform / opacity）。 */
  style?: React.CSSProperties;
}

export function CompactCard({
  header,
  children,
  expanded,
  onToggle,
  defaultExpanded = false,
  toggleLabel,
  style,
}: CompactCardProps) {
  const [internal, setInternal] = useState(defaultExpanded);
  const isControlled = expanded !== undefined;
  const open = isControlled ? expanded! : internal;
  const hasChildren = children != null && children !== false;

  const toggle = () => {
    const next = !open;
    if (isControlled) onToggle?.(next);
    else setInternal(next);
  };

  return (
    <div
      className="glass-surface"
      style={{
        display: "flex",
        flexDirection: "column",
        padding: 12,
        borderRadius: "var(--radius-md)",
        ...style,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
        <div style={{ flex: 1, minWidth: 0 }}>{header}</div>
        {hasChildren && (
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            aria-label={toggleLabel}
            aria-expanded={open}
            onClick={(e) => {
              e.stopPropagation();
              toggle();
            }}
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
              strokeLinecap="round"
              strokeLinejoin="round"
              style={{
                transition: "transform 0.2s ease",
                transform: open ? "rotate(180deg)" : "rotate(0deg)",
              }}
            >
              <path d="M6 9l6 6 6-6" />
            </svg>
          </button>
        )}
      </div>
      {hasChildren && open && (
        <div style={{ marginTop: 10, borderTop: "1px solid var(--border)", paddingTop: 10 }}>{children}</div>
      )}
    </div>
  );
}
