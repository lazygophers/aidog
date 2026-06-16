import type { StyleDefinition } from "../types";

/**
 * Terminal 终端
 * 近无圆角 + 无模糊 + accent 半透明边（color-mix 引用 palette）+ 单像素环投影。
 * --app-bg-overlay = 扫描线 repeating-linear-gradient（用 shadow-rgb 低 alpha），CRT 质感。
 * 注：理想要等宽字体，但字体非主题变量，不在本任务范围。
 */
export const terminal: StyleDefinition = {
  id: "terminal",
  label: "theme.style.terminal",
  light: {
    "--radius-sm": "0px",
    "--radius-md": "2px",
    "--radius-lg": "2px",
    "--radius-xl": "4px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1px solid color-mix(in srgb, var(--accent-1) 50%, transparent)",
    "--shadow-sm": "0 0 0 1px rgba(var(--shadow-rgb), 0.4)",
    "--shadow-md": "0 0 0 1px rgba(var(--shadow-rgb), 0.4)",
    "--shadow-lg": "0 0 0 1px rgba(var(--shadow-rgb), 0.4)",
    "--transition": "80ms linear",
    "--app-bg-overlay":
      "repeating-linear-gradient(0deg, rgba(var(--shadow-rgb), 0.03) 0 1px, transparent 1px 3px)",
  },
  dark: {
    "--radius-sm": "0px",
    "--radius-md": "2px",
    "--radius-lg": "2px",
    "--radius-xl": "4px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "1px solid color-mix(in srgb, var(--accent-1) 50%, transparent)",
    "--shadow-sm": "0 0 0 1px rgba(var(--shadow-rgb), 0.4)",
    "--shadow-md": "0 0 0 1px rgba(var(--shadow-rgb), 0.4)",
    "--shadow-lg": "0 0 0 1px rgba(var(--shadow-rgb), 0.4)",
    "--transition": "80ms linear",
    "--app-bg-overlay":
      "repeating-linear-gradient(0deg, rgba(var(--shadow-rgb), 0.03) 0 1px, transparent 1px 3px)",
  },
};
