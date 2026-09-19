// ── LTTB 降采样（对应 React 版 src/components/charts/downsample.ts）──
// Largest-Triangle-Three-Buckets：按 x 均分 bucket，每 bucket 选与
// 「上一选中点 + 下一 bucket 平均点」构成三角形面积最大的点，首尾点必留。
// 视觉保真优于均匀抽样（尖峰不丢），O(n)。
import 'dart:math' as math;

/// 折线/面积/柱状降采样阈值：超过才降（minute 粒度最坏 1440 点）。
const int lttbThreshold = 500;

/// LTTB 降采样。points 按 x 升序（调用方保证，代理日志 bucket 天然有序）。
/// xOf / yOf 取坐标（适配任意 data 形状，不必是 (x, y) 对）。
/// - 点数 ≤ threshold → **原列表原样返回**（同一个实例，不复制）
/// - threshold < 3 → 无法分桶，原样返回
List<T> downsampleLttb<T>(
  List<T> points,
  num Function(T) xOf,
  num Function(T) yOf, [
  int threshold = lttbThreshold,
]) {
  final n = points.length;
  if (threshold < 3 || n <= threshold) return points;

  final sampled = <T>[points[0]];
  final every = (n - 2) / (threshold - 2); // 每 bucket 平均摊到的点数
  var a = 0; // 上一个选中点下标

  for (var i = 0; i < threshold - 2; i++) {
    // 下一 bucket 范围 [rangeStart, rangeEnd)，求平均点
    final rangeStart = ((i + 1) * every).floor() + 1;
    final rangeEnd = math.min(((i + 2) * every).floor() + 1, n);
    final avgLen = math.max(rangeEnd - rangeStart, 1);
    var avgX = 0.0;
    var avgY = 0.0;
    for (var j = rangeStart; j < rangeEnd; j++) {
      avgX += xOf(points[j]);
      avgY += yOf(points[j]);
    }
    avgX /= avgLen;
    avgY /= avgLen;

    // 当前 bucket [rangeOffs, rangeTo) 内选三角形面积最大者
    final rangeOffs = (i * every).floor() + 1;
    final rangeTo = math.min(((i + 1) * every).floor() + 1, n - 1);
    final ax = xOf(points[a]);
    final ay = yOf(points[a]);
    var maxArea = -1.0;
    var nextA = rangeOffs;
    for (var j = rangeOffs; j < rangeTo; j++) {
      final area =
          ((ax - avgX) * (yOf(points[j]) - ay) -
                  (ax - xOf(points[j])) * (avgY - ay))
              .abs() /
          2;
      if (area > maxArea) {
        maxArea = area;
        nextA = j;
      }
    }
    sampled.add(points[nextA]);
    a = nextA;
  }

  sampled.add(points[n - 1]);
  return sampled;
}
