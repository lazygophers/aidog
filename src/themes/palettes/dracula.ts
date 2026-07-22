import type { PaletteDefinition } from "../types";

/**
 * Dracula 调色板
 * 经典暗色系，高饱和度。品牌主色 = dracula purple/pink (--primary)。
 *
 * ponytail: shadcn 语义 token 直接覆盖 globals.css :root 默认值。
 * --accent = 调色板中性 hover 色（非品牌色，对齐 shadcn 语义）。
 */
export const dracula: PaletteDefinition = {
  id: "dracula",
  label: "theme.color.dracula",
  light: {
    "--background": "#f8f8f2",
    "--foreground": "#282a36",
    "--card": "#ffffff",
    "--card-foreground": "#282a36",
    "--popover": "#eeeef0",
    "--popover-foreground": "#282a36",
    "--primary": "#bd93f9",
    "--primary-foreground": "#282a36",
    "--secondary": "#eeeef0",
    "--secondary-foreground": "#282a36",
    "--muted": "#eeeef0",
    "--muted-foreground": "#44475a",
    "--accent": "#eeeef0",
    "--accent-foreground": "#282a36",
    "--destructive": "#ff5555",
    "--destructive-foreground": "#f8f8f2",
    "--border": "rgba(40, 42, 54, 0.1)",
    "--input": "rgba(40, 42, 54, 0.1)",
    "--ring": "#bd93f9",
    "--shadow-color": "40, 42, 54",
  },
  dark: {
    "--background": "#282a36",
    "--foreground": "#f8f8f2",
    "--card": "#44475a",
    "--card-foreground": "#f8f8f2",
    "--popover": "#343746",
    "--popover-foreground": "#f8f8f2",
    "--primary": "#ff79c6",
    "--primary-foreground": "#282a36",
    "--secondary": "#343746",
    "--secondary-foreground": "#f8f8f2",
    "--muted": "#343746",
    "--muted-foreground": "#bd93f9",
    "--accent": "#343746",
    "--accent-foreground": "#f8f8f2",
    "--destructive": "#ff5555",
    "--destructive-foreground": "#f8f8f2",
    "--border": "rgba(248, 248, 242, 0.08)",
    "--input": "rgba(248, 248, 242, 0.08)",
    "--ring": "#ff79c6",
    "--shadow-color": "0, 0, 0",
  },
};
