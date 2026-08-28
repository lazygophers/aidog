// ── FilterDropdown ──
// 带搜索的筛选下拉，统一 Stats / Logs 的【平台/模型/分组】3 维筛选 UI。
// 视觉对齐 select.tsx 萤火虫玻璃规范（trigger 玻璃底 + open 流光光环 / popup 玻璃浮层 + slide / option accent-subtle 选中）。
// 数据源由各调用方传入（Stats: 有数据平台派生；Logs: 全平台 / 全分组 / 全模型）。

import { useState, useMemo } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";
import { makeRipple } from "../../utils/motion";
import { pinyinMatch } from "../../utils/pinyin";

export interface FilterDropdownProps {
  width: number;
  value: string;
  onChange: (v: string) => void;
  /** 「全部」选项标签（value="" 时显示） */
  allLabel: string;
  searchPlaceholder: string;
  /** 选项可带 searchTerms（registry 协议搜索词），搜索时跨语言/拼音匹配（label 之外） */
  options: Array<{ value: string; label: string; searchTerms?: string[] }>;
  /** 搜索无匹配时的空态文案 */
  emptyLabel: string;
}

export function FilterDropdown({ width, value, onChange, allLabel, searchPlaceholder, options, emptyLabel }: FilterDropdownProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");

  const filtered = useMemo(() => {
    const q = search.trim();
    if (!q) return options;
    return options.filter(o => pinyinMatch(q, o.label) || !!o.searchTerms?.some(t => pinyinMatch(q, t)));
  }, [options, search]);

  const current = options.find(o => o.value === value);

  return (
    <Popover open={open} onOpenChange={(o) => { setOpen(o); if (!o) setSearch(""); }}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          // 对齐 select.tsx trigger — bg-card 实色不透明 (覆盖 outline 默认 bg-background);
          // data-[state=open]:border-ring + shadow-[0_0_0_3px_var(--accent-subtle)] 流光光环 (与 SelectTrigger 同源)
          className="ripple group bg-card border-input hover:border-ring/50 hover:bg-accent data-[state=open]:border-ring data-[state=open]:shadow-[0_0_0_3px_var(--accent-subtle)] transition-colors duration-150"
          onClick={makeRipple}
          style={{ fontSize: 14, lineHeight: 1.5, height: 36, width, textAlign: "left", cursor: "pointer", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", justifyContent: "space-between" }}
          data-state={open ? "open" : "closed"}
        >
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1, minWidth: 0 }}>
            {current ? current.label : allLabel}
          </span>
          {/* 萤火虫箭头 (对齐 select.tsx rotate 180) — 用 CSS triangle 避免 lucide 依赖 */}
          <span
            aria-hidden
            style={{
              display: "inline-block",
              width: 0, height: 0,
              borderLeft: "4px solid transparent",
              borderRight: "4px solid transparent",
              borderTop: "5px solid var(--text-tertiary)",
              marginLeft: 8,
              flexShrink: 0,
              transition: "transform 200ms ease",
              transform: open ? "rotate(180deg)" : "rotate(0deg)",
            }}
          />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        // Popover 走 Radix Portal → 脱离 Stats canvas 层叠上下文, 无需自研 zIndex:1000
        // 对齐 select.tsx content — bg-popover 实色不透明 (覆盖 glass-surface 的半透 bg);
        // slide/fade/zoom 动画 PopoverContent 内置; glass-surface 加流光描边 hover 签名
        className="glass-surface bg-popover"
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
  // ponytail: 对齐 select.tsx SelectItem — selected 用 accent-subtle 底 + primary 文 + font-medium;
  // hover 走 Button ghost variant 的 hover:bg-accent (无障碍键盘 focus 同源)
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
        // 萤火虫选中态: accent-subtle 底 + primary 文 + font-medium (memory firefly-active-state-idiom)
        background: active ? "var(--accent-subtle)" : "transparent",
        color: active ? "var(--primary)" : "var(--text-primary)",
        fontWeight: active ? 500 : 400,
        padding: "6px 10px",
        borderRadius: "var(--radius-sm)",
        cursor: "pointer",
        fontFamily: "inherit",
        fontSize: 14,
        lineHeight: 1.5,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
        transition: "background-color 150ms ease, color 150ms ease",
      }}
    >
      {label}
    </Button>
  );
}
