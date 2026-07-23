import type { ThemeDefinition } from "./types";

/**
 * Mono · 金蓝流沙玻璃（唯一主题）。参考原型「微光流沙玻璃」。
 * light「晨曦」：蓝为主 primary (#0087EB) + 暖金 accent + 薰衣草蓝底 (#EEF1FB)，白 fg。
 * dark「夜空金沙」：金为主 primary (#FFD98A) + 蓝 accent/active + 深 navy 底 (#0B1220) + 金星点，近黑 fg。
 * primary/accent 双模互换签名色；边框随 primary 色（light 蓝 / dark 金）。
 * 金属「闪」+ 蓝金流光描边由 globals.css .bg-primary 渐变 sheen + .glass:hover conic flow-border 呈现。
 * 玻璃质感来自 blur + 顶边 sheen + 柔阴影（见 globals.css .glass*）。
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
    // 晨曦：蓝主光晕(顶) + 暖金侧光晕(右上)，落于薰衣草蓝底
    "--app-bg-overlay":
      "radial-gradient(70% 50% at 50% -10%, rgba(0, 135, 235, 0.10), transparent 60%), " +
      "radial-gradient(50% 42% at 88% 8%, rgba(255, 217, 138, 0.16), transparent 58%), " +
      "radial-gradient(55% 45% at 8% 100%, rgba(59, 160, 255, 0.08), transparent 60%)",
    // ── 色（晨曦 · 蓝主金辅） ──
    "--background": "#eef1fb",
    "--foreground": "#111827",
    "--card": "#ffffff",
    "--card-foreground": "#111827",
    "--popover": "#ffffff",
    "--popover-foreground": "#111827",
    "--primary": "#0087eb",
    "--primary-foreground": "#ffffff",
    "--secondary": "#e4ecfb",
    "--secondary-foreground": "#111827",
    "--muted": "#e4ecfb",
    "--muted-foreground": "#5a6478",
    "--accent": "#fbefd3",
    "--accent-foreground": "#8a6a1e",
    "--destructive": "#e0644a",
    "--destructive-foreground": "#ffffff",
    "--border": "rgba(0, 135, 235, 0.18)",
    "--input": "rgba(17, 24, 39, 0.12)",
    "--ring": "rgba(0, 135, 235, 0.40)",
    "--shadow-color": "17, 24, 39",
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
    // 夜空金沙：金主光晕(顶) + 蓝侧光晕(左) + 金星点(多 radial 微点)
    "--app-bg-overlay":
      "radial-gradient(80% 50% at 50% -10%, rgba(255, 217, 138, 0.12), transparent 60%), " +
      "radial-gradient(55% 42% at 12% 22%, rgba(59, 160, 255, 0.10), transparent 55%), " +
      "radial-gradient(1.5px 1.5px at 18% 28%, rgba(255, 217, 138, 0.55), transparent), " +
      "radial-gradient(1.5px 1.5px at 72% 18%, rgba(255, 217, 138, 0.45), transparent), " +
      "radial-gradient(1px 1px at 42% 62%, rgba(255, 217, 138, 0.40), transparent), " +
      "radial-gradient(1.5px 1.5px at 88% 48%, rgba(255, 217, 138, 0.42), transparent), " +
      "radial-gradient(1px 1px at 28% 82%, rgba(178, 227, 255, 0.40), transparent), " +
      "radial-gradient(1px 1px at 62% 88%, rgba(255, 217, 138, 0.35), transparent)",
    // ── 色（夜空金沙 · 金主蓝辅） ──
    "--background": "#0b1220",
    "--foreground": "#e8edf7",
    "--card": "#141e30",
    "--card-foreground": "#e8edf7",
    "--popover": "#0e1729",
    "--popover-foreground": "#e8edf7",
    "--primary": "#ffd98a",
    "--primary-foreground": "#1a1206",
    "--secondary": "#0e1729",
    "--secondary-foreground": "#e8edf7",
    "--muted": "#0e1729",
    "--muted-foreground": "#8a95ad",
    "--accent": "#14263f",
    "--accent-foreground": "#3ba0ff",
    "--destructive": "#e0644a",
    "--destructive-foreground": "#ffffff",
    "--border": "rgba(255, 217, 138, 0.16)",
    "--input": "rgba(232, 237, 247, 0.12)",
    "--ring": "rgba(255, 217, 138, 0.45)",
    "--shadow-color": "0, 0, 0",
  },
};
