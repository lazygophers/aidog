import type { SettingsPreset } from "./api";

/** 默认预设 id：首次填充 / 字段「与推荐值不同」/ 重置默认三处共用。 */
export const DEFAULT_PRESET_ID = "oss";

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
