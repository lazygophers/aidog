// 图表数据映射纯函数（T3 数据板块）：订阅曲线拟合 / 按量日柱 / 失败分类 / 模型分布。
// 只做数据→图形坐标的映射，不碰 React；订阅「实际剩余」用单调三次样条（Fritsch–Carlson）
// 平滑拟合，禁原始阶梯线（票 T3）。

export interface Pt { x: number; y: number }

/** Fritsch–Carlson 单调三次样条 → SVG cubic bezier path。点少于 2 个退化为 M/直线。 */
export function monotonePath(points: Pt[]): string {
  const n = points.length;
  if (n === 0) return "";
  if (n === 1) return `M ${points[0].x} ${points[0].y}`;
  // 差分与斜率
  const dx: number[] = [], dy: number[] = [], slope: number[] = [];
  for (let i = 0; i < n - 1; i++) {
    dx.push(points[i + 1].x - points[i].x);
    dy.push(points[i + 1].y - points[i].y);
    slope.push(dy[i] / (dx[i] || 1));
  }
  // 切线初值 = 相邻斜率均值，端点取单侧
  const m: number[] = [slope[0]];
  for (let i = 1; i < n - 1; i++) {
    m.push(slope[i - 1] === 0 || slope[i] === 0 ? 0 : (slope[i - 1] + slope[i]) / 2);
  }
  m.push(slope[n - 2]);
  // Fritsch–Carlson 限幅：防止过冲
  for (let i = 0; i < n - 1; i++) {
    if (slope[i] === 0) { m[i] = 0; m[i + 1] = 0; continue; }
    const a = m[i] / slope[i], b = m[i + 1] / slope[i];
    const s = a * a + b * b;
    if (s > 9) {
      const tau = 3 / Math.sqrt(s);
      m[i] = tau * a * slope[i];
      m[i + 1] = tau * b * slope[i];
    }
  }
  // 转成 cubic bezier：控制点 = 端点 + 切线×dx/3
  let d = `M ${points[0].x} ${points[0].y}`;
  for (let i = 0; i < n - 1; i++) {
    const c1x = points[i].x + dx[i] / 3, c1y = points[i].y + (m[i] * dx[i]) / 3;
    const c2x = points[i + 1].x - dx[i] / 3, c2y = points[i + 1].y - (m[i + 1] * dx[i]) / 3;
    d += ` C ${c1x} ${c1y} ${c2x} ${c2y} ${points[i + 1].x} ${points[i + 1].y}`;
  }
  return d;
}

export interface CurveTierLike {
  name: string;
  unit?: string | null;
  utilization?: number;      // quota tiers（0-100）
  est_utilization?: number;  // est_coding_plan tiers（0-100）
  limit?: number | null;     // 绝对上限（token / 次数）
  resets_at?: string | null; // quota tiers ISO
  window_start?: number;     // est tiers unix ms
}

export interface SeriesPointLike {
  time: string;
  requests: number;
  success: number;
  input_tokens: number;
  output_tokens: number;
  cache_tokens: number;
  est_cost: number;
  duration_ms: number;
}

export interface CurvePoint { t: number; pct: number }

const HOUR = 3_600_000;
const DAY = 24 * HOUR;

/** 从已知档位名推断周期毫秒；未知名称不推断周期。 */
export function cycleMsForName(name: string): number | null {
  if (name.includes("five_hour")) return 5 * HOUR;
  if (name.includes("week")) return 7 * DAY;
  if (name.includes("month")) return 30 * DAY;
  if (name.includes("day") || name.includes("daily")) return DAY;
  return null;
}

const clampPct = (p: number) => Math.max(0, Math.min(100, p));

/** 计费单位归一：空/缺省按 tokens。 */
export const unitOf = (unit?: string | null) => unit?.trim() || "tokens";

/** 一档的轨迹点（quota 查询快照换算的剩余%，queried_at 升序）。 */
export interface TierSnapshotPoint { t: number; pct: number }

export interface TierForecast {
  /** true = 按当前速率能撑到重置；false = 重置前耗尽，exhaustAt 为预计耗尽时刻。 */
  lasts: boolean;
  exhaustAt: number | null;
}

export interface TierCurve {
  windowStart: number;
  resetAt: number;
  /** 真实记录的轨迹点（窗口内，升序）；不伪造历史点。 */
  actual: TierSnapshotPoint[];
  /** 最新真实点的剩余%；无历史时取 fallback（est 估算），不进轨迹。 */
  snapshotPct: number | null;
  /** 速率投影；轨迹点 <2 或窗口已过 → null（不推算）。 */
  forecast: TierForecast | null;
}

/**
 * 单档曲线构建（分档改版）：输入某档的真实快照时间序列，输出轨迹点列 + 速率投影。
 * 速率 = 首末轨迹点斜率；投影从最新点延到重置点，重置前会归零则判「提前耗尽」。
 * 只画真实记录；无历史时仅回显 fallback 快照值，不编造轨迹。
 */
export function buildTierCurve(input: {
  windowStart: number | null;
  resetAt: number | null;
  points: TierSnapshotPoint[];
  /** 无历史时的当前快照%（est 估算），不进轨迹。 */
  fallbackPct?: number | null;
  nowMs: number;
}): TierCurve | null {
  const { windowStart, resetAt } = input;
  if (windowStart == null || resetAt == null || resetAt <= windowStart) {
    return input.fallbackPct != null
      ? { windowStart: 0, resetAt: 0, actual: [], snapshotPct: clampPct(input.fallbackPct), forecast: null }
      : null;
  }
  const actual = input.points
    .filter(p => Number.isFinite(p.t) && Number.isFinite(p.pct) &&
      p.t >= windowStart && p.t <= Math.min(input.nowMs, resetAt))
    .sort((a, b) => a.t - b.t)
    .map(p => ({ t: p.t, pct: clampPct(p.pct) }));
  const snapshotPct = actual.length > 0 ? actual[actual.length - 1].pct : (input.fallbackPct != null ? clampPct(input.fallbackPct) : null);

  let forecast: TierForecast | null = null;
  if (actual.length >= 2 && resetAt > input.nowMs) {
    const first = actual[0], last = actual[actual.length - 1];
    if (last.t > first.t) {
      const rate = (last.pct - first.pct) / (last.t - first.t); // %/ms
      if (rate >= 0) {
        forecast = { lasts: true, exhaustAt: null };
      } else {
        const pctAtReset = last.pct + rate * (resetAt - last.t);
        forecast = pctAtReset > 0
          ? { lasts: true, exhaustAt: null }
          : { lasts: false, exhaustAt: Math.round(last.t + last.pct / -rate) };
      }
    }
  }
  return { windowStart, resetAt, actual, snapshotPct, forecast };
}

export interface DailyBar { day: string; cost: number; requests: number }

/** 7 天序列 → 每日柱线图数据（day = 序列桶键原样，cost/requests 透传）。 */
export function buildDailyBars(series: SeriesPointLike[]): DailyBar[] {
  return series.map(p => ({ day: p.time, cost: p.est_cost, requests: p.requests }));
}

export interface FailRowLike {
  status_code: number;
  model: string;
  created_at: number;
}

export type FailClass = "service" | "model" | "rate" | "auth";

/** 失败分类：429→rate；401/403/402→auth；400/404/422/451→model；其余→service。 */
export function classifyFailure(row: FailRowLike): FailClass {
  const s = row.status_code;
  if (s === 429) return "rate";
  if (s === 401 || s === 403 || s === 402) return "auth";
  if (s === 400 || s === 404 || s === 422 || s === 451) return "model";
  return "service";
}

export interface FailureStats {
  counts: Record<FailClass, number>;
  /** 最近 3 条（created_at 降序）。 */
  recent: Array<FailRowLike & { cls: FailClass }>;
}

export function classifyFailures(rows: FailRowLike[]): FailureStats {
  const counts: Record<FailClass, number> = { service: 0, model: 0, rate: 0, auth: 0 };
  for (const r of rows) counts[classifyFailure(r)]++;
  const recent = [...rows]
    .sort((a, b) => b.created_at - a.created_at)
    .slice(0, 3)
    .map(r => ({ ...r, cls: classifyFailure(r) }));
  return { counts, recent };
}

export interface ModelDistRowLike { model: string; requests: number }

/** 模型分布：请求数降序取前 max 条，pct = 占总数百分比（1 位小数）。 */
export function topModels(rows: ModelDistRowLike[], max = 8): Array<ModelDistRowLike & { pct: number }> {
  const total = rows.reduce((s, r) => s + r.requests, 0);
  return [...rows]
    .sort((a, b) => b.requests - a.requests)
    .slice(0, max)
    .map(r => ({ ...r, pct: total > 0 ? Math.round((r.requests / total) * 1000) / 10 : 0 }));
}
