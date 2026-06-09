export { liquidGlass } from "./liquidGlass";
export type { ThemeDefinition, ThemeMode, ThemeName } from "./types";
export { applyThemeVars, clearThemeVars } from "./types";

import { liquidGlass } from "./liquidGlass";
import type { ThemeDefinition, ThemeMode, ThemeName } from "./types";
import { applyThemeVars, clearThemeVars } from "./types";

const themeMap: Record<ThemeName, ThemeDefinition> = {
  liquidGlass,
};

/** 获取所有可用主题 */
export function getAvailableThemes(): ThemeDefinition[] {
  return Object.values(themeMap);
}

/** 应用主题：name + mode → 写入 CSS 变量 */
export function applyTheme(name: ThemeName, mode: ThemeMode) {
  const theme = themeMap[name];
  if (!theme) return;

  // 先清旧变量
  clearThemeVars(theme.light);
  clearThemeVars(theme.dark);

  // 写新变量
  applyThemeVars(theme[mode]);

  // 同步 data 属性
  document.documentElement.setAttribute("data-theme", name);
  document.documentElement.setAttribute("data-mode", mode);
}
