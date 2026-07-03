// ─── 单卡（sortable wrapper）：左上角手柄拖拽 ──
// PointerSensor，不依赖 WKWebView HTML5 DnD。
// 自 PopoverConfigTab.tsx 外迁（arch 阶段6 S5）。
import type { CSSProperties, ReactNode } from "react";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { PopoverItem } from "../../services/api";

const gripSvg = (
  <svg width="12" height="18" viewBox="0 0 14 20" fill="currentColor">
    <circle cx="4" cy="3" r="1.8" /><circle cx="4" cy="10" r="1.8" /><circle cx="4" cy="17" r="1.8" />
    <circle cx="10" cy="3" r="1.8" /><circle cx="10" cy="10" r="1.8" /><circle cx="10" cy="17" r="1.8" />
  </svg>
);

export function SortableCard({ item, children }: { item: PopoverItem; children: ReactNode }) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({ id: item.id });
  const style: CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.4 : item.visible ? 1 : 0.5,
    zIndex: isDragging ? 10 : undefined,
    position: "relative",
  };
  return (
    <div ref={setNodeRef} style={style}>
      {children}
      <span
        ref={setActivatorNodeRef}
        {...attributes}
        {...listeners}
        style={{
          position: "absolute", top: 6, left: 6, cursor: "grab",
          color: "var(--text-tertiary)", display: "inline-flex", touchAction: "none",
        }}
      >
        {gripSvg}
      </span>
    </div>
  );
}
