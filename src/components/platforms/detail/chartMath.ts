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
export interface CalibrationPoint { t: number; pct: number; name: string; unit: string }

export interface SubscriptionCurve {
  windowStart: number;
  resetAt: number;
  /** 本地记账折算的剩余%，按桶时间升序。 */
  actual: CurvePoint[];
  /** 计划消耗线：windowStart 100% → resetAt 0%。 */
  plan: [CurvePoint, CurvePoint];
  /** 官方配额查询校准点（t = queried_at）。 */
  calibrations: CalibrationPoint[];
  /** 校准点与本地曲线的最大偏差百分点；无校准点 → null。 */
  maxDeviation: number | null;
  /** 最新匹配额度快照的剩余%；没有快照时未知。 */
  snapshotPct: number | null;
}

const HOUR = 3_600_000;
const DAY = 24 * HOUR;

/** 从档位名推断周期毫秒：five_hour→5h，week→7d，month→30d，day→24h，其余→7d 缺省。 */
export function cycleMsForName(name: string): number {
  if (name.includes("five_hour")) return 5 * HOUR;
  if (name.includes("week")) return 7 * DAY;
  if (name.includes("month")) return 30 * DAY;
  if (name.includes("day") || name.includes("daily")) return DAY;
  return 7 * DAY;
}

function hasKnownCycleName(name: string): boolean {
  return name.includes("five_hour") || name.includes("week") || name.includes("month") ||
    name.includes("day") || name.includes("daily");
}

interface WindowAnchor {
  source: "quota" | "estimate";
  name: string;
  unit: string;
  windowStart: number;
  resetAt: number;
  cycle: number;
  limit: number | null;
  quotaTier?: CurveTierLike;
}

const unitOf = (unit?: string | null) => unit?.trim() || "tokens";

/** 从 quota tiers（resets_at 反推）或 est tiers（window_start + 周期）取周期最长的窗口锚点。 */
function pickAnchor(quotaTiers: CurveTierLike[], estTiers: CurveTierLike[]): WindowAnchor | null {
  const candidates: WindowAnchor[] = [];
  for (const t of quotaTiers) {
    if (!t.resets_at) continue;
    const resetAt = Date.parse(t.resets_at);
    if (!Number.isFinite(resetAt)) continue;
    const cycle = cycleMsForName(t.name);
    candidates.push({
      source: "quota", name: t.name, unit: unitOf(t.unit),
      windowStart: resetAt - cycle, resetAt, cycle, limit: t.limit ?? null, quotaTier: t,
    });
  }
  for (const t of estTiers) {
    if (!t.window_start || t.window_start <= 0) continue;
    const cycle = cycleMsForName(t.name);
    candidates.push({
      source: "estimate", name: t.name, unit: unitOf(t.unit),
      windowStart: t.window_start, resetAt: t.window_start + cycle, cycle, limit: t.limit ?? null,
    });
  }
  if (candidates.length === 0) return null;
  return candidates.reduce((a, b) => (b.cycle > a.cycle ? b : a));
}

const tokensOf = (p: SeriesPointLike) => p.input_tokens + p.output_tokens + p.cache_tokens;

/**
 * 订阅消耗曲线构建。窗口起点/重置点由档位（quota tiers resets_at 反推或 est window_start）
 * 取周期最长的一档；只有 token 单位、完整覆盖且与本地日界对齐时才从本地用量换算历史余额。
 * 其它情况只显示同档同单位的官方快照，不投影额度或伪造历史。
 */
export function buildSubscriptionCurve(input: {
  series: SeriesPointLike[];
  seriesWindowStart?: number | null;
  seriesQueriedAt?: number | null;
  quotaTiers: CurveTierLike[];
  quotaQueriedAt: number | null;
  estTiers: CurveTierLike[];
  nowMs: number;
}): SubscriptionCurve | null {
  const anchor = pickAnchor(input.quotaTiers, input.estTiers);
  if (!anchor) return null;
  const { windowStart, resetAt } = anchor;
  const queryAt = input.seriesQueriedAt;
  const total = anchor.limit != null && anchor.limit > 0 ? anchor.limit : null;
  const completeDailyWindow =
    anchor.unit === "tokens" && hasKnownCycleName(anchor.name) && total != null &&
    anchor.cycle <= 7 * DAY && anchor.cycle % DAY === 0 &&
    isLocalDayStart(windowStart) && isLocalDayStart(resetAt) &&
    input.seriesWindowStart != null && windowStart >= input.seriesWindowStart &&
    queryAt != null && queryAt >= windowStart && queryAt < resetAt && queryAt <= input.nowMs &&
    input.series.every(p => p.time.length <= 10);

  // The API aggregates 7d into local-day buckets. Do not infer a partial day or a longer cycle.
  const buckets = completeDailyWindow
    ? input.series
        .map(point => ({ point, t: bucketTimeMs(point.time) }))
        .filter(b => Number.isFinite(b.t) && b.t >= windowStart && b.t < resetAt &&
          queryAt != null && nextLocalDayStart(b.t) <= queryAt)
        .sort((a, b) => a.t - b.t)
    : [];
  let cum = 0;
  const actual = completeDailyWindow
    ? buckets.map(b => {
        cum += tokensOf(b.point);
        return {
          t: nextLocalDayStart(b.t),
          pct: clampPct(100 * (1 - cum / total!)),
        };
      })
    : [];

  // A snapshot calibrates only its exact tier and reset window; never match by name alone.
  const tier = anchor.quotaTier ?? input.quotaTiers.find(t => {
    const tierResetAt = t.resets_at ? Date.parse(t.resets_at) : NaN;
    const tierCycle = cycleMsForName(t.name);
    return t.name === anchor.name && unitOf(t.unit) === anchor.unit &&
      tierResetAt === resetAt && tierResetAt - tierCycle === windowStart;
  });
  const snapshotAt = input.quotaQueriedAt;
  const snapshotPct = tier?.utilization != null && snapshotAt != null &&
    snapshotAt >= windowStart && snapshotAt < resetAt && snapshotAt <= input.nowMs
    ? clampPct(100 - tier.utilization)
    : null;
  const calibrations: CalibrationPoint[] = tier && snapshotAt != null && snapshotPct != null
    ? [{ t: snapshotAt, pct: snapshotPct, name: tier.name, unit: unitOf(tier.unit) }]
    : [];
  const local = actual.length > 1 && snapshotAt != null ? interpPct(actual, snapshotAt) : null;
  const maxDeviation = local != null && snapshotPct != null ? Math.abs(local - snapshotPct) : null;

  return {
    windowStart, resetAt, actual, snapshotPct,
    plan: [{ t: windowStart, pct: 100 }, { t: resetAt, pct: 0 }],
    calibrations, maxDeviation,
  };
}

const clampPct = (p: number) => Math.max(0, Math.min(100, p));

function isLocalDayStart(ms: number): boolean {
  const d = new Date(ms);
  return d.getHours() === 0 && d.getMinutes() === 0 && d.getSeconds() === 0 && d.getMilliseconds() === 0;
}

function nextLocalDayStart(ms: number): number {
  const d = new Date(ms);
  d.setDate(d.getDate() + 1);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** 桶键 → unix ms：日键 "YYYY-MM-DD"（当地 00:00）或小时键 "YYYY-MM-DD HH:00:00"。 */
function bucketTimeMs(key: string): number {
  const day = Date.parse(`${key.slice(0, 10)}T00:00:00`);
  if (!Number.isFinite(day)) return NaN;
  if (key.length <= 10) return day;
  const hour = Number(key.slice(11, 13)) || 0;
  return day + hour * HOUR;
}

/** actual 点序列在 t 处线性插值（t 越界返回端点外不插）。 */
function interpPct(actual: CurvePoint[], t: number): number | null {
  if (actual.length < 2 || t < actual[0].t || t > actual[actual.length - 1].t) return null;
  for (let i = 0; i < actual.length - 1; i++) {
    const a = actual[i], b = actual[i + 1];
    if (t >= a.t && t <= b.t && b.t > a.t) {
      return a.pct + ((t - a.t) / (b.t - a.t)) * (b.pct - a.pct);
    }
  }
  return actual[actual.length - 1].pct;
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
