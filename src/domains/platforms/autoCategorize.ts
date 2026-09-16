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

/** 上游响应头里的速率限制余量（后端 rate_limit 列）。与 EstCodingPlan 的「周期内套餐额度」
 *  不是一个维度：这里是「这一分钟还能发几个请求」，由上游每次响应带回来。 */
export interface RateLimitSnapshot {
  /** "anthropic" | "openai" | "openrouter" */
  vendor: string;
  requests_remaining?: number;
  requests_limit?: number;
  tokens_remaining?: number;
  tokens_limit?: number;
  /** 窗口重置时刻 unix ms */
  resets_at?: number;
  /** 观测时刻 unix ms，用于判断快照是否已过期 */
  observed_at: number;
}

/** 安全解析 rate_limit JSON；非法/空串返回 null */
export function parseRateLimit(raw: string): RateLimitSnapshot | null {
  if (!raw || !raw.trim()) return null;
  try {
    const obj = JSON.parse(raw) as Partial<RateLimitSnapshot>;
    if (!obj || typeof obj.vendor !== "string") return null;
    return obj as RateLimitSnapshot;
  } catch {
    return null;
  }
}

/** 速率限制余量占比 0-1；厂商没给 limit 或 remaining 时返回 null（不拿 0 冒充） */
export function rateLimitRatio(rl: RateLimitSnapshot): number | null {
  const pairs: [number | undefined, number | undefined][] = [
    [rl.requests_remaining, rl.requests_limit],
    [rl.tokens_remaining, rl.tokens_limit],
  ];
  const ratios = pairs
    .filter(([rem, lim]) => typeof rem === "number" && typeof lim === "number" && lim > 0)
    .map(([rem, lim]) => (rem as number) / (lim as number));
  // 取最紧的那一维：请求数和 token 任一见底，实际就发不出去了
  return ratios.length > 0 ? Math.min(...ratios) : null;
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
