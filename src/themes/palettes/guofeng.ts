import type { PaletteDefinition } from "../types";

/**
 * Guofeng 中国风调色板（中国传统色，授色 anchor 见 PRD）
 * 月白 / 胭脂 / 朱砂 / 竹青 / 黛 / 藤黄。
 */
export const guofeng: PaletteDefinition = {
  id: "guofeng",
  label: "theme.color.guofeng",
  light: {
    "--bg-base": "#F2EFE6",
    "--bg-elevated": "#EDEAE0",
    "--bg-floating": "#EDEAE0",
    "--bg-glass": "rgba(242, 239, 230, 0.9)",
    "--bg-glass-hover": "rgba(237, 234, 224, 0.95)",
    "--bg-surface": "#FAF7EF",
    "--text-primary": "#2E2C2B",
    "--text-secondary": "#5C5853",
    "--text-tertiary": "#8C867D",
    "--accent": "#9D2933",
    "--accent-hover": "#7E1F28",
    "--accent-subtle": "rgba(157, 41, 51, 0.13)",
    "--success": "#6B8E5A",
    "--danger": "#C0392B",
    "--border": "rgba(46, 44, 43, 0.12)",
    "--border-focus": "rgba(157, 41, 51, 0.45)",
    "--shadow-rgb": "46, 44, 43",
    "--glass-edge": "rgba(255, 255, 255, 0.5)",
  },
  dark: {
    "--bg-base": "#1C1A18",
    "--bg-elevated": "#211E1B",
    "--bg-floating": "#211E1B",
    "--bg-glass": "rgba(28, 26, 24, 0.85)",
    "--bg-glass-hover": "rgba(33, 30, 27, 0.92)",
    "--bg-surface": "#262320",
    "--text-primary": "#E6EBE8",
    "--text-secondary": "#B0AAA0",
    "--text-tertiary": "#827C72",
    "--accent": "#D8503C",
    "--accent-hover": "#E6A817",
    "--accent-subtle": "rgba(216, 80, 60, 0.12)",
    "--success": "#7FA968",
    "--danger": "#C0392B",
    "--border": "rgba(230, 235, 232, 0.1)",
    "--border-focus": "rgba(216, 80, 60, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(230, 235, 232, 0.06)",
  },
};
