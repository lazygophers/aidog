// ── Stats 页前端二次聚合（对应 React 版 src/pages/Stats.tsx:108-202 的四个导出纯函数）──
// 后端聚合在 Rust（aidog_stats），这里只做展示层的再聚合，全部是纯函数、无 I/O。
import 'dart:math' as math;

import '../charts/ticks.dart';
import 'models.dart';

// 桶串解析属图表公共层，这里转出一手（对应 React 版 Stats.tsx 顶部的 re-export）。
export '../charts/ticks.dart' show bucketMs;

int _reqSum(List<StatsBucket> bs) => bs.fold(0, (s, b) => s + b.totalRequests);

/// 稳定排序：JS `Array.prototype.sort` 自 ES2019 起保证稳定，Dart `List.sort` **不**保证。
/// 并列项要保持首现序（维度行序、同毫秒快照序），故按下标兜底比较。
List<T> _stableSorted<T>(Iterable<T> items, int Function(T a, T b) compare) {
  final indexed = [
    for (var (i, v) in items.indexed) (i, v),
  ];
  indexed.sort((a, b) {
    final c = compare(a.$2, b.$2);
    return c != 0 ? c : a.$1.compareTo(b.$1);
  });
  return [for (final e in indexed) e.$2];
}

/// 趋势图数据。[config] = 列键 → 图例标签（React 版是 `{label}` 包一层，Dart 直接存标签串）。
/// [rows] 是宽表行：`x` = 桶时间戳 ms，其余键是列值；**缺失的格子不写键**（断线由渲染层处理）。
class TrendChartData {
  const TrendChartData({
    required this.config,
    required this.rows,
    required this.multi,
  });

  final Map<String, String> config;
  final List<Map<String, num>> rows;
  final bool multi;
}

/// 时间序列 tab 数据：
/// - 多序列（series_by）→ 合并宽表，键 `s0..` 按总量降序（s0 = 最大维度值 = 主琥珀线）
/// - series 空 / 单序列 → buckets 总量单序列（键 `v`）
TrendChartData buildTrendChartData(
  List<StatsBucket> buckets,
  List<StatsSeries> series,
  String singleLabel,
) {
  if (series.length > 1) {
    final ordered = _stableSorted(
      series,
      (a, b) => _reqSum(b.buckets) - _reqSum(a.buckets),
    );
    final rowMap = <String, Map<String, num>>{};
    for (var i = 0; i < ordered.length; i++) {
      for (final b in ordered[i].buckets) {
        final row = rowMap.putIfAbsent(
          b.timeBucket,
          () => <String, num>{'x': bucketMs(b.timeBucket)},
        );
        row['s$i'] = b.totalRequests;
      }
    }
    final rows = _stableSorted(
      rowMap.values,
      (a, b) => a['x']!.compareTo(b['x']!),
    );
    return TrendChartData(
      config: {
        for (var i = 0; i < ordered.length; i++) 's$i': ordered[i].name,
      },
      rows: rows,
      multi: true,
    );
  }
  return TrendChartData(
    config: {'v': singleLabel},
    rows: [
      for (final b in buckets)
        <String, num>{'x': bucketMs(b.timeBucket), 'v': b.totalRequests},
    ],
    multi: false,
  );
}

/// 一个热力格：[day] 0 = 周日（对齐 JS `Date.getDay()`），[hour] 0-23。
typedef HeatCell = ({int day, int hour, int value});

/// 密度 tab：hourly/minute 桶 → (星期, 小时) 请求量聚合。
/// daily 桶无小时信息不入格（调用方喂 hourly 粒度数据，诚实不摊假）。
List<HeatCell> buildHeatCells(List<StatsBucket> buckets) {
  final m = <int, int>{};
  for (final b in buckets) {
    if (!b.timeBucket.contains(' ')) continue;
    final d = DateTime.tryParse(b.timeBucket.replaceFirst(' ', 'T'));
    if (d == null) continue;
    // Dart weekday 是 1=周一..7=周日；% 7 得到 JS getDay() 的 0=周日
    final k = (d.weekday % 7) * 24 + d.hour;
    m[k] = (m[k] ?? 0) + b.totalRequests;
  }
  return [
    for (final e in m.entries)
      (day: e.key ~/ 24, hour: e.key % 24, value: e.value),
  ];
}

/// 维度 × 日格子：[day] 是该本地日 00:00 的 ms 时间戳。
typedef DimensionDayCell = ({String name, int day, int value});

/// 维度×日格子（占比 tab 维度热力）：series 逐桶按本地日聚合（hourly/minute 桶摊入当日，
/// daily 桶本身就是日）。行取总量降序 topN（全画则行图过高，热力看头部维度）。
List<DimensionDayCell> buildDimensionDayCells(
  List<StatsSeries> series, [
  int topN = 8,
]) {
  final ordered = _stableSorted(
    series,
    (a, b) => _reqSum(b.buckets) - _reqSum(a.buckets),
  ).take(topN);
  final cells = <DimensionDayCell>[];
  for (final s in ordered) {
    final perDay = <int, int>{};
    for (final b in s.buckets) {
      final ms = bucketMs(b.timeBucket);
      if (ms.isNaN) continue;
      final d = DateTime.fromMillisecondsSinceEpoch(ms.toInt());
      final day = DateTime(d.year, d.month, d.day).millisecondsSinceEpoch;
      perDay[day] = (perDay[day] ?? 0) + b.totalRequests;
    }
    for (final e in perDay.entries) {
      cells.add((name: s.name, day: e.key, value: e.value));
    }
  }
  return cells;
}

/// 仪表盘趋势点：[at] 是 **Unix 秒**，[fraction] = 该刻余额 / 窗口峰值。
typedef GaugeTrendPoint = ({int at, double fraction});

/// 单平台配额仪表盘数据。
class QuotaGauge {
  const QuotaGauge({
    required this.platformId,
    required this.current,
    required this.peak,
    required this.trend,
  });

  /// 该平台最新快照余额。
  final double current;

  /// 窗口内峰值余额（快照无总量字段，仪表盘 fraction 语义 = 当前 / 峰值）。
  final double peak;
  final int platformId;
  final List<GaugeTrendPoint> trend;
}

/// 配额 tab：快照序列 → 每平台仪表盘数据。
/// 按当前余额降序（主平台排前）；peak ≤ 0 的组剔除（喂仪表盘也是空态，不如不出卡）。
List<QuotaGauge> buildQuotaGauges(List<QuotaSnapshot> snaps) {
  final byPlatform = <int, List<QuotaSnapshot>>{};
  for (final s in snaps) {
    byPlatform.putIfAbsent(s.platformId, () => <QuotaSnapshot>[]).add(s);
  }
  final gauges = <QuotaGauge>[];
  for (final entry in byPlatform.entries) {
    // 后端按 created_at 升序返回；仍显式排序一次，纯函数不依赖调用侧约定
    final arr = _stableSorted(
      entry.value,
      (a, b) => a.createdAt.compareTo(b.createdAt),
    );
    final peak = arr.map((s) => s.estBalanceRemaining).reduce(math.max);
    if (!(peak > 0)) continue;
    gauges.add(
      QuotaGauge(
        platformId: entry.key,
        current: arr.last.estBalanceRemaining,
        peak: peak,
        trend: [
          for (final s in arr)
            (
              at: (s.createdAt / 1000).floor(),
              fraction: s.estBalanceRemaining / peak,
            ),
        ],
      ),
    );
  }
  return _stableSorted(gauges, (a, b) => b.current.compareTo(a.current));
}
