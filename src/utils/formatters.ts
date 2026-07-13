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
 * 成本数值格式化（不含货币符号），按量级选择精度：
 * - 0（或非正 / NaN）→ "0"
 * - >= 1 → 2 位小数
 * - >= 0.01 → 3 位小数
 * - 0 < n < 0.01（极小值）→ 定点小数展示 2 位有效数字（如 `0.00000045`），**不用科学记数**
 *
 * 关键：极小但**非零**的成本绝不被舍成 "0"（旧版 toFixed(5) 会把 4.5e-7 显示成 "0.00000"）。
 * 合并自 Groups.tsx 行内 `u.total_cost.toFixed(...)` 逻辑。
 * 调用方自行拼接 "$" 前缀（与现有 `$${cost}` 用法一致）。
 */
export function formatCost(n: number): string {
  if (!(n > 0)) return "0";
  if (n >= 1) return n.toFixed(2);
  if (n >= 0.01) return n.toFixed(3);
  // 定点小数（非科学记数）展示 2 位有效数字：0.00000045→"0.00000045"、0.0034→"0.0034"
  // 小数位 = 保 2 位有效数字所需，下限 5、上限 12（防极小值串过长）
  const decimals = Math.min(12, Math.max(5, -Math.floor(Math.log10(n)) + 1));
  return n.toFixed(decimals);
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

/**
 * ISO 时间戳 / 毫秒戳 → 本地化日期时间字符串（如 "2026/06/26 下午2:30:00"）。
 * 收敛 Skills / Platforms 等页面原本各自 `new Date(x).toLocaleString()` 的内联调用。
 * 输入无效（空 / 非日期字符串 / NaN）→ null（调用方按 null 处理「不显示」）。
 *
 * 注：toLocaleString() 不带参数走运行时默认 locale，跨平台/跨 locale 一致性依赖浏览器实现，
 * 桌面应用场景下足够（macOS WKWebView 走系统 locale）。未来如需强制 locale 可加第二参数。
 */
export function formatDateTime(input: string | number | null | undefined): string | null {
  if (input === null || input === undefined || input === "") return null;
  const d = typeof input === "number" ? new Date(input) : new Date(input);
  if (Number.isNaN(d.getTime())) return null;
  return d.toLocaleString();
}

/**
 * ISO 时间戳 / 毫秒戳 → 相对时间简写（如 "3 天前" / "刚刚" / "2 小时前"）。
 * 用于卡片次要信息行紧凑展示（绝对时间走 formatDateTime，相对时间更易扫读）。
 * 输入无效 → null。差值 < 1 秒 → "刚刚"；未来时间（> 当前）→ 也用 "刚刚" 兜底（防倒计时 UI）。
 *
 * 粒度：秒 / 分 / 时 / 天 / 月（30 天）/ 年（365 天），取最大整数单位。
 */
export function formatRelativeTime(input: string | number | null | undefined): string | null {
  if (input === null || input === undefined || input === "") return null;
  const d = typeof input === "number" ? new Date(input) : new Date(input);
  if (Number.isNaN(d.getTime())) return null;
  const diffMs = Date.now() - d.getTime();
  // 未来时间兜底为「刚刚」（避免显示负数或倒计时）。
  const past = Math.max(0, diffMs);
  const sec = Math.floor(past / 1000);
  if (sec < 60) return "刚刚";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} 分钟前`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr} 小时前`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day} 天前`;
  const month = Math.floor(day / 30);
  if (month < 12) return `${month} 个月前`;
  const year = Math.floor(day / 365);
  return `${year} 年前`;
}

/**
 * 数字补零到 2 位：7 → "07"，12 → "12"。
 * 用于日期时间格式化（时/分/秒/月/日）。
 * 合并自 formSections.tsx（两处）、ScheduledBackupSection.tsx、ModelsMatrixSection.tsx 的 pad/pad2 实现。
 */
export function pad(n: number): string {
  return String(n).padStart(2, "0");
}

/**
 * 数值限制：将 value 限制在 [min, max] 区间内。
 * 例：clamp(15, 1, 10) → 10，clamp(-5, 0, 100) → 0，clamp(50, 0, 100) → 50。
 * 合并自 popover.tsx / usageColor.ts / PlatformCard.tsx 各自的 clamp 实现。
 */
export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
