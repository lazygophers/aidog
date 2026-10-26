// CommandPalette — G3 顶栏 ⌘K 命令面板（设计稿全局骨架 kbd 元素的实做）。
// 导航跳转最小面：输入过滤 BASE_NAV 全部可跳项（含 settings 子页），↑↓ 选择、Enter 跳转。
// 不引新依赖（无 cmdk）：Dialog + 自过滤列表，idiom 同 shadcn Dialog 消费者。
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { NavItem } from "./Sidebar";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";

/** 展平导航树为可跳项（顶层项 + settings 子页）。 */
function flattenNav(navItems: NavItem[]): { id: string; labelKey: string }[] {
  const out: { id: string; labelKey: string }[] = [];
  for (const it of navItems) {
    out.push({ id: it.id, labelKey: it.labelKey });
    if (it.children) for (const c of it.children) out.push({ id: c.id, labelKey: c.labelKey });
  }
  return out;
}

export function CommandPalette(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  navItems: NavItem[];
  activeId: string;
  onNavigate: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  // 每次打开重置输入与选中位
  useEffect(() => {
    if (props.open) { setQuery(""); setIndex(0); }
  }, [props.open]);

  const all = useMemo(() => flattenNav(props.navItems), [props.navItems]);
  const hits = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return all;
    return all.filter(it => t(it.labelKey).toLowerCase().includes(q) || it.id.includes(q));
  }, [all, query, t]);

  const jump = (id: string) => {
    props.onOpenChange(false);
    props.onNavigate(id);
  };

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent data-testid="command-palette" style={{ maxWidth: 480, padding: 0, gap: 0, top: "18%", transform: "translate(-50%, 0)" }} aria-describedby={undefined}>
        <DialogHeader style={{ padding: 0, borderBottom: "1px solid var(--border)" }}>
          <DialogTitle style={{ position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)" }}>{t("topbar.search", "搜索（⌘K）")}</DialogTitle>
          <input
            ref={inputRef}
            data-testid="command-palette-input"
            autoFocus
            value={query}
            onChange={e => { setQuery(e.target.value); setIndex(0); }}
            onKeyDown={e => {
              if (e.key === "ArrowDown") { e.preventDefault(); setIndex(i => Math.min(i + 1, hits.length - 1)); }
              else if (e.key === "ArrowUp") { e.preventDefault(); setIndex(i => Math.max(i - 1, 0)); }
              else if (e.key === "Enter" && hits[index]) { e.preventDefault(); jump(hits[index].id); }
            }}
            placeholder={t("topbar.palettePlaceholder", "跳转到…")}
            style={{
              width: "100%", background: "transparent", border: "none", outline: "none",
              fontSize: 14, padding: "14px 16px", color: "var(--foreground)",
            }}
          />
        </DialogHeader>
        <div style={{ maxHeight: 320, overflow: "auto", padding: 6 }}>
          {hits.length === 0 ? (
            <div data-testid="command-palette-empty" style={{ fontSize: 12.5, color: "var(--text-tertiary)", padding: "16px 10px", textAlign: "center" }}>
              {t("topbar.paletteNoResults", "无匹配结果")}
            </div>
          ) : hits.map((it, i) => {
            const on = it.id === props.activeId;
            const sel = i === index;
            return (
              <div
                key={it.id}
                data-nav-id={it.id}
                data-state={sel ? "selected" : on ? "active" : "idle"}
                onMouseEnter={() => setIndex(i)}
                onClick={() => jump(it.id)}
                style={{
                  display: "flex", alignItems: "center", gap: 8, cursor: "pointer",
                  fontSize: 13, padding: "8px 10px", borderRadius: "var(--radius-sm)",
                  background: sel ? "var(--accent-subtle)" : "transparent",
                  color: on ? "var(--primary)" : "var(--foreground)",
                  fontWeight: on ? 600 : 400,
                }}>
                <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{t(it.labelKey)}</span>
                {it.id.startsWith("settings/") && (
                  <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10, color: "var(--text-tertiary)" }}>settings</span>
                )}
                {sel && <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: 10, color: "var(--text-tertiary)" }}>↵</span>}
              </div>
            );
          })}
        </div>
      </DialogContent>
    </Dialog>
  );
}
