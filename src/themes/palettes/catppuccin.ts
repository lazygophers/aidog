import type { PaletteDefinition } from "../types";

/**
 * Catppuccin 调色板（Latte/Mocha）
 * 柔和暖色调。
 */
export const catppuccin: PaletteDefinition = {
  id: "catppuccin",
  label: "theme.color.catppuccin",
  light: {
    "--bg-base": "#eff1f5",
    "--bg-elevated": "#e6e9ef",
    "--bg-floating": "#e6e9ef",
    "--bg-glass": "rgba(239, 241, 245, 0.88)",
    "--bg-glass-hover": "rgba(230, 233, 239, 0.95)",
    "--bg-surface": "#ffffff",
    "--text-primary": "#4c4f69",
    "--text-secondary": "#5c5f77",
    "--text-tertiary": "#8c8fa1",
    "--accent": "#1e66f5",
    "--accent-hover": "#1865db",
    "--accent-subtle": "rgba(30, 102, 245, 0.1)",
    "--accent-1": "#1e66f5",
    "--accent-2": "#8839ef",
    "--accent-3": "#40a02b",
    "--accent-4": "#fe640b",
    "--accent-5": "#d20f39",
    "--accent-gradient":
      "linear-gradient(135deg, #1e66f5 0%, #8839ef 50%, #40a02b 100%)",
    "--success": "#40a02b",
    "--danger": "#d20f39",
    "--border": "rgba(76, 79, 105, 0.1)",
    "--border-focus": "rgba(30, 102, 245, 0.4)",
    "--shadow-rgb": "76, 79, 105",
    "--glass-edge": "rgba(76, 79, 105, 0.08)",
  },
  dark: {
    "--bg-base": "#1e1e2e",
    "--bg-elevated": "#262637",
    "--bg-floating": "#262637",
    "--bg-glass": "rgba(30, 30, 46, 0.85)",
    "--bg-glass-hover": "rgba(38, 38, 55, 0.92)",
    "--bg-surface": "#313244",
    "--text-primary": "#cdd6f4",
    "--text-secondary": "#a6adc8",
    "--text-tertiary": "#7f849c",
    "--accent": "#cba6f7",
    "--accent-hover": "#b48eed",
    "--accent-subtle": "rgba(203, 166, 247, 0.12)",
    "--accent-1": "#cba6f7",
    "--accent-2": "#89b4fa",
    "--accent-3": "#a6e3a1",
    "--accent-4": "#fab387",
    "--accent-5": "#f38ba8",
    "--accent-gradient":
      "linear-gradient(135deg, #cba6f7 0%, #89b4fa 50%, #a6e3a1 100%)",
    "--success": "#a6e3a1",
    "--danger": "#f38ba8",
    "--border": "rgba(205, 214, 244, 0.08)",
    "--border-focus": "rgba(203, 166, 247, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(205, 214, 244, 0.06)",
  },
};
