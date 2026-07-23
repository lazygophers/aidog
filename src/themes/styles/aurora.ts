import type { StyleDefinition } from "../types";

/**
 * Aurora 极光
 * 大圆角 + 高 blur 柔光毛玻璃 + 多层 color-mix 流光背景（消费 palette 的 --accent-1..3）。
 * --app-bg-overlay 是本 style 的核心：app 背景叠加色系多色径向/线性渐变层，
 * 用 color-mix(in srgb, var(--accent-N) α%, transparent) 引用 palette 协调色，禁写死家族色。
 * dark 模式 alpha 略低，避免过曝。
 */
export const aurora: StyleDefinition = {
  id: "aurora",
  label: "theme.style.aurora",
  light: {
    "--radius-sm": "12px",
    "--radius-md": "16px",
    "--radius-lg": "22px",
    "--radius-xl": "30px",
    "--glass-blur": "30px",
    "--glass-saturate": "1.8",
    "--glass-border": "1px solid var(--glass-edge)",
    "--shadow-sm": "0 1px 3px rgba(var(--shadow-rgb), 0.05)",
    "--shadow-md": "0 4px 16px rgba(var(--shadow-rgb), 0.08)",
    "--shadow-lg": "0 8px 32px rgba(var(--shadow-rgb), 0.10)",
    "--transition": "300ms cubic-bezier(0.4, 0, 0.2, 1)",
    "--app-bg-overlay":
      "radial-gradient(70% 60% at 15% 0%, color-mix(in srgb, var(--accent-1) 38%, transparent), transparent), " +
      "radial-gradient(60% 55% at 95% 12%, color-mix(in srgb, var(--accent-3) 32%, transparent), transparent), " +
      "radial-gradient(55% 50% at 60% 90%, color-mix(in srgb, var(--accent-2) 24%, transparent), transparent), " +
      "linear-gradient(160deg, color-mix(in srgb, var(--accent-2) 14%, transparent), transparent)",
  },
  dark: {
    "--radius-sm": "12px",
    "--radius-md": "16px",
    "--radius-lg": "22px",
    "--radius-xl": "30px",
    "--glass-blur": "30px",
    "--glass-saturate": "1.6",
    "--glass-border": "1px solid var(--glass-edge)",
    "--shadow-sm": "0 1px 3px rgba(var(--shadow-rgb), 0.3)",
    "--shadow-md": "0 4px 16px rgba(var(--shadow-rgb), 0.4)",
    "--shadow-lg": "0 8px 32px rgba(var(--shadow-rgb), 0.45)",
    "--transition": "300ms cubic-bezier(0.4, 0, 0.2, 1)",
    "--app-bg-overlay":
      "radial-gradient(70% 60% at 15% 0%, color-mix(in srgb, var(--accent-1) 32%, transparent), transparent), " +
      "radial-gradient(60% 55% at 95% 12%, color-mix(in srgb, var(--accent-3) 26%, transparent), transparent), " +
      "radial-gradient(55% 50% at 60% 90%, color-mix(in srgb, var(--accent-2) 20%, transparent), transparent), " +
      "linear-gradient(160deg, color-mix(in srgb, var(--accent-2) 12%, transparent), transparent)",
  },
};
