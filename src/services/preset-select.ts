import type { SettingsPreset } from "./api";

/** 默认预设 id：首次填充 / 字段「与推荐值不同」/ 重置默认三处共用。 */
export const DEFAULT_PRESET_ID = "oss";

/** 把预设里的 `{{key}}` 占位符替换为 vars 值（深遍历字符串）；无占位符原样返回同一引用。 */
export function renderPresetPlaceholders<T>(config: T, vars: Record<string, string | number>): T {
  if (!JSON.stringify(config).includes("{{")) return config;
  const walk = (v: unknown): unknown => {
    if (typeof v === "string") {
      return v.replace(/\{\{(\w+)\}\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
    }
    if (Array.isArray(v)) return v.map(walk);
    if (v && typeof v === "object") {
      return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, walk(x)]));
    }
    return v;
  };
  return walk(config) as T;
}

export interface PresetLoadPlan {
  /** ≥2 份才弹选择框；0/1 份不弹。 */
  needPicker: boolean;
  /** oss 优先，缺失回落第一份；列表空则 undefined。 */
  defaultPreset: SettingsPreset | undefined;
}

export function planPresetLoad(list: SettingsPreset[]): PresetLoadPlan {
  return {
    needPicker: list.length >= 2,
    defaultPreset: list.find((p) => p.id === DEFAULT_PRESET_ID) ?? list[0],
  };
}
