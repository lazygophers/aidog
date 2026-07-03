import React, { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { Protocol } from "../../services/api";
import { pinyinMatch } from "../../utils/pinyin";
import { PROTOCOLS, PROTOCOL_LABELS, PROTOCOL_COLORS } from "./constants";

/** 可搜索的协议选择器（支持拼音模糊匹配 + Tab/方向键键盘导航） */
export function SearchableProtocolSelect({
  value, codingPlan, onChange,
}: {
  value: Protocol;
  codingPlan: boolean;
  onChange: (proto: Protocol, codingPlan?: boolean) => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  // 仅键盘导航/打开下拉时才自动滚动；鼠标 hover 改高亮不滚动（避免滚动→hover→滚动死循环）
  const autoScrollRef = useRef(false);

  // 当前选中项的显示文本
  const selectedLabel = PROTOCOLS.find(
    p => p.value === value && !!p.codingPlan === codingPlan
  )?.label || PROTOCOL_LABELS[value];

  // 按拼音/关键词过滤
  const filtered = PROTOCOLS.filter(p => {
    if (!query.trim()) return true;
    if (pinyinMatch(query, p.label)) return true;
    if (p.keywords?.some(kw => pinyinMatch(query, kw))) return true;
    if (pinyinMatch(query, p.value)) return true;
    return false;
  });

  // Tab 切换：在完整列表中循环到下一个平台
  const tabCycle = (shift: boolean) => {
    const idx = PROTOCOLS.findIndex(p => p.value === value && !!p.codingPlan === codingPlan);
    const next = shift
      ? (idx - 1 + PROTOCOLS.length) % PROTOCOLS.length
      : (idx + 1) % PROTOCOLS.length;
    const target = PROTOCOLS[next];
    onChange(target.value, target.codingPlan);
  };

  // 打开下拉时定位到当前选中项
  const openDropdown = () => {
    setOpen(true);
    setQuery("");
    const idx = filtered.findIndex(p => p.value === value && !!p.codingPlan === codingPlan);
    autoScrollRef.current = true;
    setHighlightedIndex(idx >= 0 ? idx : 0);
    setTimeout(() => inputRef.current?.focus(), 0);
  };

  // 高亮项变化时滚动到可见区域：仅消费 autoScrollRef（键盘/打开）才滚，鼠标 hover 不滚
  useEffect(() => {
    if (!open || !scrollRef.current) return;
    if (!autoScrollRef.current) return;
    autoScrollRef.current = false;
    const btn = scrollRef.current.children[highlightedIndex] as HTMLElement | undefined;
    btn?.scrollIntoView({ block: "nearest" });
  }, [highlightedIndex, open, filtered.length]);

  // 搜索内容变化时重置高亮到第一项
  useEffect(() => { setHighlightedIndex(0); }, [query]);

  /** 触发器键盘（下拉关闭态） */
  const handleTriggerKey = (e: React.KeyboardEvent) => {
    if (open) return;
    switch (e.key) {
      case "Tab":
        e.preventDefault();
        tabCycle(e.shiftKey);
        break;
      case "ArrowDown":
      case "Enter":
      case " ":
        e.preventDefault();
        openDropdown();
        break;
    }
  };

  /** 搜索输入键盘（下拉打开态） */
  const handleInputKey = (e: React.KeyboardEvent) => {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        autoScrollRef.current = true;
        setHighlightedIndex(i => Math.min(i + 1, filtered.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        autoScrollRef.current = true;
        setHighlightedIndex(i => Math.max(i - 1, 0));
        break;
      case "Enter":
        e.preventDefault();
        if (filtered[highlightedIndex]) {
          onChange(filtered[highlightedIndex].value, filtered[highlightedIndex].codingPlan);
          setOpen(false);
          setQuery("");
        }
        break;
      case "Escape":
        e.preventDefault();
        setOpen(false);
        setQuery("");
        break;
    }
  };

  return (
    <div style={{ position: "relative" }} ref={listRef}>
      {/* 触发器：点击展开下拉，展示当前选中值 */}
      <div
        className="input"
        tabIndex={0}
        style={{
          display: "flex", alignItems: "center", justifyContent: "space-between",
          cursor: "pointer", userSelect: "none",
        }}
        onClick={() => {
          if (!open) { openDropdown(); }
          else { setOpen(false); setQuery(""); }
        }}
        onKeyDown={handleTriggerKey}
      >
        <span style={{ color: "var(--text-primary)", fontSize: 13 }}>
          {selectedLabel}
        </span>
        <span style={{
          fontSize: 10, color: "var(--text-tertiary)",
          transition: "transform 150ms ease",
          transform: open ? "rotate(180deg)" : "rotate(0deg)",
        }}>▼</span>
      </div>

      {/* 下拉面板 */}
      {open && (
        <div
          className="glass-elevated"
          style={{
            position: "absolute", top: "100%", left: 0, right: 0,
            marginTop: 4, zIndex: 100, padding: 4,
            animation: "fadeIn 150ms ease both",
          }}
        >
          {/* 搜索输入 */}
          <input
            ref={inputRef}
            className="input"
            placeholder={t("platform.searchPlaceholder", "搜索平台...")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoFocus
            style={{ fontSize: 12, padding: "6px 10px", marginBottom: 4, width: "100%" }}
            onBlur={() => { setTimeout(() => { setOpen(false); setQuery(""); }, 150); }}
            onKeyDown={handleInputKey}
          />
          {/* 选项列表 */}
          <div style={{ maxHeight: 256, overflowY: "auto" }} ref={scrollRef}>
            {filtered.length === 0 && (
              <div style={{ padding: "8px 12px", fontSize: 12, color: "var(--text-tertiary)" }}>
                No match
              </div>
            )}
            {filtered.map((p, idx) => {
              const isActive = p.value === value && !!p.codingPlan === codingPlan;
              const isHighlighted = idx === highlightedIndex;
              return (
                <button
                  key={`${p.value}-${p.codingPlan ? 1 : 0}`}
                  type="button"
                  className="btn btn-ghost"
                  style={{
                    width: "100%", justifyContent: "flex-start",
                    padding: "7px 12px", fontSize: 13,
                    fontWeight: isActive ? 600 : 400,
                    color: isActive ? "var(--accent)" : "var(--text-primary)",
                    background: isHighlighted
                      ? "var(--accent-subtle)"
                      : isActive ? "var(--accent-subtle)" : "transparent",
                    borderRadius: "var(--radius-sm)",
                    outline: isHighlighted && !isActive ? "1px solid var(--accent)" : "none",
                  }}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    onChange(p.value, p.codingPlan);
                    setOpen(false);
                    setQuery("");
                  }}
                  onMouseEnter={() => setHighlightedIndex(idx)}
                >
                  <span style={{
                    display: "inline-block", padding: "1px 6px", borderRadius: "var(--radius-sm)",
                    background: `${PROTOCOL_COLORS[p.value] || "var(--accent)"}20`,
                    color: PROTOCOL_COLORS[p.value] || "var(--accent)",
                    fontSize: 10, fontWeight: 700, marginRight: 8,
                  }}>
                    {p.value.slice(0, 2).toUpperCase()}
                  </span>
                  {p.label}
                  {p.codingPlan && (
                    <span style={{
                      marginLeft: 6, padding: "1px 5px", borderRadius: "var(--radius-sm)",
                      background: "var(--color-success, var(--color-success))20",
                      color: "var(--color-success, var(--color-success))",
                      fontSize: 10, fontWeight: 600,
                    }}>
                      Code
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
