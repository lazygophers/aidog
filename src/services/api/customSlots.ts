// customSlots.ts — 自定义模型槽位定义（全局 settings scope "model" / key "custom_model_slots"）。
// 各平台的值在 platform.extra.custom_slots[槽名]（模型链），删除级联由 custom_slot_purge 完成。

import { invoke } from "../transport";

export type CustomSlotMatch = "contains" | "exact";
export interface CustomSlotDef {
  name: string;
  match: CustomSlotMatch;
}

const SCOPE = "model";
const KEY = "custom_model_slots";

export const customSlotsApi = {
  get: async (): Promise<CustomSlotDef[]> =>
    (await invoke<CustomSlotDef[] | null>("settings_get", { scope: SCOPE, key: KEY })) ?? [],
  set: (list: CustomSlotDef[]) =>
    invoke<void>("settings_set", { input: { scope: SCOPE, key: KEY, value: list } }),
  /** extra.custom_slots 含该槽名的平台数。 */
  usage: (name: string) => invoke<number>("custom_slot_usage_count", { name }),
  /** 从所有平台 extra.custom_slots 删除该槽名，返回受影响平台数。 */
  purge: (name: string) => invoke<number>("custom_slot_purge", { name }),
};
