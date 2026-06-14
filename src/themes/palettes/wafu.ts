import type { PaletteDefinition } from "../types";

/**
 * Wafu 和风调色板（日本传统色，授色 anchor 见 PRD）
 * 藍 / 茜 / 生成り / 墨 / 浅葱。
 */
export const wafu: PaletteDefinition = {
  id: "wafu",
  label: "theme.color.wafu",
  light: {
    "--bg-base": "#FBF8F1",
    "--bg-elevated": "#F7F3EC",
    "--bg-floating": "#F7F3EC",
    "--bg-glass": "rgba(251, 248, 241, 0.9)",
    "--bg-glass-hover": "rgba(247, 243, 236, 0.95)",
    "--bg-surface": "#FFFFFF",
    "--text-primary": "#2B2B2B",
    "--text-secondary": "#5A5750",
    "--text-tertiary": "#8A857C",
    "--accent": "#1F5C8B",
    "--accent-hover": "#17456A",
    "--accent-subtle": "rgba(31, 92, 139, 0.13)",
    "--success": "#6E8B5A",
    "--danger": "#9E2236",
    "--border": "rgba(43, 43, 43, 0.12)",
    "--border-focus": "rgba(31, 92, 139, 0.45)",
    "--shadow-rgb": "43, 43, 43",
    "--glass-edge": "rgba(255, 255, 255, 0.5)",
  },
  dark: {
    "--bg-base": "#1A2230",
    "--bg-elevated": "#1F2735",
    "--bg-floating": "#1F2735",
    "--bg-glass": "rgba(26, 34, 48, 0.85)",
    "--bg-glass-hover": "rgba(31, 39, 53, 0.92)",
    "--bg-surface": "#232B3A",
    "--text-primary": "#F0EAE0",
    "--text-secondary": "#B8B2A6",
    "--text-tertiary": "#857F75",
    "--accent": "#4FA3AD",
    "--accent-hover": "#6BBAC3",
    "--accent-subtle": "rgba(79, 163, 173, 0.12)",
    "--success": "#88A06E",
    "--danger": "#C24E5C",
    "--border": "rgba(240, 234, 224, 0.1)",
    "--border-focus": "rgba(79, 163, 173, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(240, 234, 224, 0.06)",
  },
};
