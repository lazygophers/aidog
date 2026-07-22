import React, { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { Protocol } from "../../services/api";
import { pinyinMatch } from "../../utils/pinyin";
import type { ProtocolOption } from "./constants";
import { buildProtocolsFromPresets, getProtocolLabel } from "./defaults";
import { ProtocolLogo } from "./ProtocolLogo";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

/** 可搜索的协议选择器（支持拼音模糊匹配 + Tab/方向键键盘导航） */
export function SearchableProtocolSelect({
  value, codingPlan, onChange,
}: {
  value: Protocol;
  codingPlan: boolean;
  onChange: (proto: Protocol, codingPlan?: boolean) => void | Promise<void>;
}) {
  const { t, i18n } = useTranslation();
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  // 仅键盘导航/打开下拉时才自动滚动；鼠标 hover 改高亮不滚动（避免滚动→hover→滚动死循环）
  const autoScrollRef = useRef(false);

  // ProtocolOption 列表（async 派生自 platform-presets.json）。
  // 首帧 fallback 空数组（不崩），加载后 setState 重渲染。
  const [protocols, setProtocols] = useState<ProtocolOption[]>([]);
  // 协议本地化 label 映射（value → 按 i18n.language 取 JSON name）。
  // fallback: locale 缺失 → en-US → value key。docPromise 单次 RPC，切语言重拉。
  // ponytail: value 作 key（coding 变体共用同 value 的 name，调用方追加 Coding 后缀）
  const [labelMap, setLabelMap] = useState<Record<string, string>>({});
  useEffect(() => {
    let cancelled = false;
    // 切语言或挂载时拉取；不阻塞渲染，先显空 fallback 后异步替换
    (async () => {
      const list = await buildProtocolsFromPresets(i18n.language);
      if (cancelled) return;
      setProtocols(list);
      // labelMap 直接从 list派生（label 已按 locale 解析），免再发一次 RPC。
      const entries = await Promise.all(
        Array.from(new Set(list.map(p => p.value)))
          .map(async v => [v, await getProtocolLabel(v, i18n.language)] as const),
      );
      if (!cancelled) setLabelMap(Object.fromEntries(entries));
    })();
    return () => { cancelled = true; };
  }, [i18n.language]);

  // 当前选中项的显示文本（优先本地化，coding plan 追加后缀）
  // ponytail: JSON name 不区分 coding plan，硬编码 fallback label 已含 "Coding Plan" 后缀时直接用
  const selectedBase = labelMap[value] || value;
  const selectedLabel = codingPlan && !/Coding/i.test(selectedBase)
    ? `${selectedBase} Coding Plan`
    : selectedBase;

  // 按拼音/关键词过滤 — 用本地化 label（zh-Hans name 含中文，拼音搜索自动生效）
  // ponytail: labelOf 同步取，filtered 渲染共享
  const labelOf = (p: ProtocolOption): string => {
    const base = labelMap[p.value] || p.label;
    return p.codingPlan && !/Coding/i.test(base) ? `${base} Coding Plan` : base;
  };
  const filtered = protocols.filter(p => {
    if (!query.trim()) return true;
    if (pinyinMatch(query, labelOf(p))) return true;
    if (pinyinMatch(query, p.label)) return true; // fallback 英文兜底也参与匹配
    if (p.keywords?.some(kw => pinyinMatch(query, kw))) return true;
    if (pinyinMatch(query, p.value)) return true;
    return false;
  });

  // Tab 切换：在完整列表中循环到下一个平台
  const tabCycle = (shift: boolean) => {
    const idx = protocols.findIndex(p => p.value === value && !!p.codingPlan === codingPlan);
    if (idx < 0 || protocols.length === 0) return;
    const next = shift
      ? (idx - 1 + protocols.length) % protocols.length
      : (idx + 1) % protocols.length;
    const target = protocols[next];
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
        <span style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0, color: "var(--text-primary)", fontSize: 13 }}>
          <ProtocolLogo protocol={value} size={20} />
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{selectedLabel}</span>
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
          <Input
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
                <Button
                  key={`${p.value}-${p.codingPlan ? 1 : 0}`}
                  type="button"
                  variant="ghost"
                  style={{
                    display: "flex", width: "100%", justifyContent: "flex-start", textAlign: "left",
                    padding: "7px 12px", fontSize: 13, height: "auto",
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
                  <ProtocolLogo protocol={p.value} size={20} />
                  <span style={{ marginLeft: 8, flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {labelOf(p)}
                  </span>
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
                </Button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
