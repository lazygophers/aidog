import { mono } from "./mono";
import { type ThemeMode, applyThemeVars } from "./types";

export type { ThemeMode, ThemeDefinition } from "./types";
export { applyThemeVars } from "./types";

/** 默认模式（黑）。配色轴收敛为 mode：dark=黑 / light=白。 */
export const DEFAULT_MODE: ThemeMode = "dark";

/**
 * 应用主题：唯一 mono 主题按 mode 写 CSS 变量。
 * light/dark 键集相同，切换无残留，无需 clear。
 */
export function applyTheme(mode: ThemeMode) {
  applyThemeVars(mono[mode]);
  document.documentElement.setAttribute("data-mode", mode);
}
