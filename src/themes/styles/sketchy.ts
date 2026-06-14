import type { StyleDefinition } from "../types";

/**
 * Sketchy 手绘
 * 不规则多值圆角（手绘抖动感）+ 无模糊 + 墨线描边（text-primary）+ 马克笔偏移硬投影。
 * --app-bg-overlay 默认 none。
 * 注：理想要手写字体，但字体非主题变量，不在本任务范围；仅做圆角/边/投影表达。
 */
export const sketchy: StyleDefinition = {
  id: "sketchy",
  label: "theme.style.sketchy",
  light: {
    "--radius-sm": "8px 6px 9px 7px",
    "--radius-md": "12px 10px 14px 8px",
    "--radius-lg": "18px 14px 20px 16px",
    "--radius-xl": "26px 20px 28px 22px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "2px solid var(--text-primary)",
    "--shadow-sm": "2px 3px 0 rgba(var(--shadow-rgb), 0.7)",
    "--shadow-md": "2px 3px 0 rgba(var(--shadow-rgb), 0.7)",
    "--shadow-lg": "2px 3px 0 rgba(var(--shadow-rgb), 0.7)",
    "--transition": "120ms ease",
    "--app-bg-overlay": "none",
  },
  dark: {
    "--radius-sm": "8px 6px 9px 7px",
    "--radius-md": "12px 10px 14px 8px",
    "--radius-lg": "18px 14px 20px 16px",
    "--radius-xl": "26px 20px 28px 22px",
    "--glass-blur": "0px",
    "--glass-saturate": "1",
    "--glass-border": "2px solid var(--text-primary)",
    "--shadow-sm": "2px 3px 0 rgba(var(--shadow-rgb), 0.7)",
    "--shadow-md": "2px 3px 0 rgba(var(--shadow-rgb), 0.7)",
    "--shadow-lg": "2px 3px 0 rgba(var(--shadow-rgb), 0.7)",
    "--transition": "120ms ease",
    "--app-bg-overlay": "none",
  },
};
