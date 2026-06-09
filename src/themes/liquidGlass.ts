import type { ThemeDefinition } from "./types";

/**
 * Liquid Glass 主题
 * 灵感来自 Apple WWDC 2025 设计语言
 * - 半透明毛玻璃质感
 * - 柔和光影与折射效果
 * - 圆润边角 + 微妙阴影层次
 */
export const liquidGlass: ThemeDefinition = {
  name: "liquidGlass",
  label: "theme.liquidGlass",
  light: {
    "--bg-base": "rgba(245, 245, 247, 0.85)",
    "--bg-elevated": "rgba(255, 255, 255, 0.72)",
    "--bg-glass": "rgba(255, 255, 255, 0.55)",
    "--bg-glass-hover": "rgba(255, 255, 255, 0.72)",
    "--bg-surface": "rgba(255, 255, 255, 0.9)",
    "--text-primary": "rgba(0, 0, 0, 0.88)",
    "--text-secondary": "rgba(0, 0, 0, 0.55)",
    "--text-tertiary": "rgba(0, 0, 0, 0.35)",
    "--accent": "rgba(0, 122, 255, 0.9)",
    "--accent-hover": "rgba(0, 100, 220, 1)",
    "--accent-subtle": "rgba(0, 122, 255, 0.12)",
    "--border": "rgba(0, 0, 0, 0.08)",
    "--border-focus": "rgba(0, 122, 255, 0.5)",
    "--shadow-sm": "0 1px 2px rgba(0, 0, 0, 0.04)",
    "--shadow-md": "0 4px 16px rgba(0, 0, 0, 0.06)",
    "--shadow-lg": "0 8px 32px rgba(0, 0, 0, 0.08)",
    "--glass-blur": "20px",
    "--glass-saturate": "1.8",
    "--glass-border": "1px solid rgba(255, 255, 255, 0.35)",
    "--radius-sm": "8px",
    "--radius-md": "12px",
    "--radius-lg": "18px",
    "--radius-xl": "24px",
    "--transition": "200ms cubic-bezier(0.4, 0, 0.2, 1)",
  },
  dark: {
    "--bg-base": "rgba(28, 28, 30, 0.85)",
    "--bg-elevated": "rgba(44, 44, 46, 0.72)",
    "--bg-glass": "rgba(58, 58, 60, 0.45)",
    "--bg-glass-hover": "rgba(58, 58, 60, 0.65)",
    "--bg-surface": "rgba(36, 36, 38, 0.9)",
    "--text-primary": "rgba(255, 255, 255, 0.92)",
    "--text-secondary": "rgba(255, 255, 255, 0.6)",
    "--text-tertiary": "rgba(255, 255, 255, 0.35)",
    "--accent": "rgba(64, 156, 255, 0.9)",
    "--accent-hover": "rgba(80, 170, 255, 1)",
    "--accent-subtle": "rgba(64, 156, 255, 0.15)",
    "--border": "rgba(255, 255, 255, 0.08)",
    "--border-focus": "rgba(64, 156, 255, 0.5)",
    "--shadow-sm": "0 1px 2px rgba(0, 0, 0, 0.2)",
    "--shadow-md": "0 4px 16px rgba(0, 0, 0, 0.3)",
    "--shadow-lg": "0 8px 32px rgba(0, 0, 0, 0.4)",
    "--glass-blur": "20px",
    "--glass-saturate": "1.6",
    "--glass-border": "1px solid rgba(255, 255, 255, 0.1)",
    "--radius-sm": "8px",
    "--radius-md": "12px",
    "--radius-lg": "18px",
    "--radius-xl": "24px",
    "--transition": "200ms cubic-bezier(0.4, 0, 0.2, 1)",
  },
};
