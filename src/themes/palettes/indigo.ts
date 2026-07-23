import type { PaletteDefinition } from "../types";

/**
 * Indigo 调色板（靛蓝·冷石板：light / dark）。
 * 冷靛蓝主色 + 冷灰(slate)中性阶，专业信任调，高对比。
 *
 * ponytail: 仅 21 个 shadcn 语义 token；派生 token(text-x, bg-glass, accent-subtle)
 * 由 globals.css 自动派生，语义色(color-success/warning/danger)固定，均不在此重复定义。
 * --accent = 中性 hover 面（非品牌色，对齐 shadcn 语义）。
 */
export const indigo: PaletteDefinition = {
  id: "indigo",
  label: "theme.color.indigo",
  light: {
    "--background": "#f8fafc",
    "--foreground": "#1e2430",
    "--card": "#ffffff",
    "--card-foreground": "#1e2430",
    "--popover": "#ffffff",
    "--popover-foreground": "#1e2430",
    "--primary": "#4f46e5",
    "--primary-foreground": "#ffffff",
    "--secondary": "#eef1f6",
    "--secondary-foreground": "#1e2430",
    "--muted": "#eef1f6",
    "--muted-foreground": "#5b667a",
    "--accent": "#eef1f6",
    "--accent-foreground": "#1e2430",
    "--destructive": "#dc2626",
    "--destructive-foreground": "#ffffff",
    "--border": "rgba(30, 36, 48, 0.10)",
    "--input": "rgba(30, 36, 48, 0.10)",
    "--ring": "#4f46e5",
    "--shadow-color": "30, 36, 48",
  },
  dark: {
    "--background": "#0f1117",
    "--foreground": "#e4e7ee",
    "--card": "#171a23",
    "--card-foreground": "#e4e7ee",
    "--popover": "#1c2029",
    "--popover-foreground": "#e4e7ee",
    "--primary": "#6366f1",
    "--primary-foreground": "#ffffff",
    "--secondary": "#1c2029",
    "--secondary-foreground": "#e4e7ee",
    "--muted": "#1c2029",
    "--muted-foreground": "#9aa3b4",
    "--accent": "#232838",
    "--accent-foreground": "#e4e7ee",
    "--destructive": "#f87171",
    "--destructive-foreground": "#0f1117",
    "--border": "rgba(228, 231, 238, 0.10)",
    "--input": "rgba(228, 231, 238, 0.10)",
    "--ring": "#6366f1",
    "--shadow-color": "0, 0, 0",
  },
};
