import type { PaletteDefinition } from "../types";

/**
 * Nord 调色板
 * 北极蓝调色板，冷色系低对比度。品牌主色 = nord frost blue (--primary)。
 *
 * ponytail: shadcn 语义 token 直接覆盖 globals.css :root 默认值。
 * --accent = 调色板中性 hover 色（非品牌色，对齐 shadcn 语义）。
 */
export const nord: PaletteDefinition = {
  id: "nord",
  label: "theme.color.nord",
  light: {
    "--background": "#eceff4",
    "--foreground": "#2e3440",
    "--card": "#f5f7fa",
    "--card-foreground": "#2e3440",
    "--popover": "#e5e9f0",
    "--popover-foreground": "#2e3440",
    "--primary": "#5e81ac",
    "--primary-foreground": "#eceff4",
    "--secondary": "#e5e9f0",
    "--secondary-foreground": "#2e3440",
    "--muted": "#e5e9f0",
    "--muted-foreground": "#4c566a",
    "--accent": "#e5e9f0",
    "--accent-foreground": "#2e3440",
    "--destructive": "#bf616a",
    "--destructive-foreground": "#eceff4",
    "--border": "rgba(76, 86, 106, 0.15)",
    "--input": "rgba(76, 86, 106, 0.15)",
    "--ring": "#5e81ac",
    "--shadow-color": "46, 52, 64",
  },
  dark: {
    "--background": "#2e3440",
    "--foreground": "#eceff4",
    "--card": "#434c5e",
    "--card-foreground": "#eceff4",
    "--popover": "#3b4252",
    "--popover-foreground": "#eceff4",
    "--primary": "#88c0d0",
    "--primary-foreground": "#2e3440",
    "--secondary": "#3b4252",
    "--secondary-foreground": "#eceff4",
    "--muted": "#3b4252",
    "--muted-foreground": "#d8dee9",
    "--accent": "#3b4252",
    "--accent-foreground": "#eceff4",
    "--destructive": "#bf616a",
    "--destructive-foreground": "#eceff4",
    "--border": "rgba(216, 222, 233, 0.08)",
    "--input": "rgba(216, 222, 233, 0.08)",
    "--ring": "#88c0d0",
    "--shadow-color": "0, 0, 0",
  },
};
