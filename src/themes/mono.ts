import type { ThemeDefinition } from "./types";

/**
 * Mono · 单色磨砂玻璃（唯一主题）。
 * 配色轴收敛为纯黑(dark) / 纯白(light) 中性阶 —— 无品牌色相，
 * 玻璃质感来自 blur + 白色顶边 sheen + 柔阴影（见 globals.css .glass*）。
 * 语义色（success/warning/danger）在 globals.css 固定，可访问性需要故不单色化。
 * 单文件同时持结构变量(radius/blur/shadow) + shadcn 语义色 token。
 */
export const mono: ThemeDefinition = {
  light: {
    // ── 结构 ──
    "--radius-sm": "10px",
    "--radius-md": "14px",
    "--radius-lg": "20px",
    "--radius-xl": "28px",
    "--glass-blur": "24px",
    "--glass-saturate": "1.8",
    "--glass-border": "1px solid var(--glass-edge)",
    "--shadow-sm": "0 1px 3px rgba(var(--shadow-rgb), 0.06), 0 1px 2px rgba(var(--shadow-rgb), 0.03)",
    "--shadow-md": "0 4px 16px rgba(var(--shadow-rgb), 0.08), 0 1px 4px rgba(var(--shadow-rgb), 0.04)",
    "--shadow-lg": "0 12px 40px rgba(var(--shadow-rgb), 0.10), 0 4px 12px rgba(var(--shadow-rgb), 0.05)",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
    "--app-bg-overlay":
      "radial-gradient(90% 60% at 50% -10%, rgba(0, 0, 0, 0.03), transparent 60%)",
    // ── 色（白） ──
    "--background": "#ffffff",
    "--foreground": "#0a0a0b",
    "--card": "#ffffff",
    "--card-foreground": "#0a0a0b",
    "--popover": "#ffffff",
    "--popover-foreground": "#0a0a0b",
    "--primary": "#0a0a0b",
    "--primary-foreground": "#ffffff",
    "--secondary": "#f4f4f5",
    "--secondary-foreground": "#0a0a0b",
    "--muted": "#f4f4f5",
    "--muted-foreground": "#52525b",
    "--accent": "#f4f4f5",
    "--accent-foreground": "#0a0a0b",
    "--destructive": "#dc2626",
    "--destructive-foreground": "#ffffff",
    "--border": "rgba(10, 10, 11, 0.10)",
    "--input": "rgba(10, 10, 11, 0.12)",
    "--ring": "rgba(10, 10, 11, 0.25)",
    "--shadow-color": "10, 10, 11",
  },
  dark: {
    // ── 结构 ──
    "--radius-sm": "10px",
    "--radius-md": "14px",
    "--radius-lg": "20px",
    "--radius-xl": "28px",
    "--glass-blur": "24px",
    "--glass-saturate": "1.5",
    "--glass-border": "1px solid var(--glass-edge)",
    "--shadow-sm": "0 1px 3px rgba(var(--shadow-rgb), 0.3), 0 1px 2px rgba(var(--shadow-rgb), 0.2)",
    "--shadow-md": "0 4px 16px rgba(var(--shadow-rgb), 0.4), 0 1px 4px rgba(var(--shadow-rgb), 0.3)",
    "--shadow-lg": "0 12px 40px rgba(var(--shadow-rgb), 0.5), 0 4px 12px rgba(var(--shadow-rgb), 0.3)",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
    "--app-bg-overlay":
      "radial-gradient(90% 60% at 50% -10%, rgba(255, 255, 255, 0.05), transparent 60%)",
    // ── 色（黑） ──
    "--background": "#0a0a0b",
    "--foreground": "#fafafa",
    "--card": "#141416",
    "--card-foreground": "#fafafa",
    "--popover": "#1b1b1e",
    "--popover-foreground": "#fafafa",
    "--primary": "#fafafa",
    "--primary-foreground": "#0a0a0b",
    "--secondary": "#1b1b1e",
    "--secondary-foreground": "#fafafa",
    "--muted": "#1b1b1e",
    "--muted-foreground": "#a1a1aa",
    "--accent": "#26262a",
    "--accent-foreground": "#fafafa",
    "--destructive": "#f87171",
    "--destructive-foreground": "#0a0a0b",
    "--border": "rgba(250, 250, 250, 0.10)",
    "--input": "rgba(250, 250, 250, 0.12)",
    "--ring": "rgba(250, 250, 250, 0.30)",
    "--shadow-color": "0, 0, 0",
  },
};
