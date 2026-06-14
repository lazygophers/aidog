import type { PaletteDefinition } from "../types";

/**
 * Monet 莫奈调色板（印象派睡莲，授色 anchor 见 PRD）
 * 睡莲蓝 / 池绿 / 暖玫，柔蓝绿紫。
 */
export const monet: PaletteDefinition = {
  id: "monet",
  label: "theme.color.monet",
  light: {
    "--bg-base": "#EAF0F1",
    "--bg-elevated": "#EDF2F2",
    "--bg-floating": "#EDF2F2",
    "--bg-glass": "rgba(234, 240, 241, 0.88)",
    "--bg-glass-hover": "rgba(237, 242, 242, 0.95)",
    "--bg-surface": "#F2F6F6",
    "--text-primary": "#3A4A52",
    "--text-secondary": "#5E727B",
    "--text-tertiary": "#90A2AA",
    "--accent": "#6E92B8",
    "--accent-hover": "#567BA3",
    "--accent-subtle": "rgba(110, 146, 184, 0.13)",
    "--accent-1": "#6E92B8",
    "--accent-2": "#7FA284",
    "--accent-3": "#A88FB5",
    "--accent-4": "#E0A87E",
    "--accent-5": "#9FB46A",
    "--accent-gradient":
      "linear-gradient(135deg, #6E92B8 0%, #A88FB5 50%, #7FA284 100%)",
    "--success": "#7FA284",
    "--danger": "#C98A86",
    "--border": "rgba(58, 74, 82, 0.12)",
    "--border-focus": "rgba(110, 146, 184, 0.45)",
    "--shadow-rgb": "58, 74, 82",
    "--glass-edge": "rgba(255, 255, 255, 0.55)",
  },
  dark: {
    "--bg-base": "#1E2A30",
    "--bg-elevated": "#22323A",
    "--bg-floating": "#22323A",
    "--bg-glass": "rgba(30, 42, 48, 0.85)",
    "--bg-glass-hover": "rgba(34, 50, 58, 0.92)",
    "--bg-surface": "#273840",
    "--text-primary": "#DCE6E8",
    "--text-secondary": "#A6B8BD",
    "--text-tertiary": "#7A8C92",
    "--accent": "#8AAECB",
    "--accent-hover": "#A4C2D9",
    "--accent-subtle": "rgba(138, 174, 203, 0.12)",
    "--accent-1": "#8AAECB",
    "--accent-2": "#93B597",
    "--accent-3": "#BCA5C8",
    "--accent-4": "#EEBA94",
    "--accent-5": "#B3C682",
    "--accent-gradient":
      "linear-gradient(135deg, #8AAECB 0%, #BCA5C8 50%, #93B597 100%)",
    "--success": "#93B597",
    "--danger": "#D29C98",
    "--border": "rgba(220, 230, 232, 0.1)",
    "--border-focus": "rgba(138, 174, 203, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.05)",
  },
};
