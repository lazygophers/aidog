export type ThemeMode = "light" | "dark";

/** 单一主题定义：light / dark 两组完整 CSS 变量（结构 + 语义色）。 */
export interface ThemeDefinition {
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
