import type { Platform, PlatformQuota, ManualBudget, ManualBudgetUnit, ManualBudgetKind } from "../../services/api";
import { cycleMsForTier, codingTierLevel, type ColorLevel } from "../../components/shared";
import { MODEL_SLOTS } from "./constants";
import { parseEstCodingPlan } from "./autoCategorize";
import type { HealthStatus } from "./constants";

/** 判断平台健康状态：「成功即绿」语义 —— 最近 N 次请求中只要有一次成功即判健康，
 * 全失败才红，无请求灰。不返回 warning 中间态（避免「能用却显黄」），warning
 * 仅作类型成员保留供其它语义复用。 */
export function healthStatus(recentTotal: number, recentFailures: number): HealthStatus {
  if (recentTotal === 0) return "unknown";
  if (recentFailures >= recentTotal) return "error";        // 全部失败
  return "healthy";                                          // 有任一成功即绿
}

/** 从 PlatformModels 中提取所有非空值（去重） */
export function allModelValues(models: Platform["models"]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const slot of MODEL_SLOTS) {
    const v = models[slot.key];
    if (v && !seen.has(v)) {
      seen.add(v);
      result.push(v);
    }
  }
  return result;
}

/** 配额展示数据：合并预估(est_*)与真查(quotaMap)，优先级与原列表逻辑一致。
 *  手动刷新校准过(preferReal)→优先真值；否则有预估→预估；冷启动→真查回退。 */
export interface QuotaDisplay {
  estimated: boolean;
  /** 余额剩余（用于 BalanceBar）。null 表示无余额数据。 */
  balanceRemaining: number | null;
  balanceTotal: number | null;
  currency: string;
  /** coding plan 各档剩余百分比（0–100，越高越充足）。level 按使用速率算（usageColor，唯一阈值源）。 */
  tiers: { name: string; remainPct: number; utilization: number; resetsAt: string | null; limit: number | null; remaining: number | null; level: ColorLevel }[];
  /** 是否有任意配额数据（余额或 coding plan）。 */
  hasData: boolean;
}

export function computeQuotaDisplay(p: Platform, q: PlatformQuota | undefined, preferRealCalibrated: boolean): QuotaDisplay {
  const tierRemain = (utilization: number) => Math.max(0, Math.min(100, 100 - utilization));
  const preferReal = preferRealCalibrated && !!q;
  const estCoding = parseEstCodingPlan(p.est_coding_plan);
  const hasEstBalance = p.est_balance_remaining > 0;
  const hasEst = hasEstBalance || (estCoding !== null && estCoding.tiers.length > 0);

  if (hasEst && !preferReal) {
    const tiers = estCoding
      ? estCoding.tiers.map(tier => {
          const limit = tier.limit ?? null;
          const remaining = limit != null ? Math.round(limit * tierRemain(tier.est_utilization) / 100) : null;
          // 预估侧：remain = window_start + cycle - now（无 window_start → null → 中性）。
          const cycleMs = cycleMsForTier(tier.name);
          const remainMs = tier.window_start && tier.window_start > 0 && cycleMs != null
            ? tier.window_start + cycleMs - Date.now()
            : null;
          const level = codingTierLevel(tier.est_utilization, remainMs, cycleMs);
          const resetsAt = remainMs != null ? new Date(Date.now() + remainMs).toISOString() : null;
          return { name: tier.name, remainPct: tierRemain(tier.est_utilization), utilization: tier.est_utilization, resetsAt, limit, remaining, level };
        })
      : [];
    return {
      estimated: true,
      balanceRemaining: hasEstBalance ? p.est_balance_remaining : null,
      balanceTotal: null,
      currency: q?.balance?.currency || "USD",
      tiers,
      hasData: hasEstBalance || tiers.length > 0,
    };
  }
  if (q) {
    const tiers = q.coding_plan
      ? q.coding_plan.tiers.map(tier => {
          // 真查侧：remain = resets_at - now（无 resets_at → null → 中性）。
          const cycleMs = cycleMsForTier(tier.name);
          const resetsMs = tier.resets_at ? new Date(tier.resets_at).getTime() : NaN;
          const remainMs = Number.isFinite(resetsMs) && cycleMs != null ? resetsMs - Date.now() : null;
          const level = codingTierLevel(tier.utilization, remainMs, cycleMs);
          return { name: tier.name, remainPct: tierRemain(tier.utilization), utilization: tier.utilization, resetsAt: tier.resets_at, limit: tier.limit, remaining: tier.remaining, level };
        })
      : [];
    return {
      estimated: false,
      // ponytail: ACU (Devin) 无余额端点，balance.remaining 恒 0；改用 used (累计 ACU) 作展示值，
      //   total=null 抑制进度条，currency="ACU" 让 PlatformCard 标"ACU 用量" label 而非 $ 前缀。
      balanceRemaining: q.balance
        ? (q.balance.currency === "ACU" ? (q.balance.used ?? 0) : q.balance.remaining)
        : null,
      balanceTotal: q.balance?.currency === "ACU" ? null : (q.balance?.total ?? null),
      currency: q.balance?.currency || "USD",
      tiers,
      hasData: !!q.balance || tiers.length > 0,
    };
  }
  return { estimated: false, balanceRemaining: null, balanceTotal: null, currency: "USD", tiers: [], hasData: false };
}

/** coding plan 档名 → 简短标签 */
export function tierLabel(name: string): string {
  if (name === "five_hour") return "5h";
  if (name === "weekly_limit") return "week";
  if (name === "mcp_monthly") return "MCP";
  return name;
}

/** ISO 8601 或 millis → 剩余时间人类可读字符串 */
export function formatResetCountdown(resetsAt: string | null): string {
  if (!resetsAt) return "";
  const ts = new Date(resetsAt).getTime();
  if (isNaN(ts)) return "";
  const diffMs = ts - Date.now();
  if (diffMs <= 0) return "";
  const diffMin = Math.ceil(diffMs / 60000);
  const diffHours = Math.floor(diffMin / 60);
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays > 0) return `${diffDays}d ${diffHours % 24}h`;
  if (diffHours > 0) return `${diffHours}h ${diffMin % 60}m`;
  return `${diffMin}m`;
}

/** ISO 8601 → 绝对重置时间 clock：当天 `HH:mm`，跨天 `M/D HH:mm`，null/无效/已过期 → ""。
 *  ponytail: 月/日 数字格式跨 locale 通用，避「明天」i18n key。 */
export function formatResetClock(resetsAt: string | null): string {
  if (!resetsAt) return "";
  const ts = new Date(resetsAt).getTime();
  if (isNaN(ts)) return "";
  if (ts - Date.now() <= 0) return "";
  const d = new Date(ts);
  const clock = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
  const now = new Date();
  const sameDay = d.getFullYear() === now.getFullYear()
    && d.getMonth() === now.getMonth()
    && d.getDate() === now.getDate();
  return sameDay ? clock : `${d.getMonth() + 1}/${d.getDate()} ${clock}`;
}

// ── 手动预算（无上游 quota 平台）──

/** 生成一条新手动预算的默认值（uuid id + total/usd）。 */
export function newManualBudget(): ManualBudget {
  const id = (typeof crypto !== "undefined" && crypto.randomUUID)
    ? crypto.randomUUID().replace(/-/g, "")
    : Math.random().toString(36).slice(2) + Date.now().toString(36);
  return { id, kind: "total", unit: "usd", amount: 0, window_hours: null, window_unit: "hour", consumed: 0, window_start_at: null, enabled: true };
}

/** 手动预算剩余展示数据（取剩余比例最低那条；token 单位尽力折算，缺价显 token）。 */
export interface ManualBudgetDisplay {
  hasData: boolean;
  /** 剩余值（usd 单位为 $；token 单位为 token 数）。 */
  remaining: number;
  amount: number;
  unit: ManualBudgetUnit;
  kind: ManualBudgetKind;
  /** 剩余占比 0–1，越低越紧。 */
  ratio: number;
  depleted: boolean;
}

/** 从平台 manual_budgets 选「剩余比例最低」那条用于卡片展示。 */
export function computeManualBudgetDisplay(budgets: ManualBudget[] | undefined): ManualBudgetDisplay | null {
  const enabled = (budgets ?? []).filter(b => b.enabled && b.amount > 0);
  if (enabled.length === 0) return null;
  let tightest: ManualBudget | null = null;
  let minRatio = Infinity;
  for (const b of enabled) {
    const rem = b.amount - b.consumed;
    const ratio = b.amount > 0 ? rem / b.amount : 0;
    if (ratio < minRatio) { minRatio = ratio; tightest = b; }
  }
  if (!tightest) return null;
  const rem = tightest.amount - tightest.consumed;
  return {
    hasData: true,
    remaining: rem,
    amount: tightest.amount,
    unit: tightest.unit,
    kind: tightest.kind,
    ratio: Math.max(0, Math.min(1, minRatio === Infinity ? 0 : minRatio)),
    depleted: rem <= 0,
  };
}
