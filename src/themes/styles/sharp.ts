import type { StyleDefinition } from "../types";

/**
 * Sharp 硬朗
 * 近无圆角 + 无模糊 + 硬投影 + 粗实色边框（text-primary）。
 */
export const sharp: StyleDefinition = {
  id: "sharp",
  label: "theme.style.sharp",
  light: {
    "--radius-sm": "0px",
    "--radius-md": "0px",
    "--radius-lg": "2px",
    "--radius-xl": "2px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1.5px solid var(--text-primary)",
    "--shadow-sm": "2px 2px 0 rgba(var(--shadow-rgb), 0.85)",
    "--shadow-md": "3px 3px 0 rgba(var(--shadow-rgb), 0.85)",
    "--shadow-lg": "5px 5px 0 rgba(var(--shadow-rgb), 0.85)",
    "--transition": "100ms linear",
    "--app-bg-overlay": "none",
  },
  dark: {
    "--radius-sm": "0px",
    "--radius-md": "0px",
    "--radius-lg": "2px",
    "--radius-xl": "2px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1.5px solid var(--text-primary)",
    "--shadow-sm": "2px 2px 0 rgba(var(--shadow-rgb), 0.85)",
    "--shadow-md": "3px 3px 0 rgba(var(--shadow-rgb), 0.85)",
    "--shadow-lg": "5px 5px 0 rgba(var(--shadow-rgb), 0.85)",
    "--transition": "100ms linear",
    "--app-bg-overlay": "none",
  },
};
