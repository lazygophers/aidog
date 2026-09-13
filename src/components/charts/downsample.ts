// ── LTTB 降采样（spec §F1：前端 >500 点时降采样，阈值常量放公共层）──
// Largest-Triangle-Three-Buckets：按 x 均分 bucket，每 bucket 选与
// 「上一选中点 + 下一 bucket 平均点」构成三角形面积最大的点，首尾点必留。
// 视觉保真优于均匀抽样（尖峰不丢），O(n)。

/** 折线/面积/柱状降采样阈值：超过才降（spec §F1，minute 粒度最坏 1440 点）。 */
export const LTTB_THRESHOLD = 500;

/**
 * LTTB 降采样。points 按 x 升序（调用方保证，代理日志 bucket 天然有序）。
 * xOf / yOf 取坐标（适配任意 data 形状，不必是 [x,y] 对）。
 * - 点数 ≤ threshold → 原数组原样返回（不复制）
 * - threshold < 3 → 无法分桶，原样返回
 */
export function downsampleLTTB<T>(
  points: readonly T[],
  xOf: (p: T) => number,
  yOf: (p: T) => number,
  threshold: number = LTTB_THRESHOLD,
): T[] {
  const n = points.length;
  if (threshold < 3 || n <= threshold) return points as T[];

  const sampled: T[] = [points[0]];
  const every = (n - 2) / (threshold - 2); // 每 bucket 平均摊到的点数
  let a = 0; // 上一个选中点下标

  for (let i = 0; i < threshold - 2; i++) {
    // 下一 bucket 范围 [rangeStart, rangeEnd)，求平均点
    const rangeStart = Math.floor((i + 1) * every) + 1;
    const rangeEnd = Math.min(Math.floor((i + 2) * every) + 1, n);
    const avgLen = Math.max(rangeEnd - rangeStart, 1);
    let avgX = 0;
    let avgY = 0;
    for (let j = rangeStart; j < rangeEnd; j++) {
      avgX += xOf(points[j]);
      avgY += yOf(points[j]);
    }
    avgX /= avgLen;
    avgY /= avgLen;

    // 当前 bucket [rangeOffs, rangeTo) 内选三角形面积最大者
    const rangeOffs = Math.floor(i * every) + 1;
    const rangeTo = Math.min(Math.floor((i + 1) * every) + 1, n - 1);
    const ax = xOf(points[a]);
    const ay = yOf(points[a]);
    let maxArea = -1;
    let nextA = rangeOffs;
    for (let j = rangeOffs; j < rangeTo; j++) {
      const area =
        Math.abs((ax - avgX) * (yOf(points[j]) - ay) - (ax - xOf(points[j])) * (avgY - ay)) / 2;
      if (area > maxArea) {
        maxArea = area;
        nextA = j;
      }
    }
    sampled.push(points[nextA]);
    a = nextA;
  }

  sampled.push(points[n - 1]);
  return sampled;
}
