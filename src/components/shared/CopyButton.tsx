// ── shared CopyButton（D6 消重）──
// Groups 版超集（含 icon / menu），Home 版子集（不传 icon）。视觉/逻辑与原两处完全一致。

import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "@/components/ui/dropdown-menu";

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

/** Copy text to clipboard with a brief visual feedback; optional dropdown menu mode. */
export function CopyButton({
  text, title, label, icon, size = 14,
  menu, defaultLabel, hoverLabel,
}: CopyButtonProps) {
  const [copied, setCopied] = useState(false);
  const [open, setOpen] = useState(false);
  const [hovered, setHovered] = useState(false);
  // hover menu 延迟关闭 timer（button↔menu 移动间隙靠 delay 兜底）
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
    if (isMenu) return; // menu 模式由 DropdownMenuTrigger 掌管开合
    runCopy(text);
  };

  const hasContent = !!(label || icon || (isMenu && defaultLabel));

  // menu 模式：默认态/悬浮态文案切换
  const effectiveLabel = isMenu
    ? (hovered ? (hoverLabel ?? defaultLabel) : defaultLabel)
    : label;

  const triggerBtn = (
    <Button
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
  );

  if (!isMenu || !menu) return triggerBtn;

  return (
    // modal={false} 保 hover-open UX（不锁背景交互）; DropdownMenu 走 Radix Portal 脱离 transform 祖先
    <DropdownMenu open={open} onOpenChange={setOpen} modal={false}>
      <DropdownMenuTrigger asChild>{triggerBtn}</DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        style={{ width: MENU_WIDTH }}
        onMouseEnter={cancelClose}
        onMouseLeave={() => scheduleClose(120)}
      >
        {menu.map(item => (
          <DropdownMenuItem
            key={item.key}
            onSelect={() => runCopy(item.text)}
            style={{ gap: 8, fontSize: 12, whiteSpace: "nowrap" }}
          >
            {item.icon ?? <span style={{ width: 14, display: "inline-block" }} />}
            <span>{item.label}</span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
