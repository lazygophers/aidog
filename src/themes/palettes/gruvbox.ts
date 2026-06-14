import type { PaletteDefinition } from "../types";

/**
 * Gruvbox 调色板（官方 canonical：light / dark）
 * 复古暖褐，高对比怀旧调。
 */
export const gruvbox: PaletteDefinition = {
  id: "gruvbox",
  label: "theme.color.gruvbox",
  light: {
    "--bg-base": "#fbf1c7",
    "--bg-elevated": "#f2e5bc",
    "--bg-floating": "#f2e5bc",
    "--bg-glass": "rgba(251, 241, 199, 0.9)",
    "--bg-glass-hover": "rgba(242, 229, 188, 0.95)",
    "--bg-surface": "#f9f5d7",
    "--text-primary": "#3c3836",
    "--text-secondary": "#665c54",
    "--text-tertiary": "#928374",
    "--accent": "#d65d0e",
    "--accent-hover": "#af3a03",
    "--accent-subtle": "rgba(214, 93, 14, 0.13)",
    "--success": "#79740e",
    "--danger": "#9d0006",
    "--border": "rgba(60, 56, 54, 0.12)",
    "--border-focus": "rgba(214, 93, 14, 0.45)",
    "--shadow-rgb": "60, 56, 54",
    "--glass-edge": "rgba(60, 56, 54, 0.1)",
  },
  dark: {
    "--bg-base": "#282828",
    "--bg-elevated": "#32302f",
    "--bg-floating": "#32302f",
    "--bg-glass": "rgba(40, 40, 40, 0.85)",
    "--bg-glass-hover": "rgba(50, 48, 47, 0.92)",
    "--bg-surface": "#3c3836",
    "--text-primary": "#ebdbb2",
    "--text-secondary": "#a89984",
    "--text-tertiary": "#7c6f64",
    "--accent": "#fe8019",
    "--accent-hover": "#d65d0e",
    "--accent-subtle": "rgba(254, 128, 25, 0.12)",
    "--success": "#b8bb26",
    "--danger": "#fb4934",
    "--border": "rgba(235, 219, 178, 0.1)",
    "--border-focus": "rgba(254, 128, 25, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(235, 219, 178, 0.06)",
  },
};
