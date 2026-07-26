import type { ThemeDefinition } from "./types";

/**
 * Mono · 萤火虫玻璃（2026-07-26 重设计，照搬 example 萤火虫配色规范）。
 * light「奶油纸白 + 萤火虫暖光」：纯白底 + 暖琥珀 primary (#c49a3c) + 奶油白卡面 + 莫兰迪语义色。
 * dark「纯黑 + 萤火虫微光」：纯黑底 + 亮萤火虫 primary (#e8c547) + 深岩卡面 + 莫兰迪语义色。
 * 签名色 = 萤火虫暖琥珀，dark 下更亮；语义色全部去饱和柔和化（莫兰迪）。
 * 蓝金流光描边由 globals.css .glass:hover conic flow-border 呈现，已改萤火虫暖光序列。
 * ponytail: 莫兰迪去饱和语义色对比度低于原饱和色，可访问性退化（用户明确选完全照搬萤火虫）；
 *   若后续需 WCAG AA 兜底，success/warning/danger 可朝 example soft 系列提饱和 10-15%。
 * 单文件同时持结构变量(radius/blur/shadow) + shadcn 语义色 token。
 */
export const mono: ThemeDefinition = {
  light: {
    // ── 结构 ──
    "--radius-sm": "8px",
    "--radius-md": "12px",
    "--radius-lg": "16px",
    "--radius-xl": "24px",
    "--glass-blur": "20px",
    "--glass-saturate": "1.4",
    "--glass-border": "1px solid var(--glass-edge)",
    // 奶油纸白：柔阴影，低饱和暖灰
    "--shadow-sm": "0 1px 3px rgba(28, 25, 23, 0.04), 0 1px 2px rgba(28, 25, 23, 0.02)",
    "--shadow-md": "0 4px 20px rgba(28, 25, 23, 0.06)",
    "--shadow-lg": "0 8px 32px rgba(28, 25, 23, 0.08)",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
    // 萤火虫暖光：纯白底 + 暖琥珀顶光晕 + 极淡暖金侧光
    "--app-bg-overlay":
      "radial-gradient(72% 52% at 50% -10%, rgba(196, 154, 60, 0.10), transparent 62%), " +
      "radial-gradient(52% 44% at 92% 8%, rgba(212, 184, 122, 0.12), transparent 60%), " +
      "radial-gradient(60% 50% at 6% 100%, rgba(196, 154, 60, 0.06), transparent 64%)",
    // ── 色（萤火虫 · 暖琥珀） ──
    "--background": "#ffffff",
    "--foreground": "#1c1917",
    "--card": "#faf8f5",
    "--card-foreground": "#1c1917",
    "--popover": "#ffffff",
    "--popover-foreground": "#1c1917",
    "--primary": "#c49a3c",
    "--primary-foreground": "#ffffff",
    "--secondary": "#f5f2ed",
    "--secondary-foreground": "#1c1917",
    "--muted": "#f5f2ed",
    "--muted-foreground": "#78716c",
    "--accent": "#d4b87a",
    "--accent-foreground": "#5a4a1e",
    "--destructive": "#c47a7a",
    "--destructive-foreground": "#ffffff",
    "--border": "rgba(28, 25, 23, 0.09)",
    "--input": "rgba(28, 25, 23, 0.09)",
    "--ring": "rgba(196, 154, 60, 0.40)",
    "--shadow-color": "28, 25, 23",
  },
  dark: {
    // ── 结构 ──
    "--radius-sm": "8px",
    "--radius-md": "12px",
    "--radius-lg": "16px",
    "--radius-xl": "24px",
    "--glass-blur": "20px",
    "--glass-saturate": "1.4",
    "--glass-border": "1px solid var(--glass-edge)",
    // 纯黑：深阴影
    "--shadow-sm": "0 1px 3px rgba(0, 0, 0, 0.4)",
    "--shadow-md": "0 4px 20px rgba(0, 0, 0, 0.5)",
    "--shadow-lg": "0 8px 32px rgba(0, 0, 0, 0.6)",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
    // 萤火虫微光：纯黑底 + 亮萤火虫顶光晕 + 极淡暖金侧光
    "--app-bg-overlay":
      "radial-gradient(80% 50% at 50% -12%, rgba(232, 197, 71, 0.10), transparent 60%), " +
      "radial-gradient(56% 42% at 10% 20%, rgba(232, 197, 71, 0.06), transparent 58%)",
    // ── 色（萤火虫 · 暗夜更亮） ──
    "--background": "#000000",
    "--foreground": "#f5f5f0",
    "--card": "#0c0c0c",
    "--card-foreground": "#f5f5f0",
    "--popover": "#0c0c0c",
    "--popover-foreground": "#f5f5f0",
    "--primary": "#e8c547",
    "--primary-foreground": "#1a1206",
    "--secondary": "#1a1a1a",
    "--secondary-foreground": "#f5f5f0",
    "--muted": "#1a1a1a",
    "--muted-foreground": "#8a8580",
    "--accent": "#c4a83a",
    "--accent-foreground": "#f5f5f0",
    "--destructive": "#b07070",
    "--destructive-foreground": "#ffffff",
    "--border": "rgba(255, 255, 255, 0.08)",
    "--input": "rgba(255, 255, 255, 0.10)",
    "--ring": "rgba(232, 197, 71, 0.45)",
    "--shadow-color": "0, 0, 0",
  },
};
