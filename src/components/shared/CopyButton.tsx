// ── shared CopyButton（D6 消重）──
// Groups 版超集（含 icon），Home 版子集（不传 icon）。视觉/逻辑与原两处完全一致。

import { useState } from "react";
import type { ReactNode } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

export interface CopyButtonProps {
  text: string;
  title?: string;
  label?: string;
  /** 自定义图标（传则替代默认复制/对勾 SVG）。 */
  icon?: ReactNode;
  size?: number;
}

/** Copy text to clipboard with a brief visual feedback */
export function CopyButton({ text, title, label, icon, size = 14 }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);
  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    // Tauri writeText 走权限系统（capabilities default.json allow-write-text），
    // WKWebView 无手势激活时 navigator.clipboard 被拒静默失败，Tauri 路径更可靠（参 ShareModal/SmartPasteModal）。
    writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };
  const hasContent = !!(label || icon);
  return (
    <button
      className={hasContent ? "btn btn-ghost" : "btn btn-ghost btn-icon"}
      onClick={handleCopy}
      title={title || text}
      style={{ position: "relative", flexShrink: 0, gap: hasContent ? 5 : 0, fontSize: hasContent ? 12 : undefined, padding: hasContent ? "4px 10px" : undefined }}
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
      {!icon && label && <span style={{ fontWeight: 500 }}>{label}</span>}
    </button>
  );
}
