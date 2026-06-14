import type { PaletteDefinition } from "../types";

/**
 * One Dark 调色板（Atom One Dark syntax / Atom One Light syntax）
 * dark: 官方色 atom/one-dark-syntax styles/colors.less (hue-2 #61AFEF 蓝 / hue-4 #98C379 绿 / hue-5 #E06C75 红)
 * light: 官方色 atom/one-light-syntax styles/colors.less (hue-2 #4078F2 蓝 / hue-4 #50A14F 绿 / hue-5 #E45649 红)
 */
export const oneDark: PaletteDefinition = {
  id: "oneDark",
  label: "theme.color.oneDark",
  light: {
    "--bg-base": "#FAFAFA",
    "--bg-elevated": "#FFFFFF",
    "--bg-floating": "#FFFFFF",
    "--bg-glass": "rgba(255, 255, 255, 0.7)",
    "--bg-glass-hover": "rgba(255, 255, 255, 0.85)",
    "--bg-surface": "#F2F2F2",
    "--text-primary": "#383A42",
    "--text-secondary": "#636D75",
    "--text-tertiary": "#9DA3AB",
    "--accent": "#4078F2",
    "--accent-hover": "#3366E0",
    "--accent-subtle": "rgba(64, 120, 242, 0.1)",
    "--accent-1": "#4078F2",
    "--accent-2": "#50A14F",
    "--accent-3": "#C18401",
    "--accent-4": "#E45649",
    "--accent-5": "#A626A4",
    "--accent-gradient":
      "linear-gradient(135deg, #4078F2 0%, #A626A4 50%, #50A14F 100%)",
    "--success": "#50A14F",
    "--danger": "#E45649",
    "--border": "rgba(56, 58, 66, 0.12)",
    "--border-focus": "rgba(64, 120, 242, 0.4)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.4)",
  },
  dark: {
    "--bg-base": "#282C34",
    "--bg-elevated": "#21252B",
    "--bg-floating": "#1B1D23",
    "--bg-glass": "rgba(40, 44, 52, 0.85)",
    "--bg-glass-hover": "rgba(33, 37, 43, 0.92)",
    "--bg-surface": "#2C313A",
    "--text-primary": "#ABB2BF",
    "--text-secondary": "#828997",
    "--text-tertiary": "#5C6370",
    "--accent": "#61AFEF",
    "--accent-hover": "#4C9CE6",
    "--accent-subtle": "rgba(97, 175, 239, 0.13)",
    "--accent-1": "#61AFEF",
    "--accent-2": "#98C379",
    "--accent-3": "#E5C07B",
    "--accent-4": "#E06C75",
    "--accent-5": "#C678DD",
    "--accent-gradient":
      "linear-gradient(135deg, #61AFEF 0%, #C678DD 50%, #98C379 100%)",
    "--success": "#98C379",
    "--danger": "#E06C75",
    "--border": "rgba(171, 178, 191, 0.12)",
    "--border-focus": "rgba(97, 175, 239, 0.45)",
    "--shadow-rgb": "0, 0, 0",
    "--glass-edge": "rgba(255, 255, 255, 0.06)",
  },
};
