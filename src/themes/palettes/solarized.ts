import type { PaletteDefinition } from "../types";

/**
 * Solarized 调色板
 * 经典科学配色，柔和暖黄基底。
 */
export const solarized: PaletteDefinition = {
  id: "solarized",
  label: "theme.color.solarized",
  light: {
    "--bg-base": "#fdf6e3",
    "--bg-elevated": "#eee8d5",
    "--bg-floating": "#eee8d5",
    "--bg-glass": "rgba(253, 246, 227, 0.9)",
    "--bg-glass-hover": "rgba(238, 232, 213, 0.95)",
    "--bg-surface": "#faf5e8",
    "--text-primary": "#073642",
    "--text-secondary": "#586e75",
    "--text-tertiary": "#93a1a1",
    "--accent": "#268bd2",
    "--accent-hover": "#1a6ba0",
    "--accent-subtle": "rgba(38, 139, 210, 0.1)",
    "--accent-1": "#268bd2",
    "--accent-2": "#2aa198",
    "--accent-3": "#859900",
    "--accent-4": "#b58900",
    "--accent-5": "#d33682",
    "--accent-gradient":
      "linear-gradient(135deg, #268bd2 0%, #2aa198 50%, #859900 100%)",
    "--success": "#859900",
    "--danger": "#dc322f",
    "--border": "rgba(7, 54, 66, 0.1)",
    "--border-focus": "rgba(38, 139, 210, 0.4)",
    "--shadow-rgb": "7, 54, 66",
    "--glass-edge": "rgba(7, 54, 66, 0.08)",
  },
  dark: {
    "--bg-base": "#002b36",
    "--bg-elevated": "#073642",
    "--bg-floating": "#073642",
    "--bg-glass": "rgba(0, 43, 54, 0.88)",
    "--bg-glass-hover": "rgba(7, 54, 66, 0.94)",
    "--bg-surface": "#0a3a47",
    "--text-primary": "#fdf6e3",
    "--text-secondary": "#93a1a1",
    "--text-tertiary": "#657b83",
    "--accent": "#b58900",
    "--accent-hover": "#cb9a00",
    "--accent-subtle": "rgba(181, 137, 0, 0.12)",
    "--accent-1": "#b58900",
    "--accent-2": "#268bd2",
    "--accent-3": "#2aa198",
    "--accent-4": "#859900",
    "--accent-5": "#d33682",
    "--accent-gradient":
      "linear-gradient(135deg, #b58900 0%, #2aa198 50%, #268bd2 100%)",
    "--success": "#859900",
    "--danger": "#dc322f",
    "--border": "rgba(253, 246, 227, 0.06)",
    "--border-focus": "rgba(181, 137, 0, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(253, 246, 227, 0.05)",
  },
};
