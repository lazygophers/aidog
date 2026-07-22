import type { PaletteDefinition } from "../types";

/**
 * Gruvbox 调色板（官方 canonical：light / dark）
 * 复古暖褐，高对比怀旧调。品牌主色 = 暖橙 (--primary)。
 *
 * ponytail: shadcn 语义 token 直接覆盖 globals.css :root 默认值。
 * --accent = 调色板中性 hover 色（非品牌色，对齐 shadcn 语义）。
 */
export const gruvbox: PaletteDefinition = {
  id: "gruvbox",
  label: "theme.color.gruvbox",
  light: {
    "--background": "#fbf1c7",
    "--foreground": "#3c3836",
    "--card": "#f9f5d7",
    "--card-foreground": "#3c3836",
    "--popover": "#f2e5bc",
    "--popover-foreground": "#3c3836",
    "--primary": "#d65d0e",
    "--primary-foreground": "#fbf1c7",
    "--secondary": "#f2e5bc",
    "--secondary-foreground": "#3c3836",
    "--muted": "#f2e5bc",
    "--muted-foreground": "#665c54",
    "--accent": "#f2e5bc",
    "--accent-foreground": "#3c3836",
    "--destructive": "#9d0006",
    "--destructive-foreground": "#fbf1c7",
    "--border": "rgba(60, 56, 54, 0.12)",
    "--input": "rgba(60, 56, 54, 0.12)",
    "--ring": "#d65d0e",
    "--shadow-color": "60, 56, 54",
  },
  dark: {
    "--background": "#282828",
    "--foreground": "#ebdbb2",
    "--card": "#3c3836",
    "--card-foreground": "#ebdbb2",
    "--popover": "#32302f",
    "--popover-foreground": "#ebdbb2",
    "--primary": "#fe8019",
    "--primary-foreground": "#282828",
    "--secondary": "#32302f",
    "--secondary-foreground": "#ebdbb2",
    "--muted": "#32302f",
    "--muted-foreground": "#a89984",
    "--accent": "#32302f",
    "--accent-foreground": "#ebdbb2",
    "--destructive": "#fb4934",
    "--destructive-foreground": "#282828",
    "--border": "rgba(235, 219, 178, 0.1)",
    "--input": "rgba(235, 219, 178, 0.1)",
    "--ring": "#fe8019",
    "--shadow-color": "0, 0, 0",
  },
};
