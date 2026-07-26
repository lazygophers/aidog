// ── CompactCard ──
// 可折叠卡片壳：header（常显关键指标区）+ 可展开二级明细（expandable children）。
// 默认只显 header；点击展开区切换二级内容。供 Platforms / Groups 列表卡片复用。
// 外观：萤火虫玻璃签名（.glass-surface 扁平卡面 + hover 萤火虫流光描边）+ reveal 入场 + hover-lift。
//
// 受控 / 非受控双模式：
// - 传 `expanded` + `onToggle` → 受控（父管理展开态，适合「全展开/全折叠」批量控制）。
// - 不传 → 内部 useState 自管理。
// children 为空时不渲染展开触发器（无二级内容则纯 header 卡片）。

import { useState, type ReactNode } from "react";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { useReveal, makeRipple } from "../../utils/motion";

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
  /** reveal 入场错峰延迟（ms），列表 stagger 用。默认 0。 */
  revealDelay?: number;
  /** 禁用 reveal 入场（如静态已渲染的列表项重排）。 */
  noReveal?: boolean;
}

export function CompactCard({
  header,
  children,
  expanded,
  onToggle,
  defaultExpanded = false,
  toggleLabel,
  style,
  revealDelay = 0,
  noReveal = false,
}: CompactCardProps) {
  const [internal, setInternal] = useState(defaultExpanded);
  const isControlled = expanded !== undefined;
  const open = isControlled ? expanded! : internal;
  const hasChildren = children != null && children !== false;
  const { ref, shown } = useReveal<HTMLDivElement>(revealDelay);
  const revealOn = noReveal || shown;

  const toggle = () => {
    const next = !open;
    if (isControlled) onToggle?.(next);
    else setInternal(next);
  };

  return (
    <Card
      ref={ref}
      className={`glass-surface hover-lift${revealOn ? " reveal in" : " reveal"}`}
      style={{
        display: "flex",
        flexDirection: "column",
        padding: 20,
        borderRadius: "var(--radius-md)",
        position: "relative",
        overflow: "hidden",
        ...style,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
        <div style={{ flex: 1, minWidth: 0 }}>{header}</div>
        {hasChildren && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="ripple"
            style={{ height: "auto" }}
            aria-label={toggleLabel}
            aria-expanded={open}
            onClick={(e) => {
              e.stopPropagation();
              makeRipple(e);
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
                transition: "transform 0.25s cubic-bezier(0.4,0,0.2,1)",
                transform: open ? "rotate(180deg)" : "rotate(0deg)",
              }}
            >
              <path d="M6 9l6 6 6-6" />
            </svg>
          </Button>
        )}
      </div>
      {hasChildren && open && (
        <div
          className="animate-fade-in"
          style={{
            marginTop: 12,
            borderTop: "1px solid var(--border)",
            paddingTop: 12,
          }}
        >
          {children}
        </div>
      )}
    </Card>
  );
}
