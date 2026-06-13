// ── 共享 inline SVG 图标 ──
// 替代 UI 上的 emoji / 符号字形，跨平台渲染一致。
// 全部 stroke="currentColor"，通过 size / color 控制尺寸与颜色。

import React from "react";

export interface IconProps {
  size?: number;
  color?: string;
  strokeWidth?: number;
  style?: React.CSSProperties;
}

function base(size = 14, color = "currentColor", strokeWidth = 2, style?: React.CSSProperties) {
  return {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none" as const,
    stroke: color,
    strokeWidth,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    style: { flexShrink: 0, ...style },
  };
}

/** ✕ 关闭 / ✗ 失败 */
export const IconClose = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}><path d="M18 6 6 18M6 6l12 12" /></svg>
);

/** ✓ 勾选 / 成功 */
export const IconCheck = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}><path d="M20 6 9 17l-5-5" /></svg>
);

/** ☰ 菜单 / 拖拽手柄 */
export const IconMenu = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}><path d="M3 6h18M3 12h18M3 18h18" /></svg>
);

/** ✎ 编辑 */
export const IconEdit = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}>
    <path d="M12 20h9" />
    <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
  </svg>
);

/** ⚡ tokens */
export const IconBolt = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}><path d="M13 2 3 14h9l-1 8 10-12h-9l1-8Z" /></svg>
);

/** 💰 成本 */
export const IconCost = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7v10M9.5 9.2a2.2 2.2 0 0 1 2.5-1.2c1.2 0 2.2.8 2.2 1.9 0 2.6-4.7 1.3-4.7 4 0 1.1 1 1.9 2.2 1.9a2.2 2.2 0 0 0 2.5-1.2" />
  </svg>
);

/** 📦 缓存 */
export const IconPackage = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}>
    <path d="M21 8 12 3 3 8v8l9 5 9-5V8Z" />
    <path d="m3 8 9 5 9-5M12 13v8" />
  </svg>
);

/** 💳 余额 */
export const IconCard = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}>
    <rect x="2" y="5" width="20" height="14" rx="2" />
    <path d="M2 10h20" />
  </svg>
);

/** 🪙 配额 / 额度 */
export const IconCoin = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}>
    <circle cx="12" cy="12" r="9" />
    <circle cx="12" cy="12" r="4.5" />
  </svg>
);

/** ⏱ 重置倒计时 */
export const IconClock = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7v5l3 2" />
  </svg>
);

/** 🎨 主题 */
export const IconPalette = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}>
    <path d="M12 3a9 9 0 0 0 0 18c1 0 1.7-.8 1.7-1.8 0-.5-.2-.9-.5-1.2-.3-.3-.5-.7-.5-1.2 0-1 .8-1.8 1.8-1.8H16a5 5 0 0 0 5-5c0-4.4-4-8-9-8Z" />
    <circle cx="7.5" cy="10.5" r="1" fill="currentColor" stroke="none" />
    <circle cx="12" cy="7.5" r="1" fill="currentColor" stroke="none" />
    <circle cx="16.5" cy="10.5" r="1" fill="currentColor" stroke="none" />
  </svg>
);

/** 🌐 语言 */
export const IconGlobe = ({ size, color, strokeWidth, style }: IconProps) => (
  <svg {...base(size, color, strokeWidth, style)}>
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3a14 14 0 0 1 0 18 14 14 0 0 1 0-18Z" />
  </svg>
);

/** 🐕 应用 Logo（爪印） */
export const IconPaw = ({ size, color, style }: IconProps) => (
  <svg
    width={size ?? 14}
    height={size ?? 14}
    viewBox="0 0 24 24"
    fill={color ?? "currentColor"}
    style={{ flexShrink: 0, ...style }}
  >
    <ellipse cx="6" cy="11" rx="2" ry="2.6" />
    <ellipse cx="10" cy="7" rx="2" ry="2.8" />
    <ellipse cx="14" cy="7" rx="2" ry="2.8" />
    <ellipse cx="18" cy="11" rx="2" ry="2.6" />
    <path d="M12 12c2.8 0 5 2 5 4.3 0 1.7-1.4 2.7-3.1 2.7-.9 0-1.3-.4-1.9-.4s-1 .4-1.9.4C8.4 19 7 18 7 16.3 7 14 9.2 12 12 12Z" />
  </svg>
);
