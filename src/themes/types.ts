export type ThemeMode = "light" | "dark";

export type ThemeName = "liquidGlass" | "nord" | "dracula" | "catppuccin" | "solarized";

export interface ThemeDefinition {
  name: ThemeName;
  label: string;
  light: Record<string, string>;
  dark: Record<string, string>;
}

/**
 * 将主题变量应用到 document 根元素
 */
export function applyThemeVars(vars: Record<string, string>) {
  const root = document.documentElement;
  for (const [key, value] of Object.entries(vars)) {
    root.style.setProperty(key, value);
  }
}

/**
 * 清除所有主题变量
 */
export function clearThemeVars(vars: Record<string, string>) {
  const root = document.documentElement;
  for (const key of Object.keys(vars)) {
    root.style.removeProperty(key);
  }
}
