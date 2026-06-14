import type { PaletteDefinition } from "../types";

/**
 * Dracula 调色板
 * 经典暗色系，高饱和度。
 */
export const dracula: PaletteDefinition = {
  id: "dracula",
  label: "theme.color.dracula",
  light: {
    "--bg-base": "#f8f8f2",
    "--bg-elevated": "#eeeef0",
    "--bg-floating": "#eeeef0",
    "--bg-glass": "rgba(248, 248, 242, 0.88)",
    "--bg-glass-hover": "rgba(238, 238, 240, 0.95)",
    "--bg-surface": "#ffffff",
    "--text-primary": "#282a36",
    "--text-secondary": "#44475a",
    "--text-tertiary": "#6272a4",
    "--accent": "#bd93f9",
    "--accent-hover": "#a67be0",
    "--accent-subtle": "rgba(189, 147, 249, 0.12)",
    "--accent-1": "#bd93f9",
    "--accent-2": "#ff79c6",
    "--accent-3": "#8be9fd",
    "--accent-4": "#50fa7b",
    "--accent-5": "#ffb86c",
    "--accent-gradient":
      "linear-gradient(135deg, #ff79c6 0%, #8be9fd 50%, #bd93f9 100%)",
    "--success": "#50fa7b",
    "--danger": "#ff5555",
    "--border": "rgba(40, 42, 54, 0.1)",
    "--border-focus": "rgba(189, 147, 249, 0.5)",
    "--shadow-rgb": "40, 42, 54",
    "--glass-edge": "rgba(40, 42, 54, 0.08)",
  },
  dark: {
    "--bg-base": "#282a36",
    "--bg-elevated": "#343746",
    "--bg-floating": "#343746",
    "--bg-glass": "rgba(40, 42, 54, 0.85)",
    "--bg-glass-hover": "rgba(52, 55, 70, 0.92)",
    "--bg-surface": "#44475a",
    "--text-primary": "#f8f8f2",
    "--text-secondary": "#bd93f9",
    "--text-tertiary": "#6272a4",
    "--accent": "#ff79c6",
    "--accent-hover": "#ff92d0",
    "--accent-subtle": "rgba(255, 121, 198, 0.12)",
    "--accent-1": "#ff79c6",
    "--accent-2": "#bd93f9",
    "--accent-3": "#8be9fd",
    "--accent-4": "#50fa7b",
    "--accent-5": "#ffb86c",
    "--accent-gradient":
      "linear-gradient(135deg, #ff79c6 0%, #8be9fd 50%, #bd93f9 100%)",
    "--success": "#50fa7b",
    "--danger": "#ff5555",
    "--border": "rgba(248, 248, 242, 0.08)",
    "--border-focus": "rgba(255, 121, 198, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(248, 248, 242, 0.06)",
  },
};
