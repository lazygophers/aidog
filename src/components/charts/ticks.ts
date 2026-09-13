// ── 轴刻度公共层（spec §B2：nice-ticks + 时间轴标签，组件不各自实现）──
// 数值补零复用 utils/formatters.ts 的 pad，禁本文件重复定义。
import { pad } from "@/utils/formatters";

/** 1/2/5×10^n 好看步长：norm = rawStep / 10^floor(log10) 落档取整倍率。 */
function niceStep(norm: number, mag: number): number {
  const mult = norm <= 1 ? 1 : norm <= 2 ? 2 : norm <= 5 ? 5 : 10;
  return mult * mag;
}

/**
 * nice-ticks：给数据域 [min, max] 生成含两端的「好看」均匀刻度（1/2/5 步长）。
 * 调用方直接喂给 Recharts CartesianAxis 的 ticks。
 * - min === max 或 max < min → 单刻度 [min]（退化域，调用方自行加 padding）
 * - 非有限值（NaN/Infinity，脏数据漏到轴上）→ []（轴回落 recharts 自动刻度）
 */
export function niceTicks(min: number, max: number, tickCount = 5): number[] {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return [];
  if (!(max > min)) return [min];
  const rawStep = (max - min) / Math.max(2, tickCount - 1);
  const mag = Math.pow(10, Math.floor(Math.log10(rawStep)));
  const step = niceStep(rawStep / mag, mag);
  const ticks: number[] = [];
  // + step/2 容差吃掉浮点累加误差，保证闭区间右端点入列；toFixed(10) 去 0.30000000000000004 噪声。
  for (let v = Math.floor(min / step) * step; v <= max + step / 2; v += step) {
    ticks.push(Number(v.toPrecision(12)));
  }
  return ticks;
}

const HOUR_MS = 3_600_000;

/**
 * 时间轴刻度标签：按横轴总跨度（spanMs = max - min）选粒度，全部本地时区。
 * - ≤ 48h → "HH:MM"（含跨日的日内窗）
 * - ≤ 60 天 → "MM-DD"
 * - 更长 → "YYYY-MM"
 * value 无效（NaN 等）→ ""（recharts 对空串不渲染该 tick）。
 */
export function formatTimeTick(value: number, spanMs: number): string {
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return "";
  if (spanMs <= 48 * HOUR_MS) return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  if (spanMs <= 60 * 24 * HOUR_MS) return `${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}`;
}
