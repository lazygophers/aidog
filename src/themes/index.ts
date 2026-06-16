import { liquidGlass } from "./styles/liquidGlass";
import { flat } from "./styles/flat";
import { soft } from "./styles/soft";
import { sharp } from "./styles/sharp";
import { aurora } from "./styles/aurora";
import { paper } from "./styles/paper";
import { terminal } from "./styles/terminal";
import { bento } from "./styles/bento";
import { sketchy } from "./styles/sketchy";

import { appleBlue } from "./palettes/appleBlue";
import { nord } from "./palettes/nord";
import { dracula } from "./palettes/dracula";
import { catppuccin } from "./palettes/catppuccin";
import { solarized } from "./palettes/solarized";
import { rosePine } from "./palettes/rosePine";
import { tokyoNight } from "./palettes/tokyoNight";
import { gruvbox } from "./palettes/gruvbox";
import { oneDark } from "./palettes/oneDark";
import { material } from "./palettes/material";
import { github } from "./palettes/github";
import { nightOwl } from "./palettes/nightOwl";

import {
  type ThemeMode,
  type ThemeStyle,
  type ThemeColor,
  type StyleDefinition,
  type PaletteDefinition,
  applyThemeVars,
  clearThemeKeys,
} from "./types";

export type {
  ThemeMode,
  ThemeStyle,
  ThemeColor,
  StyleDefinition,
  PaletteDefinition,
} from "./types";
export { applyThemeVars, clearThemeVars, clearThemeKeys } from "./types";

/** Style 注册表（结构轴）。 */
const styleMap: Record<ThemeStyle, StyleDefinition> = {
  liquidGlass,
  flat,
  soft,
  sharp,
  aurora,
  paper,
  terminal,
  bento,
  sketchy,
};

/**
 * Palette 注册表（色彩轴）。12 个均为业界知名命名色板。
 */
const paletteMap: Partial<Record<ThemeColor, PaletteDefinition>> = {
  appleBlue,
  nord,
  dracula,
  catppuccin,
  solarized,
  rosePine,
  tokyoNight,
  gruvbox,
  oneDark,
  material,
  github,
  nightOwl,
};

/** 默认轴值（迁移失败 / 未注册时回退）。 */
export const DEFAULT_STYLE: ThemeStyle = "aurora";
export const DEFAULT_COLOR: ThemeColor = "gruvbox";
export const DEFAULT_MODE: ThemeMode = "dark";

/** 获取所有可用 style（结构轴）。 */
export function getAvailableStyles(): StyleDefinition[] {
  return Object.values(styleMap);
}

/** 获取所有可用 color（色彩轴，仅已注册的）。阶段 2 注册新文件后自动出现。 */
export function getAvailableColors(): PaletteDefinition[] {
  return Object.values(paletteMap).filter(
    (p): p is PaletteDefinition => p != null,
  );
}

/** 解析 style，未注册回退默认。 */
function resolveStyle(style: ThemeStyle): StyleDefinition {
  return styleMap[style] ?? styleMap[DEFAULT_STYLE];
}

/** 解析 palette，未注册回退默认。 */
function resolvePalette(color: ThemeColor): PaletteDefinition {
  return paletteMap[color] ?? paletteMap[DEFAULT_COLOR]!;
}

/**
 * 全量已知变量键并集（style ∪ palette 的所有 light/dark key）。
 * apply 前先清此并集，避免从某组合切到另一组合时残留旧变量（如 liquidGlass→flat 后 blur 残留）。
 */
const ALL_KNOWN_KEYS: Set<string> = (() => {
  const keys = new Set<string>();
  const collect = (defs: { light: Record<string, string>; dark: Record<string, string> }[]) => {
    for (const d of defs) {
      for (const k of Object.keys(d.light)) keys.add(k);
      for (const k of Object.keys(d.dark)) keys.add(k);
    }
  };
  collect(Object.values(styleMap));
  collect(getAvailableColors());
  return keys;
})();

/**
 * 应用主题：style + color + mode → 合并写 CSS 变量。
 * 顺序：先 palette[mode]（提供 --shadow-rgb/--glass-edge/色彩），再 style[mode]（结构变量引用 palette 提供的色）。
 */
export function applyTheme(style: ThemeStyle, color: ThemeColor, mode: ThemeMode) {
  const styleDef = resolveStyle(style);
  const paletteDef = resolvePalette(color);

  // 清全量已知键并集，避免切换残留
  clearThemeKeys(ALL_KNOWN_KEYS);

  // palette 先于 style：style 的 shadow/glass-border 引用 palette 的 --shadow-rgb/--glass-edge
  applyThemeVars(paletteDef[mode]);
  applyThemeVars(styleDef[mode]);

  // 同步 data 属性（用解析后真实 id，保证迁移回退也写对）
  const root = document.documentElement;
  root.setAttribute("data-theme-style", styleDef.id);
  root.setAttribute("data-theme-color", paletteDef.id);
  root.setAttribute("data-mode", mode);
}
