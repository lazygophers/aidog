// ── FilterDropdown ──
// 带搜索的筛选下拉，统一 Stats / Logs 的【平台/模型/分组】3 维筛选 UI。
// 从 Stats 本地 SearchableFilter 提炼而来，保留 glass-elevated + zIndex 1000 + search。
// 数据源由各调用方传入（Stats: 有数据平台派生；Logs: 全平台 / 全分组 / 全模型）。

import { useState, useEffect, useMemo, useRef } from "react";

export interface FilterDropdownProps {
  width: number;
  value: string;
  onChange: (v: string) => void;
  /** 「全部」选项标签（value="" 时显示） */
  allLabel: string;
  searchPlaceholder: string;
  options: Array<{ value: string; label: string }>;
  /** 搜索无匹配时的空态文案 */
  emptyLabel: string;
}

export function FilterDropdown({ width, value, onChange, allLabel, searchPlaceholder, options, emptyLabel }: FilterDropdownProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return options;
    return options.filter(o => o.label.toLowerCase().includes(q));
  }, [options, search]);

  const current = options.find(o => o.value === value);

  return (
    <div ref={ref} style={{ position: "relative", width }}>
      <button
        className="input"
        // ponytail: lineHeight/height 显式锁, 防 .input transition:all + 继承链抖动致 trigger 文字错位
        style={{ fontSize: 14, lineHeight: 1.5, height: 36, width: "100%", textAlign: "left", cursor: "pointer", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
        onClick={() => setOpen(o => !o)}
      >
        {current ? current.label : allLabel}
      </button>
      {open && (
        <div
          className="glass-elevated"
          style={{
            position: "absolute",
            top: "calc(100% + 4px)",
            left: 0,
            width: Math.max(width, 320),
            // ponytail: zIndex 提到 1000, Stats 下方 OverviewCard/图表 canvas 建立层叠上下文会盖住 zIndex:20 的浮层, 致选项视觉上叠在图表文字上
            zIndex: 1000,
            borderRadius: "var(--radius-sm)",
            padding: 8,
            display: "flex",
            flexDirection: "column",
            gap: 6,
            maxHeight: 320,
          }}
        >
          <input
            className="input"
            autoFocus
            style={{ fontSize: 14 }}
            placeholder={searchPlaceholder}
            value={search}
            onChange={e => setSearch(e.target.value)}
          />
          <div style={{ overflowY: "auto", maxHeight: 250, display: "flex", flexDirection: "column", gap: 2 }}>
            <FilterOption label={allLabel} active={value === ""} onClick={() => { onChange(""); setOpen(false); setSearch(""); }} />
            {filtered.length === 0 ? (
              <div style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "6px 8px" }}>{emptyLabel}</div>
            ) : (
              filtered.map(o => (
                <FilterOption
                  key={o.value}
                  label={o.label}
                  active={value === o.value}
                  onClick={() => { onChange(o.value); setOpen(false); setSearch(""); }}
                />
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function FilterOption({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        display: "block",
        width: "100%",
        textAlign: "left",
        border: "none",
        background: active ? "var(--bg-glass)" : "transparent",
        color: active ? "var(--accent)" : "var(--text-primary)",
        fontWeight: active ? 600 : 400,
        padding: "20px 14px",
        borderRadius: "var(--radius-sm)",
        cursor: "pointer",
        fontFamily: "inherit",
        fontSize: 15,
        lineHeight: 1.5,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </button>
  );
}
