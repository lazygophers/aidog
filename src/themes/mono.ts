import type { ThemeDefinition } from "./types";

/**
 * Mono · 金蓝流沙玻璃（唯一主题）。2026-07 重设计：light=柔感微光 / dark=静谧玻璃(中性近黑底)。
 * light「柔光」：蓝为主 primary (#0087EB) + 暖金 accent + 淡紫蓝渐变底 (#eef1fb)，偏蓝柔阴影 + tinted 近白卡面。
 * dark「静谧」：金为主 primary (#FFD98A) + 蓝 accent/active + 中性近黑底 (#0a0a0c) + 中性深灰卡面 (#161619/#1c1c20)，近黑 fg。
 * primary/accent 双模互换签名色；边框 light 随蓝 primary / dark 中性白描边。
 * 蓝金流光描边由 globals.css .glass:hover conic flow-border 呈现（发光边框签名，保留）。
 * 主按钮已扁平化（.bg-primary 去金属多段渐变+外发光，改单柔渐变）。
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
    // 柔感微光：双层柔阴影，偏蓝低饱和（--shadow-color 偏蓝），整体轻
    "--shadow-sm": "0 1px 2px rgba(var(--shadow-rgb), 0.05)",
    "--shadow-md": "0 1px 0 rgba(255, 255, 255, 0.8) inset, 0 4px 12px rgba(var(--shadow-rgb), 0.09), 0 1px 3px rgba(var(--shadow-rgb), 0.05)",
    "--shadow-lg": "0 1px 0 rgba(255, 255, 255, 0.85) inset, 0 8px 24px rgba(var(--shadow-rgb), 0.11), 0 2px 6px rgba(var(--shadow-rgb), 0.06)",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
    // 柔光：淡紫蓝渐变底，蓝主光晕(顶) + 极淡暖金侧光晕(右上)
    "--app-bg-overlay":
      "radial-gradient(72% 52% at 50% -10%, rgba(0, 135, 235, 0.12), transparent 62%), " +
      "radial-gradient(52% 44% at 90% 6%, rgba(255, 217, 138, 0.14), transparent 60%), " +
      "radial-gradient(60% 50% at 6% 100%, rgba(120, 140, 220, 0.14), transparent 64%)",
    // ── 色（柔光 · 蓝主金辅） ──
    "--background": "#eef1fb",
    "--foreground": "#111827",
    "--card": "#f8faff",
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
    "--border": "rgba(80, 100, 160, 0.16)",
    "--input": "rgba(17, 24, 39, 0.12)",
    "--ring": "rgba(0, 135, 235, 0.40)",
    "--shadow-color": "80, 100, 160",
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
    // 静谧：柔和暗阴影，纯黑派生（近黑底上读作轻微下沉），整体轻
    "--shadow-sm": "0 1px 2px rgba(var(--shadow-rgb), 0.30)",
    "--shadow-md": "0 4px 12px rgba(var(--shadow-rgb), 0.36), 0 1px 3px rgba(var(--shadow-rgb), 0.24)",
    "--shadow-lg": "0 8px 24px rgba(var(--shadow-rgb), 0.44), 0 2px 6px rgba(var(--shadow-rgb), 0.28)",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
    // 静谧：中性近黑底上极淡金主光晕(顶) + 极淡蓝侧光晕(左)，去金星点(C 极简)
    "--app-bg-overlay":
      "radial-gradient(80% 50% at 50% -12%, rgba(255, 217, 138, 0.10), transparent 60%), " +
      "radial-gradient(56% 42% at 10% 20%, rgba(59, 160, 255, 0.08), transparent 58%)",
    // ── 色（静谧 · 中性近黑底 + 金主蓝辅） ──
    "--background": "#0a0a0c",
    "--foreground": "#ededf0",
    "--card": "#161619",
    "--card-foreground": "#ededf0",
    "--popover": "#1c1c20",
    "--popover-foreground": "#ededf0",
    "--primary": "#ffd98a",
    "--primary-foreground": "#1a1206",
    "--secondary": "#1c1c20",
    "--secondary-foreground": "#ededf0",
    "--muted": "#1c1c20",
    "--muted-foreground": "#9a9aa3",
    "--accent": "#20242c",
    "--accent-foreground": "#7cc0ff",
    "--destructive": "#e0644a",
    "--destructive-foreground": "#ffffff",
    "--border": "rgba(255, 255, 255, 0.08)",
    "--input": "rgba(255, 255, 255, 0.10)",
    "--ring": "rgba(255, 217, 138, 0.45)",
    "--shadow-color": "0, 0, 0",
  },
};
