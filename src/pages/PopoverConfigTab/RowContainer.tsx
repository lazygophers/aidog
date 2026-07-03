// ─── 行容器（droppable）：列数选择 + 该行 grid 子项 ──
// 自 PopoverConfigTab.tsx 外迁（arch 阶段6 S5）。
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useDroppable } from "@dnd-kit/core";
import { SortableContext, rectSortingStrategy } from "@dnd-kit/sortable";
import type { PopoverItem } from "../../services/api";
import { MAX_COLS } from "./constants";

export function RowContainer({
  row, cols, items, onSetCols, children,
}: {
  row: number;
  cols: number;
  items: PopoverItem[];
  onSetCols: (c: number) => void;
  children: ReactNode;
}) {
  const { t } = useTranslation();
  const { setNodeRef, isOver } = useDroppable({ id: `row-${row}` });
  return (
    <div
      ref={setNodeRef}
      style={{
        border: `1px solid ${isOver ? "var(--accent)" : "var(--border)"}`,
        borderRadius: 10, padding: 8,
        background: isOver ? "var(--bg-glass)" : "transparent",
        display: "flex", flexDirection: "column", gap: 8,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
          {t("popover.rowLabel", "第 {{n}} 行", { n: row + 1 })}
        </span>
        <span style={{ fontSize: 11, color: "var(--text-tertiary)" }}>{t("popover.cols", "列数")}</span>
        {[1, 2, 3].map((c) => (
          <button
            key={c}
            style={{
              fontSize: 11, padding: "2px 8px", borderRadius: 4, cursor: "pointer",
              border: cols === c ? "none" : "1px solid var(--glass-border)",
              background: cols === c ? "var(--accent)" : "transparent",
              color: cols === c ? "#fff" : "var(--text-secondary)",
            }}
            onClick={() => onSetCols(c)}
          >
            {c}
          </button>
        ))}
      </div>
      <SortableContext items={items.map((i) => i.id)} strategy={rectSortingStrategy}>
        <div style={{
          display: "grid",
          gridTemplateColumns: `repeat(${Math.min(cols, MAX_COLS)}, minmax(0, 1fr))`,
          gap: 8,
        }}>
          {children}
        </div>
      </SortableContext>
    </div>
  );
}
