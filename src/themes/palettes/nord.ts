import type { PaletteDefinition } from "../types";

/**
 * Nord 调色板
 * 北极蓝调色板，冷色系低对比度。
 */
export const nord: PaletteDefinition = {
  id: "nord",
  label: "theme.color.nord",
  light: {
    "--bg-base": "#eceff4",
    "--bg-elevated": "#e5e9f0",
    "--bg-floating": "#e5e9f0",
    "--bg-glass": "rgba(236, 239, 244, 0.85)",
    "--bg-glass-hover": "rgba(229, 233, 240, 0.95)",
    "--bg-surface": "#f5f7fa",
    "--text-primary": "#2e3440",
    "--text-secondary": "#4c566a",
    "--text-tertiary": "#81a1c1",
    "--accent": "#5e81ac",
    "--accent-hover": "#4c6b8a",
    "--accent-subtle": "rgba(94, 129, 172, 0.15)",
    "--success": "#a3be8c",
    "--danger": "#bf616a",
    "--border": "rgba(76, 86, 106, 0.15)",
    "--border-focus": "rgba(94, 129, 172, 0.5)",
    "--shadow-rgb": "46, 52, 64",
    "--glass-edge": "rgba(76, 86, 106, 0.12)",
  },
  dark: {
    "--bg-base": "#2e3440",
    "--bg-elevated": "#3b4252",
    "--bg-floating": "#3b4252",
    "--bg-glass": "rgba(59, 66, 82, 0.8)",
    "--bg-glass-hover": "rgba(67, 76, 94, 0.9)",
    "--bg-surface": "#434c5e",
    "--text-primary": "#eceff4",
    "--text-secondary": "#d8dee9",
    "--text-tertiary": "#81a1c1",
    "--accent": "#88c0d0",
    "--accent-hover": "#8fbcbb",
    "--accent-subtle": "rgba(136, 192, 208, 0.12)",
    "--success": "#a3be8c",
    "--danger": "#bf616a",
    "--border": "rgba(216, 222, 233, 0.08)",
    "--border-focus": "rgba(136, 192, 208, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(216, 222, 233, 0.06)",
  },
};
