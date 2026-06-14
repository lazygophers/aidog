import type { PaletteDefinition } from "../types";

/**
 * Morandi 莫兰迪调色板（低饱和灰调，授色 anchor 见 PRD）
 * 尘玫 / 鼠尾草绿 / 陶土，温润低饱和。
 */
export const morandi: PaletteDefinition = {
  id: "morandi",
  label: "theme.color.morandi",
  light: {
    "--bg-base": "#ECE8E3",
    "--bg-elevated": "#F0ECE6",
    "--bg-floating": "#F0ECE6",
    "--bg-glass": "rgba(236, 232, 227, 0.88)",
    "--bg-glass-hover": "rgba(240, 236, 230, 0.95)",
    "--bg-surface": "#F4F1EC",
    "--text-primary": "#4A4540",
    "--text-secondary": "#6E665E",
    "--text-tertiary": "#9A9088",
    "--accent": "#B08A7E",
    "--accent-hover": "#9A7065",
    "--accent-subtle": "rgba(176, 138, 126, 0.13)",
    "--accent-1": "#B08A7E",
    "--accent-2": "#8C9A82",
    "--accent-3": "#8E9DAB",
    "--accent-4": "#B5746B",
    "--accent-5": "#A9947E",
    "--accent-gradient":
      "linear-gradient(135deg, #B08A7E 0%, #8E9DAB 50%, #8C9A82 100%)",
    "--success": "#8C9A82",
    "--danger": "#B5746B",
    "--border": "rgba(74, 69, 64, 0.12)",
    "--border-focus": "rgba(176, 138, 126, 0.45)",
    "--shadow-rgb": "74, 69, 64",
    "--glass-edge": "rgba(255, 255, 255, 0.5)",
  },
  dark: {
    "--bg-base": "#2B2926",
    "--bg-elevated": "#302E2A",
    "--bg-floating": "#302E2A",
    "--bg-glass": "rgba(43, 41, 38, 0.85)",
    "--bg-glass-hover": "rgba(48, 46, 42, 0.92)",
    "--bg-surface": "#34322E",
    "--text-primary": "#E2DDD6",
    "--text-secondary": "#B5AFA6",
    "--text-tertiary": "#857F77",
    "--accent": "#C29C8F",
    "--accent-hover": "#D2B2A6",
    "--accent-subtle": "rgba(194, 156, 143, 0.12)",
    "--accent-1": "#C29C8F",
    "--accent-2": "#9DAA92",
    "--accent-3": "#9EADBA",
    "--accent-4": "#C28178",
    "--accent-5": "#B9A38C",
    "--accent-gradient":
      "linear-gradient(135deg, #C29C8F 0%, #9EADBA 50%, #9DAA92 100%)",
    "--success": "#9DAA92",
    "--danger": "#C28178",
    "--border": "rgba(226, 221, 214, 0.1)",
    "--border-focus": "rgba(194, 156, 143, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.05)",
  },
};
