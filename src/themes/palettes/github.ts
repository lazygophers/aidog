import type { PaletteDefinition } from "../types";

/**
 * GitHub 调色板（Primer color system — GitHub Dark Default / GitHub Light Default）
 * dark: canvas #0D1117, fg #C9D1D9, accent #58A6FF, success #3FB950, danger #F85149
 * light: canvas #FFFFFF, fg #1F2328, accent #0969DA, success #1A7F37, danger #CF222E
 */
export const github: PaletteDefinition = {
  id: "github",
  label: "theme.color.github",
  light: {
    "--bg-base": "#FFFFFF",
    "--bg-elevated": "#F6F8FA",
    "--bg-floating": "#FFFFFF",
    "--bg-glass": "rgba(255, 255, 255, 0.7)",
    "--bg-glass-hover": "rgba(246, 248, 250, 0.85)",
    "--bg-surface": "#F6F8FA",
    "--text-primary": "#1F2328",
    "--text-secondary": "#59636E",
    "--text-tertiary": "#818B98",
    "--accent": "#0969DA",
    "--accent-hover": "#0860CA",
    "--accent-subtle": "rgba(9, 105, 218, 0.1)",
    "--accent-1": "#0969DA",
    "--accent-2": "#1A7F37",
    "--accent-3": "#9A6700",
    "--accent-4": "#CF222E",
    "--accent-5": "#8250DF",
    "--accent-gradient":
      "linear-gradient(135deg, #0969DA 0%, #8250DF 50%, #1A7F37 100%)",
    "--success": "#1A7F37",
    "--danger": "#CF222E",
    "--border": "rgba(31, 35, 40, 0.15)",
    "--border-focus": "rgba(9, 105, 218, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.4)",
  },
  dark: {
    "--bg-base": "#0D1117",
    "--bg-elevated": "#161B22",
    "--bg-floating": "#1C2128",
    "--bg-glass": "rgba(13, 17, 23, 0.85)",
    "--bg-glass-hover": "rgba(22, 27, 34, 0.92)",
    "--bg-surface": "#161B22",
    "--text-primary": "#C9D1D9",
    "--text-secondary": "#8B949E",
    "--text-tertiary": "#6E7681",
    "--accent": "#58A6FF",
    "--accent-hover": "#4493F8",
    "--accent-subtle": "rgba(88, 166, 255, 0.13)",
    "--accent-1": "#58A6FF",
    "--accent-2": "#3FB950",
    "--accent-3": "#D29922",
    "--accent-4": "#F85149",
    "--accent-5": "#BC8CFF",
    "--accent-gradient":
      "linear-gradient(135deg, #58A6FF 0%, #BC8CFF 50%, #3FB950 100%)",
    "--success": "#3FB950",
    "--danger": "#F85149",
    "--border": "rgba(240, 246, 252, 0.1)",
    "--border-focus": "rgba(88, 166, 255, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.06)",
  },
};
