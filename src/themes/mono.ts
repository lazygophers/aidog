import type { ThemeDefinition } from "./types";

/**
 * Mono · 金色磨砂玻璃（唯一主题）。
 * 底仍纯黑(dark) / 纯白(light) 中性磨砂玻璃；强调色 (primary/ring/accent/border)
 * 换金色族（金闪闪）：dark 高亮金 #e6c34d / light 深金 #c9a227，金字底黑 fg 金属经典读法。
 * 金属「闪」效由 globals.css 主按钮渐变 sheen + 柔金 glow 呈现。
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
      "radial-gradient(90% 60% at 50% -10%, rgba(201, 162, 39, 0.07), transparent 60%)",
    // ── 色（白） ──
    "--background": "#ffffff",
    "--foreground": "#0a0a0b",
    "--card": "#ffffff",
    "--card-foreground": "#0a0a0b",
    "--popover": "#ffffff",
    "--popover-foreground": "#0a0a0b",
    "--primary": "#c9a227",
    "--primary-foreground": "#1a1400",
    "--secondary": "#f4f4f5",
    "--secondary-foreground": "#0a0a0b",
    "--muted": "#f4f4f5",
    "--muted-foreground": "#52525b",
    "--accent": "#faf3d8",
    "--accent-foreground": "#7a5c00",
    "--destructive": "#dc2626",
    "--destructive-foreground": "#ffffff",
    "--border": "rgba(201, 162, 39, 0.18)",
    "--input": "rgba(10, 10, 11, 0.12)",
    "--ring": "rgba(201, 162, 39, 0.40)",
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
      "radial-gradient(90% 60% at 50% -10%, rgba(230, 195, 77, 0.10), transparent 60%)",
    // ── 色（黑） ──
    "--background": "#0a0a0b",
    "--foreground": "#fafafa",
    "--card": "#141416",
    "--card-foreground": "#fafafa",
    "--popover": "#1b1b1e",
    "--popover-foreground": "#fafafa",
    "--primary": "#e6c34d",
    "--primary-foreground": "#1a1400",
    "--secondary": "#1b1b1e",
    "--secondary-foreground": "#fafafa",
    "--muted": "#1b1b1e",
    "--muted-foreground": "#a1a1aa",
    "--accent": "#2a2515",
    "--accent-foreground": "#e6c34d",
    "--destructive": "#f87171",
    "--destructive-foreground": "#0a0a0b",
    "--border": "rgba(230, 195, 77, 0.16)",
    "--input": "rgba(250, 250, 250, 0.12)",
    "--ring": "rgba(230, 195, 77, 0.45)",
    "--shadow-color": "0, 0, 0",
  },
};
