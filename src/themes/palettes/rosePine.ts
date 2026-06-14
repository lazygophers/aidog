import type { PaletteDefinition } from "../types";

/**
 * Rosé Pine 调色板（官方 canonical：Dawn=light / Main=dark）
 * 柔和玫瑰木质暖调，低对比。
 */
export const rosePine: PaletteDefinition = {
  id: "rosePine",
  label: "theme.color.rosePine",
  light: {
    "--bg-base": "#faf4ed",
    "--bg-elevated": "#fffaf3",
    "--bg-floating": "#fffaf3",
    "--bg-glass": "rgba(250, 244, 237, 0.88)",
    "--bg-glass-hover": "rgba(255, 250, 243, 0.95)",
    "--bg-surface": "#fffaf3",
    "--text-primary": "#575279",
    "--text-secondary": "#797593",
    "--text-tertiary": "#9893a5",
    "--accent": "#d7827e",
    "--accent-hover": "#c4615c",
    "--accent-subtle": "rgba(215, 130, 126, 0.13)",
    "--success": "#286983",
    "--danger": "#b4637a",
    "--border": "rgba(87, 82, 121, 0.12)",
    "--border-focus": "rgba(215, 130, 126, 0.45)",
    "--shadow-rgb": "87, 82, 121",
    "--glass-edge": "rgba(87, 82, 121, 0.1)",
  },
  dark: {
    "--bg-base": "#191724",
    "--bg-elevated": "#1f1d2e",
    "--bg-floating": "#1f1d2e",
    "--bg-glass": "rgba(25, 23, 36, 0.85)",
    "--bg-glass-hover": "rgba(31, 29, 46, 0.92)",
    "--bg-surface": "#26233a",
    "--text-primary": "#e0def4",
    "--text-secondary": "#908caa",
    "--text-tertiary": "#6e6a86",
    "--accent": "#ebbcba",
    "--accent-hover": "#f0cfcd",
    "--accent-subtle": "rgba(235, 188, 186, 0.12)",
    "--success": "#31748f",
    "--danger": "#eb6f92",
    "--border": "rgba(224, 222, 244, 0.1)",
    "--border-focus": "rgba(235, 188, 186, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(224, 222, 244, 0.06)",
  },
};
