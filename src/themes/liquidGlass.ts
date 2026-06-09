import type { ThemeDefinition } from "./types";

/**
 * Liquid Glass 主题
 * 深化 Apple Vision Pro / macOS Tahoe 设计语言
 * — 多层半透明毛玻璃
 * — 内发光折射边缘
 * — 深度阴影层次
 * — 渐变 accent
 */
export const liquidGlass: ThemeDefinition = {
  name: "liquidGlass",
  label: "theme.liquidGlass",
  light: {
    "--bg-base": "#f0f0f3",
    "--bg-elevated": "rgba(255, 255, 255, 0.82)",
    "--bg-glass": "rgba(255, 255, 255, 0.55)",
    "--bg-glass-hover": "rgba(255, 255, 255, 0.72)",
    "--bg-surface": "rgba(255, 255, 255, 0.88)",
    "--text-primary": "rgba(0, 0, 0, 0.88)",
    "--text-secondary": "rgba(0, 0, 0, 0.5)",
    "--text-tertiary": "rgba(0, 0, 0, 0.3)",
    "--accent": "#007AFF",
    "--accent-hover": "#0056CC",
    "--accent-subtle": "rgba(0, 122, 255, 0.1)",
    "--border": "rgba(0, 0, 0, 0.06)",
    "--border-focus": "rgba(0, 122, 255, 0.4)",
    "--shadow-sm": "0 1px 3px rgba(0, 0, 0, 0.04), 0 1px 2px rgba(0, 0, 0, 0.02)",
    "--shadow-md": "0 4px 12px rgba(0, 0, 0, 0.06), 0 1px 4px rgba(0, 0, 0, 0.04)",
    "--shadow-lg": "0 12px 40px rgba(0, 0, 0, 0.08), 0 4px 12px rgba(0, 0, 0, 0.04)",
    "--glass-blur": "24px",
    "--glass-saturate": "1.8",
    "--glass-border": "1px solid rgba(255, 255, 255, 0.35)",
    "--radius-sm": "10px",
    "--radius-md": "14px",
    "--radius-lg": "20px",
    "--radius-xl": "28px",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
  },
  dark: {
    "--bg-base": "#0a0a0c",
    "--bg-elevated": "rgba(30, 30, 34, 0.8)",
    "--bg-glass": "rgba(44, 44, 50, 0.4)",
    "--bg-glass-hover": "rgba(55, 55, 62, 0.55)",
    "--bg-surface": "rgba(28, 28, 32, 0.85)",
    "--text-primary": "rgba(255, 255, 255, 0.93)",
    "--text-secondary": "rgba(255, 255, 255, 0.55)",
    "--text-tertiary": "rgba(255, 255, 255, 0.3)",
    "--accent": "#4A9EFF",
    "--accent-hover": "#6BB3FF",
    "--accent-subtle": "rgba(74, 158, 255, 0.12)",
    "--border": "rgba(255, 255, 255, 0.06)",
    "--border-focus": "rgba(74, 158, 255, 0.45)",
    "--shadow-sm": "0 1px 3px rgba(0, 0, 0, 0.3), 0 1px 2px rgba(0, 0, 0, 0.2)",
    "--shadow-md": "0 4px 12px rgba(0, 0, 0, 0.4), 0 1px 4px rgba(0, 0, 0, 0.3)",
    "--shadow-lg": "0 12px 40px rgba(0, 0, 0, 0.5), 0 4px 12px rgba(0, 0, 0, 0.3)",
    "--glass-blur": "24px",
    "--glass-saturate": "1.6",
    "--glass-border": "1px solid rgba(255, 255, 255, 0.07)",
    "--radius-sm": "10px",
    "--radius-md": "14px",
    "--radius-lg": "20px",
    "--radius-xl": "28px",
    "--transition": "250ms cubic-bezier(0.4, 0, 0.2, 1)",
  },
};
