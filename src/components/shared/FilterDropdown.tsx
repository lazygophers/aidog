// ── FilterDropdown ──
// 带搜索的筛选下拉，统一 Stats / Logs 的【平台/模型/分组】3 维筛选 UI。
// 从 Stats 本地 SearchableFilter 提炼而来，保留 glass-elevated + zIndex 1000 + search。
// 数据源由各调用方传入（Stats: 有数据平台派生；Logs: 全平台 / 全分组 / 全模型）。

import { useState, useMemo } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";

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

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return options;
    return options.filter(o => o.label.toLowerCase().includes(q));
  }, [options, search]);

  const current = options.find(o => o.value === value);

  return (
    <Popover open={open} onOpenChange={(o) => { setOpen(o); if (!o) setSearch(""); }}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          // ponytail: lineHeight/height 显式锁, 防 .input transition:all + 继承链抖动致 trigger 文字错位
          style={{ fontSize: 14, lineHeight: 1.5, height: 36, width, textAlign: "left", cursor: "pointer", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", justifyContent: "flex-start" }}
        >
          {current ? current.label : allLabel}
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        // Popover 走 Radix Portal → 脱离 Stats canvas 层叠上下文, 无需自研 zIndex:1000
        style={{ width: Math.max(width, 320), padding: 8, display: "flex", flexDirection: "column", gap: 6, maxHeight: 320 }}
      >
        <Input
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
      </PopoverContent>
    </Popover>
  );
}

function FilterOption({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <Button
      variant="ghost"
      onClick={onClick}
      style={{
        display: "block",
        width: "100%",
        height: "auto",
        textAlign: "left",
        justifyContent: "flex-start",
        background: active ? "var(--bg-glass)" : "transparent",
        color: active ? "var(--primary)" : "var(--text-primary)",
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
    </Button>
  );
}
