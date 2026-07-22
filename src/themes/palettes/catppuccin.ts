import type { PaletteDefinition } from "../types";

/**
 * Catppuccin 调色板（Latte/Mocha）
 * 柔和暖色调。品牌主色 = catppuccin blue/mauve (--primary)。
 *
 * ponytail: shadcn 语义 token 直接覆盖 globals.css :root 默认值。
 * --accent = 调色板中性 hover 色（非品牌色，对齐 shadcn 语义）。
 */
export const catppuccin: PaletteDefinition = {
  id: "catppuccin",
  label: "theme.color.catppuccin",
  light: {
    "--background": "#eff1f5",
    "--foreground": "#4c4f69",
    "--card": "#ffffff",
    "--card-foreground": "#4c4f69",
    "--popover": "#e6e9ef",
    "--popover-foreground": "#4c4f69",
    "--primary": "#1e66f5",
    "--primary-foreground": "#eff1f5",
    "--secondary": "#e6e9ef",
    "--secondary-foreground": "#4c4f69",
    "--muted": "#e6e9ef",
    "--muted-foreground": "#5c5f77",
    "--accent": "#e6e9ef",
    "--accent-foreground": "#4c4f69",
    "--destructive": "#d20f39",
    "--destructive-foreground": "#eff1f5",
    "--border": "rgba(76, 79, 105, 0.1)",
    "--input": "rgba(76, 79, 105, 0.1)",
    "--ring": "#1e66f5",
    "--shadow-color": "76, 79, 105",
  },
  dark: {
    "--background": "#1e1e2e",
    "--foreground": "#cdd6f4",
    "--card": "#313244",
    "--card-foreground": "#cdd6f4",
    "--popover": "#262637",
    "--popover-foreground": "#cdd6f4",
    "--primary": "#cba6f7",
    "--primary-foreground": "#1e1e2e",
    "--secondary": "#262637",
    "--secondary-foreground": "#cdd6f4",
    "--muted": "#262637",
    "--muted-foreground": "#a6adc8",
    "--accent": "#262637",
    "--accent-foreground": "#cdd6f4",
    "--destructive": "#f38ba8",
    "--destructive-foreground": "#1e1e2e",
    "--border": "rgba(205, 214, 244, 0.08)",
    "--input": "rgba(205, 214, 244, 0.08)",
    "--ring": "#cba6f7",
    "--shadow-color": "0, 0, 0",
  },
};
