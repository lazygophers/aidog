import type { ModelSlot } from "../../services/api";

/** 预估 coding plan JSON 结构（后端 est_coding_plan 列） */
export interface EstCodingTier {
  name: string;
  est_utilization: number;
  coef_per_token: number;
  util_at_last_real: number;
  tokens_since_real: number;
  has_base: boolean;
  limit?: number;
  /** 本周期起点 unix ms（系统维护）；0/缺失 = 无可靠周期起点 → 配色中性。 */
  window_start?: number;
}
export interface EstCodingPlan {
  tiers: EstCodingTier[];
  level: string | null;
}

/** 安全解析 est_coding_plan JSON；非法/空串返回 null */
export function parseEstCodingPlan(raw: string): EstCodingPlan | null {
  if (!raw || !raw.trim()) return null;
  try {
    const obj = JSON.parse(raw) as Partial<EstCodingPlan>;
    if (!obj || !Array.isArray(obj.tiers)) return null;
    return { tiers: obj.tiers as EstCodingTier[], level: obj.level ?? null };
  } catch {
    return null;
  }
}

/** 根据模型名模式自动分配到槽位 */
export function autoCategorize(modelIds: string[]): Record<ModelSlot, string> {
  const result: Record<ModelSlot, string> = {
    default: "", sonnet: "", opus: "", haiku: "", gpt: "",
  };
  const patterns: { slot: ModelSlot; test: (id: string) => boolean }[] = [
    { slot: "opus", test: (id) => /opus/i.test(id) },
    { slot: "sonnet", test: (id) => /sonnet/i.test(id) },
    { slot: "haiku", test: (id) => /haiku/i.test(id) },
    { slot: "gpt", test: (id) => /gpt/i.test(id) && !/mini/i.test(id) },
  ];
  const assigned = new Set<string>();
  for (const { slot, test } of patterns) {
    for (const id of modelIds) {
      if (test(id) && !assigned.has(id)) {
        result[slot] = id;
        assigned.add(id);
      }
    }
  }
  const first = modelIds.find(id => !assigned.has(id)) ?? modelIds[0];
  if (first && !result.default) result.default = first;
  return result;
}
