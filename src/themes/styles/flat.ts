import type { StyleDefinition } from "../types";

/**
 * Flat 极简
 * 小圆角 + 无模糊 + 极轻单层阴影 + 实色边框。
 */
export const flat: StyleDefinition = {
  id: "flat",
  label: "theme.style.flat",
  light: {
    "--radius-sm": "6px",
    "--radius-md": "8px",
    "--radius-lg": "10px",
    "--radius-xl": "12px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1px solid var(--border)",
    "--shadow-sm": "0 1px 2px rgba(var(--shadow-rgb), 0.06)",
    "--shadow-md": "0 1px 2px rgba(var(--shadow-rgb), 0.06)",
    "--shadow-lg": "0 2px 4px rgba(var(--shadow-rgb), 0.08)",
    "--transition": "150ms ease",
    "--app-bg-overlay": "none",
  },
  dark: {
    "--radius-sm": "6px",
    "--radius-md": "8px",
    "--radius-lg": "10px",
    "--radius-xl": "12px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1px solid var(--border)",
    "--shadow-sm": "0 1px 2px rgba(var(--shadow-rgb), 0.25)",
    "--shadow-md": "0 1px 2px rgba(var(--shadow-rgb), 0.25)",
    "--shadow-lg": "0 2px 4px rgba(var(--shadow-rgb), 0.3)",
    "--transition": "150ms ease",
    "--app-bg-overlay": "none",
  },
};
