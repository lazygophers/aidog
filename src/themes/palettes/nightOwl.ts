import type { PaletteDefinition } from "../types";

/**
 * Night Owl 调色板（Sarah Drasner night-owl-vscode-theme — Night Owl / Light Owl）
 * dark: editor.bg #011627, fg #d6deeb, blue #82AAFF, green #c5e478, red #EF5350, purple #c792ea
 * light: editor.bg #FAFBFC, fg #403F53, blue #4373EE, green #2D9C5F, red #E64545
 */
export const nightOwl: PaletteDefinition = {
  id: "nightOwl",
  label: "theme.color.nightOwl",
  light: {
    "--bg-base": "#FAFBFC",
    "--bg-elevated": "#FFFFFF",
    "--bg-floating": "#FFFFFF",
    "--bg-glass": "rgba(255, 255, 255, 0.7)",
    "--bg-glass-hover": "rgba(250, 251, 252, 0.85)",
    "--bg-surface": "#F1F2F4",
    "--text-primary": "#403F53",
    "--text-secondary": "#6B6F76",
    "--text-tertiary": "#90A4AE",
    "--accent": "#4373EE",
    "--accent-hover": "#2F5DD4",
    "--accent-subtle": "rgba(67, 115, 238, 0.1)",
    "--accent-1": "#4373EE",
    "--accent-2": "#2D9C5F",
    "--accent-3": "#E0AF02",
    "--accent-4": "#E64545",
    "--accent-5": "#9F61E0",
    "--accent-gradient":
      "linear-gradient(135deg, #4373EE 0%, #9F61E0 50%, #2D9C5F 100%)",
    "--success": "#2D9C5F",
    "--danger": "#E64545",
    "--border": "rgba(64, 63, 83, 0.12)",
    "--border-focus": "rgba(67, 115, 238, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.4)",
  },
  dark: {
    "--bg-base": "#011627",
    "--bg-elevated": "#0B2942",
    "--bg-floating": "#082034",
    "--bg-glass": "rgba(1, 22, 39, 0.85)",
    "--bg-glass-hover": "rgba(11, 41, 66, 0.92)",
    "--bg-surface": "#122D42",
    "--text-primary": "#D6DEEB",
    "--text-secondary": "#8BADCC",
    "--text-tertiary": "#5F7E97",
    "--accent": "#82AAFF",
    "--accent-hover": "#6B90F5",
    "--accent-subtle": "rgba(130, 170, 255, 0.13)",
    "--accent-1": "#82AAFF",
    "--accent-2": "#C5E478",
    "--accent-3": "#ECC48D",
    "--accent-4": "#EF5350",
    "--accent-5": "#C792EA",
    "--accent-gradient":
      "linear-gradient(135deg, #82AAFF 0%, #C792EA 50%, #C5E478 100%)",
    "--success": "#22DA6E",
    "--danger": "#EF5350",
    "--border": "rgba(214, 222, 235, 0.1)",
    "--border-focus": "rgba(130, 170, 255, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.06)",
  },
};
