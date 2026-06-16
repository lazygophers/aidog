// 共享图表工具：SVG 平滑曲线路径生成（Home / Stats 趋势图复用）。

export interface ChartPoint {
  x: number;
  y: number;
}

/**
 * Catmull-Rom → 三次贝塞尔平滑曲线（张力 0.5 经典），生成 SVG path `d`。
 * 控制点 y clamp 到 [clampMin, clampMax] 防过冲超出绘图区。
 * 点数 < 3 退化为直线（单点/双点不崩）。
 */
export function smoothPath(
  points: ChartPoint[],
  clampMin: number,
  clampMax: number,
): string {
  if (points.length === 0) return "";
  if (points.length < 3) {
    return points
      .map((p, i) => `${i === 0 ? "M" : "L"} ${p.x.toFixed(1)},${p.y.toFixed(1)}`)
      .join(" ");
  }
  const clampY = (v: number) => Math.min(clampMax, Math.max(clampMin, v));
  let d = `M ${points[0].x.toFixed(1)},${points[0].y.toFixed(1)}`;
  for (let i = 0; i < points.length - 1; i++) {
    const p0 = points[i === 0 ? 0 : i - 1];
    const p1 = points[i];
    const p2 = points[i + 1];
    const p3 = points[i + 2 < points.length ? i + 2 : points.length - 1];
    // 张力 0.5：控制点 = 端点 ± (相邻点切线)/6
    const c1x = p1.x + (p2.x - p0.x) / 6;
    const c1y = clampY(p1.y + (p2.y - p0.y) / 6);
    const c2x = p2.x - (p3.x - p1.x) / 6;
    const c2y = clampY(p2.y - (p3.y - p1.y) / 6);
    d += ` C ${c1x.toFixed(1)},${c1y.toFixed(1)} ${c2x.toFixed(1)},${c2y.toFixed(1)} ${p2.x.toFixed(1)},${p2.y.toFixed(1)}`;
  }
  return d;
}
