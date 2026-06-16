import type { PaletteDefinition } from "../types";

/**
 * Tokyo Night 调色板（官方 canonical：Day=light / Night=dark）
 * 霓虹都市夜，深蓝紫基底。
 */
export const tokyoNight: PaletteDefinition = {
  id: "tokyoNight",
  label: "theme.color.tokyoNight",
  light: {
    "--bg-base": "#e1e2e7",
    "--bg-elevated": "#d0d5e3",
    "--bg-floating": "#d0d5e3",
    "--bg-glass": "rgba(225, 226, 231, 0.88)",
    "--bg-glass-hover": "rgba(208, 213, 227, 0.95)",
    "--bg-surface": "#e9e9ed",
    "--text-primary": "#343b58",
    "--text-secondary": "#6172b0",
    "--text-tertiary": "#848cb5",
    "--accent": "#2e7de9",
    "--accent-hover": "#1d5fc4",
    "--accent-subtle": "rgba(46, 125, 233, 0.12)",
    "--accent-1": "#2e7de9",
    "--accent-2": "#bb9af7",
    "--accent-3": "#7dcfff",
    "--accent-4": "#9ece6a",
    "--accent-5": "#e0af68",
    "--accent-gradient":
      "linear-gradient(135deg, #2e7de9 0%, #bb9af7 50%, #7dcfff 100%)",
    "--success": "#587539",
    "--danger": "#f52a65",
    "--border": "rgba(52, 59, 88, 0.12)",
    "--border-focus": "rgba(46, 125, 233, 0.45)",
    "--shadow-rgb": "52, 59, 88",
    "--glass-edge": "rgba(52, 59, 88, 0.1)",
  },
  dark: {
    "--bg-base": "#1a1b26",
    "--bg-elevated": "#24283b",
    "--bg-floating": "#24283b",
    "--bg-glass": "rgba(26, 27, 38, 0.85)",
    "--bg-glass-hover": "rgba(36, 40, 59, 0.92)",
    "--bg-surface": "#2a2e42",
    "--text-primary": "#c0caf5",
    "--text-secondary": "#9aa5ce",
    "--text-tertiary": "#717695",
    "--accent": "#7aa2f7",
    "--accent-hover": "#bb9af7",
    "--accent-subtle": "rgba(122, 162, 247, 0.12)",
    "--accent-1": "#7aa2f7",
    "--accent-2": "#bb9af7",
    "--accent-3": "#7dcfff",
    "--accent-4": "#9ece6a",
    "--accent-5": "#e0af68",
    "--accent-gradient":
      "linear-gradient(135deg, #7aa2f7 0%, #bb9af7 50%, #7dcfff 100%)",
    "--success": "#9ece6a",
    "--danger": "#f7768e",
    "--border": "rgba(192, 202, 245, 0.1)",
    "--border-focus": "rgba(122, 162, 247, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(192, 202, 245, 0.06)",
  },
};
