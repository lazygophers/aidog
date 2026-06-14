import type { PaletteDefinition } from "../types";

/**
 * Apple Blue 调色板（从原 liquidGlass 主题抽色）
 * 苹果系统蓝，半透明白/暗面。
 */
export const appleBlue: PaletteDefinition = {
  id: "appleBlue",
  label: "theme.color.appleBlue",
  light: {
    "--bg-base": "#f0f0f3",
    "--bg-elevated": "rgba(255, 255, 255, 0.82)",
    "--bg-floating": "#ffffff",
    "--bg-glass": "rgba(255, 255, 255, 0.55)",
    "--bg-glass-hover": "rgba(255, 255, 255, 0.72)",
    "--bg-surface": "rgba(255, 255, 255, 0.88)",
    "--text-primary": "rgba(0, 0, 0, 0.88)",
    "--text-secondary": "rgba(0, 0, 0, 0.5)",
    "--text-tertiary": "rgba(0, 0, 0, 0.3)",
    "--accent": "#007AFF",
    "--accent-hover": "#0056CC",
    "--accent-subtle": "rgba(0, 122, 255, 0.1)",
    "--accent-1": "#007AFF",
    "--accent-2": "#34C759",
    "--accent-3": "#FF9500",
    "--accent-4": "#FF3B30",
    "--accent-5": "#AF52DE",
    "--accent-gradient":
      "linear-gradient(135deg, #007AFF 0%, #AF52DE 50%, #34C759 100%)",
    "--success": "#34C759",
    "--danger": "#FF3B30",
    "--border": "rgba(0, 0, 0, 0.06)",
    "--border-focus": "rgba(0, 122, 255, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.35)",
  },
  dark: {
    "--bg-base": "#0a0a0c",
    "--bg-elevated": "rgba(30, 30, 34, 0.8)",
    "--bg-floating": "#1e1e22",
    "--bg-glass": "rgba(44, 44, 50, 0.4)",
    "--bg-glass-hover": "rgba(55, 55, 62, 0.55)",
    "--bg-surface": "rgba(28, 28, 32, 0.85)",
    "--text-primary": "rgba(255, 255, 255, 0.93)",
    "--text-secondary": "rgba(255, 255, 255, 0.55)",
    "--text-tertiary": "rgba(255, 255, 255, 0.3)",
    "--accent": "#4A9EFF",
    "--accent-hover": "#6BB3FF",
    "--accent-subtle": "rgba(74, 158, 255, 0.12)",
    "--accent-1": "#4A9EFF",
    "--accent-2": "#30D158",
    "--accent-3": "#FF9F0A",
    "--accent-4": "#FF453A",
    "--accent-5": "#BF5AF2",
    "--accent-gradient":
      "linear-gradient(135deg, #4A9EFF 0%, #BF5AF2 50%, #30D158 100%)",
    "--success": "#30D158",
    "--danger": "#FF453A",
    "--border": "rgba(255, 255, 255, 0.06)",
    "--border-focus": "rgba(74, 158, 255, 0.45)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.07)",
  },
};
