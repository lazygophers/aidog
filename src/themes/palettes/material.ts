import type { PaletteDefinition } from "../types";

/**
 * Material 调色板（Material Theme Palenight / Material Lighter）
 * dark: 官方色 whizkydee/vscode-palenight-theme themes/palenight.json
 *   (editor.bg #292D3E, blue #82AAFF, green #C3E88D, red #F07178, purple #C792EA, yellow #FFCB6B)
 * light: Material Lighter 派生 (bg #FAFAFA, fg #546E7A, blue #6182B8, green #91B859, red #E53935)
 */
export const material: PaletteDefinition = {
  id: "material",
  label: "theme.color.material",
  light: {
    "--bg-base": "#FAFAFA",
    "--bg-elevated": "#FFFFFF",
    "--bg-floating": "#FFFFFF",
    "--bg-glass": "rgba(255, 255, 255, 0.7)",
    "--bg-glass-hover": "rgba(250, 250, 250, 0.85)",
    "--bg-surface": "#F2F4F6",
    "--text-primary": "#546E7A",
    "--text-secondary": "#78909C",
    "--text-tertiary": "#B0BEC5",
    "--accent": "#6182B8",
    "--accent-hover": "#4F6FA3",
    "--accent-subtle": "rgba(97, 130, 184, 0.1)",
    "--accent-1": "#6182B8",
    "--accent-2": "#91B859",
    "--accent-3": "#FFB62C",
    "--accent-4": "#E53935",
    "--accent-5": "#7C4DFF",
    "--accent-gradient":
      "linear-gradient(135deg, #6182B8 0%, #7C4DFF 50%, #91B859 100%)",
    "--success": "#91B859",
    "--danger": "#E53935",
    "--border": "rgba(84, 110, 122, 0.12)",
    "--border-focus": "rgba(97, 130, 184, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.4)",
  },
  dark: {
    "--bg-base": "#292D3E",
    "--bg-elevated": "#31364A",
    "--bg-floating": "#232635",
    "--bg-glass": "rgba(41, 45, 62, 0.85)",
    "--bg-glass-hover": "rgba(49, 54, 74, 0.92)",
    "--bg-surface": "#2E3245",
    "--text-primary": "#BFC7D5",
    "--text-secondary": "#8B95B4",
    "--text-tertiary": "#676E95",
    "--accent": "#82AAFF",
    "--accent-hover": "#6F94EE",
    "--accent-subtle": "rgba(130, 170, 255, 0.13)",
    "--accent-1": "#82AAFF",
    "--accent-2": "#C3E88D",
    "--accent-3": "#FFCB6B",
    "--accent-4": "#F07178",
    "--accent-5": "#C792EA",
    "--accent-gradient":
      "linear-gradient(135deg, #82AAFF 0%, #C792EA 50%, #C3E88D 100%)",
    "--success": "#C3E88D",
    "--danger": "#F07178",
    "--border": "rgba(191, 199, 213, 0.1)",
    "--border-focus": "rgba(130, 170, 255, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.06)",
  },
};
