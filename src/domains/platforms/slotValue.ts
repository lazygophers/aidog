// 槽位值（PlatformModels 已改有序列表）与 UI 单值表单态的互转。
// 票 01 最小适配：表单仍是单值，读取取链首、写回单元素数组；多选 UI 在票 03。
// 兼容读取：registry 默认值（票 02 前仍是字符串）与旧形态一并接受。
import type { ModelSlot } from "../../services/api/types/manual";
import type { PlatformModels } from "../../services/api/types/generated/PlatformModels";

export type SlotValue = string | string[] | null | undefined;

const SLOTS: ModelSlot[] = ["default", "sonnet", "opus", "haiku", "gpt", "jev", "embedding", "rerank"];

/** 槽位完整链：trim、去空、去重保序（与 Rust normalize_model_list 同口径）。 */
export function slotList(v: SlotValue): string[] {
  const raw = Array.isArray(v) ? v : v ? [v] : [];
  const out: string[] = [];
  for (const s of raw) {
    const t = (s ?? "").trim();
    if (t && !out.includes(t)) out.push(t);
  }
  return out;
}

/** 槽位主模型（链首），无则 ""。 */
export const slotPrimary = (v: SlotValue): string => slotList(v)[0] ?? "";

/** 后端/registry 槽位表 → 单值表单态（取链首）。 */
export function slotsToForm(m: Partial<Record<ModelSlot, SlotValue>> | undefined): Record<ModelSlot, string> {
  const out = {} as Record<ModelSlot, string>;
  for (const k of SLOTS) out[k] = slotPrimary(m?.[k]);
  return out;
}

/** 单值表单态 → PlatformModels（非空槽写单元素数组，空槽省略）。 */
export function formToSlots(f: Partial<Record<ModelSlot, string>>): PlatformModels {
  const out: PlatformModels = {};
  for (const k of SLOTS) {
    const v = slotPrimary(f[k]);
    if (v) out[k] = [v];
  }
  return out;
}

/** 后端/registry 槽位表 → 文本表单态（整条链，", " 连接）。批量覆盖用，不降成单值。 */
export function slotsToText(m: Partial<Record<ModelSlot, SlotValue>> | undefined): Record<ModelSlot, string> {
  const out = {} as Record<ModelSlot, string>;
  for (const k of SLOTS) out[k] = slotList(m?.[k]).join(", ");
  return out;
}

/** 文本表单态（逗号分隔）→ PlatformModels；空槽省略。 */
export function textToSlots(f: Partial<Record<ModelSlot, string>>): PlatformModels {
  const out: PlatformModels = {};
  for (const k of SLOTS) {
    const l = slotList((f[k] ?? "").split(/[,，]/));
    if (l.length) out[k] = l;
  }
  return out;
}
