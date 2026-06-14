import type { StyleDefinition } from "../types";

/**
 * Soft 柔拟态（neumorphic）
 * 大圆角 + 无模糊 + 双向柔阴影（凸感）+ 极淡 glass-edge 边。
 */
export const soft: StyleDefinition = {
  id: "soft",
  label: "theme.style.soft",
  light: {
    "--radius-sm": "12px",
    "--radius-md": "16px",
    "--radius-lg": "22px",
    "--radius-xl": "28px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1px solid var(--glass-edge)",
    "--shadow-sm": "3px 3px 7px rgba(var(--shadow-rgb), 0.10), -3px -3px 7px rgba(255, 255, 255, 0.04)",
    "--shadow-md": "6px 6px 14px rgba(var(--shadow-rgb), 0.10), -6px -6px 14px rgba(255, 255, 255, 0.04)",
    "--shadow-lg": "10px 10px 24px rgba(var(--shadow-rgb), 0.12), -10px -10px 24px rgba(255, 255, 255, 0.05)",
    "--transition": "200ms ease",
  },
  dark: {
    "--radius-sm": "12px",
    "--radius-md": "16px",
    "--radius-lg": "22px",
    "--radius-xl": "28px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1px solid var(--glass-edge)",
    "--shadow-sm": "3px 3px 7px rgba(var(--shadow-rgb), 0.45), -3px -3px 7px rgba(255, 255, 255, 0.04)",
    "--shadow-md": "6px 6px 14px rgba(var(--shadow-rgb), 0.45), -6px -6px 14px rgba(255, 255, 255, 0.04)",
    "--shadow-lg": "10px 10px 24px rgba(var(--shadow-rgb), 0.5), -10px -10px 24px rgba(255, 255, 255, 0.05)",
    "--transition": "200ms ease",
  },
};
