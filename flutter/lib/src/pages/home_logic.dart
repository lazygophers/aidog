/// 首页的纯逻辑（对应 React 版 `src/pages/Home.tsx` 里不碰 DOM 的那部分）。
/// 无 I/O、无 widget —— 测试不用起内核也不用建树。
library;

import 'dart:math' as math;

import '../../stats/models.dart';
import 'models.dart';

/// 首页 KPI / 平台 Top 的条数上限（React 版 `TOP_PLATFORMS = 4`）。

/// 代理端口缺省值（React 版 `DEFAULT_PORT = 7890`）。
const int kDefaultPort = 7890;

/// 数值序列 → 归一化 `[x, y]` 点集（KPI sparkline 用）。
///
/// 逐字对应 React 版 `Home.tsx::normPoints`：y 按 min-max 缩放进 `[pad, h-pad]`
/// （平坦序列落中线，不除零）；x 均分（单点居中）。
List<(double, double)> normPoints(
  List<double> values,
  double w,
  double h, [
  double pad = 2,
]) {
  if (values.isEmpty) return const [];
  final min = values.reduce(math.min);
  final max = values.reduce(math.max);
  final span = max - min;
  return [
    for (var i = 0; i < values.length; i++)
      (
        values.length > 1 ? (i / (values.length - 1)) * w : w / 2,
        span > 0 ? pad + (1 - (values[i] - min) / span) * (h - 2 * pad) : h / 2,
      ),
  ];
}

/// 今日是否有数据：requests / cost / tokens 任一 > 0（React 版 `hasTodayData`）。
bool hasTodayData(TodayStats? today) =>
    today != null &&
    (today.totalRequests > 0 || today.cost > 0 || today.tokens > 0);

/// 总余额 = 关联平台 `est_balance_remaining` 求和（平台级属性，无 per-group 概念）。
double totalBalanceOf(List<PlatformSummary> platforms) =>
    platforms.fold(0, (acc, p) => acc + p.estBalanceRemaining);

/// 24 小时滚动窗口 `[now-24h, now]`（React 版 `load()` 里的 `windowStart`）。
({int start, int end}) last24h(DateTime now) {
  final end = now.millisecondsSinceEpoch;
  return (start: end - 24 * 3600 * 1000, end: end);
}

/// 趋势桶 → 四条 KPI sparkline 序列（hourly 桶）。
/// `tokens` = input + output + cache，与 React 版同。
({
  List<double> requests,
  List<double> cost,
  List<double> tokens,
  List<double> cache,
})
trendSeriesOf(List<StatsBucket> buckets) => (
  requests: [for (final b in buckets) b.totalRequests.toDouble()],
  cost: [for (final b in buckets) b.totalCost],
  tokens: [
    for (final b in buckets)
      (b.inputTokens + b.outputTokens + b.cacheTokens).toDouble(),
  ],
  cache: [for (final b in buckets) b.cacheTokens.toDouble()],
);

/// x 轴整点小时标注：每 6 桶标一个，标的是 `time_bucket` 的 `HH` 两位。
/// 桶串短于 13 字符（如 daily 桶 `YYYY-MM-DD`）时无小时信息，返回空串。
String hourTickOf(StatsBucket b) =>
    b.timeBucket.length >= 13 ? b.timeBucket.substring(11, 13) : '';
