import type { StyleDefinition } from "../types";

/**
 * Bento 便当
 * 大圆角 + 无模糊 + 粗实色分隔边框 + 轻投影，模块化便当格质感。
 * --app-bg-overlay 默认 none。
 */
export const bento: StyleDefinition = {
  id: "bento",
  label: "theme.style.bento",
  light: {
    "--radius-sm": "16px",
    "--radius-md": "20px",
    "--radius-lg": "26px",
    "--radius-xl": "32px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1.5px solid var(--border)",
    "--shadow-sm": "0 2px 8px rgba(var(--shadow-rgb), 0.10)",
    "--shadow-md": "0 2px 8px rgba(var(--shadow-rgb), 0.10)",
    "--shadow-lg": "0 4px 16px rgba(var(--shadow-rgb), 0.12)",
    "--transition": "200ms ease",
    "--app-bg-overlay": "none",
  },
  dark: {
    "--radius-sm": "16px",
    "--radius-md": "20px",
    "--radius-lg": "26px",
    "--radius-xl": "32px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1.5px solid var(--border)",
    "--shadow-sm": "0 2px 8px rgba(var(--shadow-rgb), 0.4)",
    "--shadow-md": "0 2px 8px rgba(var(--shadow-rgb), 0.4)",
    "--shadow-lg": "0 4px 16px rgba(var(--shadow-rgb), 0.45)",
    "--transition": "200ms ease",
    "--app-bg-overlay": "none",
  },
};
