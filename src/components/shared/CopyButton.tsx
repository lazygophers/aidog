// ── shared CopyButton（D6 消重）──
// Groups 版超集（含 icon / menu），Home 版子集（不传 icon）。视觉/逻辑与原两处完全一致。

import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Button } from "@/components/ui/button";

export interface CopyMenuItem {
  key: string;
  label: string;
  text: string;
  icon?: ReactNode;
}

export interface CopyButtonProps {
  text: string;
  title?: string;
  label?: string;
  /** 自定义图标（传则替代默认复制/对勾 SVG）。 */
  icon?: ReactNode;
  size?: number;
  /** 传入则 click 弹菜单（替代直接复制）。 */
  menu?: CopyMenuItem[];
  /** menu 模式默认态文案。 */
  defaultLabel?: string;
  /** menu 模式 hover 态文案（视觉切换，click 行为不变）。 */
  hoverLabel?: string;
}

const MENU_WIDTH = 200;
const MENU_EST_HEIGHT = 116; // 3 项 × ~36px + padding，仅供边界翻转估算

/** Copy text to clipboard with a brief visual feedback; optional dropdown menu mode. */
export function CopyButton({
  text, title, label, icon, size = 14,
  menu, defaultLabel, hoverLabel,
}: CopyButtonProps) {
  const [copied, setCopied] = useState(false);
  const [open, setOpen] = useState(false);
  const [hovered, setHovered] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  // hover menu 延迟关闭 timer（portal 到 body，button↔menu 移动间隙靠 delay 兜底）
  const closeTimer = useRef<number | null>(null);

  const isMenu = !!menu;

  const cancelClose = () => {
    if (closeTimer.current !== null) {
      clearTimeout(closeTimer.current);
      closeTimer.current = null;
    }
  };

  const scheduleClose = (delay: number) => {
    cancelClose();
    closeTimer.current = window.setTimeout(() => {
      setOpen(false);
      closeTimer.current = null;
    }, delay);
  };

  // 卸载时清理 timer，防内存泄漏
  useEffect(() => {
    return () => cancelClose();
  }, []);

  const runCopy = (t: string) => {
    // Tauri writeText 走权限系统（capabilities default.json allow-write-text），
    // WKWebView 无手势激活时 navigator.clipboard 被拒静默失败，Tauri 路径更可靠（参 ShareModal/SmartPasteModal）。
    writeText(t).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };

  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (isMenu) {
      setOpen(o => !o);
      return;
    }
    runCopy(text);
  };

  // 关闭：外点 + Esc（仅 open 时挂载监听）
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (menuRef.current?.contains(t)) return;
      if (btnRef.current?.contains(t)) return;
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const hasContent = !!(label || icon || (isMenu && defaultLabel));

  // menu 模式：默认态/悬浮态文案切换
  const effectiveLabel = isMenu
    ? (hovered ? (hoverLabel ?? defaultLabel) : defaultLabel)
    : label;

  // 菜单定位：右对齐按钮，下方不够则上方翻转
  let menuStyle: React.CSSProperties = {};
  if (open && btnRef.current) {
    const rect = btnRef.current.getBoundingClientRect();
    const flipUp = rect.bottom + MENU_EST_HEIGHT + 4 > window.innerHeight;
    menuStyle = {
      position: "fixed",
      top: flipUp ? Math.max(4, rect.top - MENU_EST_HEIGHT - 4) : rect.bottom + 4,
      left: Math.max(4, rect.right - MENU_WIDTH),
      width: MENU_WIDTH,
      zIndex: 1000,
    };
  }

  return (
    <>
      <Button
        ref={btnRef}
        variant="ghost"
        size={hasContent ? "default" : "icon"}
        onClick={handleCopy}
        onMouseEnter={isMenu ? () => {
          cancelClose();
          setHovered(true);
          setOpen(true);
        } : undefined}
        onMouseLeave={isMenu ? () => {
          scheduleClose(120);
          setHovered(false);
        } : undefined}
        title={title || text}
        style={{ position: "relative", flexShrink: 0, height: "auto", gap: hasContent ? 5 : 0, fontSize: hasContent ? 12 : undefined, padding: hasContent ? "4px 10px" : undefined }}
      >
        {icon ? icon : copied ? (
          <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M20 6L9 17l-5-5" />
          </svg>
        ) : (
          <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
          </svg>
        )}
        {!icon && effectiveLabel && <span style={{ fontWeight: 500 }}>{effectiveLabel}</span>}
      </Button>

      {open && isMenu && menu && createPortal(
        <div
          ref={menuRef}
          className="glass-elevated"
          style={{
            ...menuStyle,
            padding: "4px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--border)",
            boxShadow: "var(--shadow-md, 0 8px 24px rgba(0,0,0,0.18))",
            display: "flex",
            flexDirection: "column",
            gap: 2,
          }}
          onClick={e => e.stopPropagation()}
          onMouseEnter={cancelClose}
          onMouseLeave={() => scheduleClose(120)}
        >
          {menu.map(item => (
            <Button
              variant="ghost"
              key={item.key}
              style={{ justifyContent: "flex-start", gap: 8, fontSize: 12, padding: "6px 10px", width: "100%", height: "auto", whiteSpace: "nowrap" }}
              onClick={(e) => {
                e.stopPropagation();
                runCopy(item.text);
                setOpen(false);
              }}
            >
              {item.icon ?? <span style={{ width: 14, display: "inline-block" }} />}
              <span>{item.label}</span>
            </Button>
          ))}
        </div>,
        document.body,
      )}
    </>
  );
}
