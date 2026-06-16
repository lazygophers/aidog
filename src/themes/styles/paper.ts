import type { StyleDefinition } from "../types";

/**
 * Paper 纸感
 * 小圆角 + 无模糊 + 柔纸双层投影（近+远柔光）+ 实色边框。
 * --app-bg-overlay 默认 none（不叠加流光），保持纸面纯净。
 */
export const paper: StyleDefinition = {
  id: "paper",
  label: "theme.style.paper",
  light: {
    "--radius-sm": "4px",
    "--radius-md": "6px",
    "--radius-lg": "8px",
    "--radius-xl": "10px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1px solid var(--border)",
    "--shadow-sm": "0 1px 3px rgba(var(--shadow-rgb), 0.10), 0 8px 24px rgba(var(--shadow-rgb), 0.04)",
    "--shadow-md": "0 1px 3px rgba(var(--shadow-rgb), 0.10), 0 8px 24px rgba(var(--shadow-rgb), 0.04)",
    "--shadow-lg": "0 2px 4px rgba(var(--shadow-rgb), 0.12), 0 12px 32px rgba(var(--shadow-rgb), 0.06)",
    "--transition": "200ms ease",
    "--app-bg-overlay": "none",
  },
  dark: {
    "--radius-sm": "4px",
    "--radius-md": "6px",
    "--radius-lg": "8px",
    "--radius-xl": "10px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1px solid var(--border)",
    "--shadow-sm": "0 1px 3px rgba(var(--shadow-rgb), 0.4), 0 8px 24px rgba(var(--shadow-rgb), 0.2)",
    "--shadow-md": "0 1px 3px rgba(var(--shadow-rgb), 0.4), 0 8px 24px rgba(var(--shadow-rgb), 0.2)",
    "--shadow-lg": "0 2px 4px rgba(var(--shadow-rgb), 0.45), 0 12px 32px rgba(var(--shadow-rgb), 0.25)",
    "--transition": "200ms ease",
    "--app-bg-overlay": "none",
  },
};
