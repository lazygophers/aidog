// ── 统一数值 / 成本 / 百分比 格式化工具 ──
// 收敛 Groups / Stats / Logs 等页面原本各自重复定义的 formatter，保证显示一致、修复传播。
// 仅纯函数，无副作用，无 React 依赖。

/**
 * 大数字缩写：1_200_000 → "1.2M"，3_500 → "3.5K"，整数原样，带小数则保留 1 位。
 *
 * 合并自 Stats.tsx `formatNumber`（最完善版，含 `n % 1` 小数判断）。
 * Groups.tsx `fmtTk`（非整数 < 1000 时原样输出 `${n}`）行为统一为此版（更一致）。
 */
export function formatNumber(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(1) + "K";
  return n.toFixed(n % 1 === 0 ? 0 : 1);
}

/**
 * 成本数值格式化（不含货币符号），按量级选择小数位：
 * - 0 → "0"
 * - >= 1 → 2 位
 * - >= 0.01 → 3 位
 * - 其余（极小值）→ 5 位
 *
 * 合并自 Groups.tsx 行内 `u.total_cost.toFixed(...)` 逻辑。
 * 调用方自行拼接 "$" 前缀（与现有 `$${cost}` 用法一致）。
 */
export function formatCost(n: number): string {
  if (!(n > 0)) return "0";
  const digits = n >= 1 ? 2 : n >= 0.01 ? 3 : 5;
  return n.toFixed(digits);
}

/**
 * 带货币符号的成本：`$` + formatCost(n)。便于 chip / 表格统一调用。
 */
export function formatCostUsd(n: number): string {
  return "$" + formatCost(n);
}

/**
 * 百分比格式化：值已是 0–100 的百分数，保留 `digits` 位（默认 1）。
 * 例：formatPercent(98.7) → "98.7%"，formatPercent(98.7, 0) → "99%"。
 */
export function formatPercent(n: number, digits = 1): string {
  return n.toFixed(digits) + "%";
}

/**
 * 由请求计数推导成功率（百分数，0–100）。total 为 0 时返回 0。
 */
export function successRate(successCount: number, totalRequests: number): number {
  if (totalRequests <= 0) return 0;
  return (successCount / totalRequests) * 100;
}

/**
 * token 求和：把多个 token 维度（输入 / 输出 / 缓存等）相加，忽略 undefined / NaN。
 */
export function sumTokens(...parts: Array<number | undefined | null>): number {
  return parts.reduce<number>((acc, p) => acc + (typeof p === "number" && !Number.isNaN(p) ? p : 0), 0);
}
