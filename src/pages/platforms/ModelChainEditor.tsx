// ModelChainEditor — 槽位 / 时段共用的有序模型链编辑器：
// chip 横排 wrap（拖拽 + 键盘手柄排序，序号徽标 1/2/3，链首紫底 = 主，✕ 悬停删除）+ 行尾 ModelCombobox 添加（下拉多选：已选打勾，再点取消，点选不关闭）。
// 纯受控：value 为链，所有变更经 onChange 写出归一化后的新链（trim / 去空 / 去重保序）。
import { useState } from "react";
import type { TFunction } from "i18next";
import type { Protocol } from "../../services/api";
import { SortableList } from "../../components/SortableList";
import { slotList } from "../../domains/platforms/slotValue";
import { ModelCombobox } from "./ModelCombobox";

export function ModelChainEditor({
  testId, value, onChange, source, protocol, open, onOpenChange, t,
}: {
  /** 稳定 id 前缀（槽位名 / 时段槽位名），测试与 DOM 锚点用。 */
  testId: string;
  value: string[];
  onChange: (next: string[]) => void;
  source: string[];
  protocol: Protocol;
  open?: boolean;
  onOpenChange?: (o: boolean) => void;
  t: TFunction;
}) {
  const [draft, setDraft] = useState("");
  const add = (v: string) => {
    setDraft("");
    const next = slotList([...value, v]);
    if (next.length !== value.length) onChange(next);
  };
  // 下拉点选 = 切换：已在链中则移除，否则追加链尾
  const toggle = (v: string) => (value.includes(v) ? onChange(value.filter((m) => m !== v)) : add(v));
  return (
    // 横排 wrap（V2 行式增强）：chip 内联序号徽标，链首紫底 = 主模型；✕ 悬停出现（globals.css .model-chip）。
    <div data-testid={`chain-${testId}`} style={{ display: "flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>
      <SortableList
        strategy="grid"
        items={value.map((id) => ({ id }))}
        onReorder={(next) => onChange(next.map((i) => i.id))}
        renderItem={({ id }, handle) => (
          <div
            data-testid={`chip-${id}`}
            data-state={id === value[0] ? "primary" : "backup"}
            className="model-chip"
            style={{
              display: "inline-flex", alignItems: "center", gap: 6, padding: "3px 9px", fontSize: 14,
              borderRadius: "var(--radius-sm)", border: "1px solid var(--border)",
              background: id === value[0] ? "var(--accent-subtle)" : "transparent",
            }}
          >
            <span
              style={{
                fontSize: 14, fontWeight: 600, width: 15, height: 15, borderRadius: 4,
                display: "grid", placeItems: "center", flexShrink: 0,
                background: id === value[0] ? "var(--primary)" : "var(--muted)",
                color: id === value[0] ? "var(--primary-foreground)" : "var(--muted-foreground)",
              }}
            >
              {value.indexOf(id) + 1}
            </span>
            <span
              ref={handle.ref}
              data-testid={`chip-handle-${id}`}
              {...handle.attributes}
              {...handle.listeners}
              aria-label={t("platform.modelChain.drag", "拖动排序")}
              title={t("platform.modelChain.drag", "拖动排序")}
              style={{ cursor: "grab", color: "var(--text-tertiary)", touchAction: "none", userSelect: "none" }}
            >
              ⠿
            </span>
            <span style={{ minWidth: 0, maxWidth: 240, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{id}</span>
            <button
              type="button"
              data-testid={`chip-remove-${id}`}
              className="model-chip-remove"
              aria-label={t("platform.modelChain.remove", "移除")}
              title={t("platform.modelChain.remove", "移除")}
              onClick={() => onChange(value.filter((m) => m !== id))}
              style={{ cursor: "pointer", color: "var(--text-tertiary)", background: "transparent", border: 0, padding: "0 2px" }}
            >
              ✕
            </button>
          </div>
        )}
      />
      <div style={{ flex: "1 1 200px", minWidth: 200, maxWidth: 360 }}>
        <ModelCombobox
        value={draft} onChange={setDraft} onSelect={toggle}
        source={source} selected={value} protocol={protocol}
        open={open} onOpenChange={onOpenChange}
        placeholder={t("platform.modelChain.add", "添加模型（回车确认）")}
        inputProps={{
          "data-testid": `chain-input-${testId}`,
          onKeyDown: (e) => {
            if (e.key === "Enter") { e.preventDefault(); add(draft); }
          },
        }}
        t={t}
        />
      </div>
    </div>
  );
}
