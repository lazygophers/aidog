export type ThemeMode = "light" | "dark";

/** 结构/材质轴：style 决定圆角 / 模糊 / 阴影形态 / 过渡。 */
export type ThemeStyle =
  | "liquidGlass"
  | "flat"
  | "soft"
  | "sharp"
  | "aurora"
  | "paper"
  | "terminal"
  | "bento"
  | "sketchy";

/** 调色板轴：color 决定全部色彩变量 + shadow-color。4 个业界知名命名色板。 */
export type ThemeColor = "gruvbox" | "nord" | "dracula" | "catppuccin";

/** Style 定义：仅结构变量（radius/blur/saturate/glass-border/shadow/transition）。 */
export interface StyleDefinition {
  id: ThemeStyle;
  /** i18n key，如 "theme.style.liquidGlass"。 */
  label: string;
  light: Record<string, string>;
  dark: Record<string, string>;
}

/** Palette 定义：仅 shadcn 语义色彩变量 + --shadow-color。 */
export interface PaletteDefinition {
  id: ThemeColor;
  /** i18n key，如 "theme.color.appleBlue"。 */
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
 * 清除给定变量集
 */
export function clearThemeVars(vars: Record<string, string>) {
  const root = document.documentElement;
  for (const key of Object.keys(vars)) {
    root.style.removeProperty(key);
  }
}

/**
 * 按 key 名清除一组变量（用于 clear 全量已知键并集，避免切换残留）。
 */
export function clearThemeKeys(keys: Iterable<string>) {
  const root = document.documentElement;
  for (const key of keys) {
    root.style.removeProperty(key);
  }
}
