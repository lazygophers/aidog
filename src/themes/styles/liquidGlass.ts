import type { StyleDefinition } from "../types";

/**
 * Liquid Glass 结构（保留原值）
 * 多层柔阴影 + 大圆角 + 高 blur/saturate 毛玻璃。
 * shadow 用 rgba(var(--shadow-rgb), α) 由 palette 提供色，glass-border 用 var(--glass-edge)。
 */
export const liquidGlass: StyleDefinition = {
  id: "liquidGlass",
  label: "theme.style.liquidGlass",
  light: {
    "--radius-sm": "10px",
    "--radius-md": "14px",
    "--radius-lg": "20px",
    "--radius-xl": "28px",
    "--glass-blur": "24px",
    "--glass-saturate": "1.8",
    "--glass-border": "1px solid var(--glass-edge)",
    "--shadow-sm": "0 1px 3px rgba(var(--shadow-rgb), 0.04), 0 1px 2px rgba(var(--shadow-rgb), 0.02)",
    "--shadow-md": "0 4px 12px rgba(var(--shadow-rgb), 0.06), 0 1px 4px rgba(var(--shadow-rgb), 0.04)",
    "--shadow-lg": "0 12px 40px rgba(var(--shadow-rgb), 0.08), 0 4px 12px rgba(var(--shadow-rgb), 0.04)",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
  },
  dark: {
    "--radius-sm": "10px",
    "--radius-md": "14px",
    "--radius-lg": "20px",
    "--radius-xl": "28px",
    "--glass-blur": "24px",
    "--glass-saturate": "1.6",
    "--glass-border": "1px solid var(--glass-edge)",
    "--shadow-sm": "0 1px 3px rgba(var(--shadow-rgb), 0.3), 0 1px 2px rgba(var(--shadow-rgb), 0.2)",
    "--shadow-md": "0 4px 12px rgba(var(--shadow-rgb), 0.4), 0 1px 4px rgba(var(--shadow-rgb), 0.3)",
    "--shadow-lg": "0 12px 40px rgba(var(--shadow-rgb), 0.5), 0 4px 12px rgba(var(--shadow-rgb), 0.3)",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
  },
};
