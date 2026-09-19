// 逐条翻自 React 版 src/pages/Stats.test.ts（数据与期望值一字不改；JS 月份索引 8 = 9 月）。
// 断言一律用 DateTime(y, m, d, h, min) 同基准构造，不依赖跑测试机器的时区。
import 'package:aidog_flutter/stats/aggregation.dart';
import 'package:aidog_flutter/stats/models.dart';
import 'package:flutter_test/flutter_test.dart';

StatsBucket bucket(String tb, int requests) =>
    StatsBucket(timeBucket: tb, totalRequests: requests, successCount: requests);

StatsSeries series(String name, List<StatsBucket> buckets) =>
    StatsSeries(name: name, buckets: buckets);

void main() {
  group('bucketMs', () {
    test('datetime 串按本地时区解析（空格补 T）', () {
      expect(
        bucketMs('2026-09-13 14:30'),
        DateTime(2026, 9, 13, 14, 30).millisecondsSinceEpoch,
      );
    });
    test('纯日期串补 T00:00:00 本地午夜（不被当 UTC）', () {
      expect(bucketMs('2026-09-13'), DateTime(2026, 9, 13).millisecondsSinceEpoch);
    });
  });

  group('buildTrendChartData', () {
    test('series 空/单序列 → buckets 总量单序列（v）', () {
      final buckets = [
        bucket('2026-09-13 10:00:00', 5),
        bucket('2026-09-13 11:00:00', 7),
      ];
      for (final s in [<StatsSeries>[], [series('only', buckets)]]) {
        final r = buildTrendChartData(buckets, s, 'requests');
        expect(r.multi, false);
        expect(r.config, {'v': 'requests'});
        expect(r.rows, [
          {'x': DateTime(2026, 9, 13, 10).millisecondsSinceEpoch, 'v': 5},
          {'x': DateTime(2026, 9, 13, 11).millisecondsSinceEpoch, 'v': 7},
        ]);
      }
    });

    test('多序列 → 宽表合并 + 按总量降序（s0 = 最大维度，主琥珀线）', () {
      // 总量 3
      final a = series('A', [
        bucket('2026-09-13 10:00:00', 1),
        bucket('2026-09-13 11:00:00', 2),
      ]);
      // 总量 30
      final b = series('B', [
        bucket('2026-09-13 10:00:00', 10),
        bucket('2026-09-13 12:00:00', 20),
      ]);
      final r = buildTrendChartData([], [a, b], 'requests');
      expect(r.multi, true);
      expect(r.config, {'s0': 'B', 's1': 'A'});
      final t10 = DateTime(2026, 9, 13, 10).millisecondsSinceEpoch;
      final t11 = DateTime(2026, 9, 13, 11).millisecondsSinceEpoch;
      final t12 = DateTime(2026, 9, 13, 12).millisecondsSinceEpoch;
      // 三桶并集、x 升序；缺失格不写键（渲染层断线处理）
      expect(r.rows, [
        {'x': t10, 's0': 10, 's1': 1},
        {'x': t11, 's1': 2},
        {'x': t12, 's0': 20},
      ]);
    });
  });

  group('buildHeatCells', () {
    test('hourly 桶 → (星期, 小时) 聚合，同格累加', () {
      final cells = buildHeatCells([
        bucket('2026-09-13 10:00:00', 3), // 2026-09-13 是周日
        bucket('2026-09-13 10:05:00', 4), // 同 (day=0, hour=10) 累加
        bucket('2026-09-14 09:00:00', 2), // 周一
      ]);
      expect(cells, contains((day: 0, hour: 10, value: 7)));
      expect(cells, contains((day: 1, hour: 9, value: 2)));
      expect(cells, hasLength(2));
    });
    test('daily 桶无小时信息不入格', () {
      expect(buildHeatCells([bucket('2026-09-13', 99)]), <HeatCell>[]);
    });
    test('脏时间串跳过不抛', () {
      expect(buildHeatCells([bucket('bad bucket', 1)]), <HeatCell>[]);
    });
  });

  group('buildDimensionDayCells', () {
    test('series 桶按本地日聚合，行按总量降序（首现序）', () {
      final cells = buildDimensionDayCells([
        series('glm', [bucket('2026-09-12', 1), bucket('2026-09-13 10:00:00', 2)]),
        series('claude', [
          bucket('2026-09-12', 5),
          bucket('2026-09-13 09:00:00', 5),
          bucket('2026-09-14', 1),
        ]),
      ], 2);
      // claude 总量 11 > glm 3 → claude 行在前；hourly 桶摊入当日
      expect(cells, [
        (name: 'claude', day: DateTime(2026, 9, 12).millisecondsSinceEpoch, value: 5),
        (name: 'claude', day: DateTime(2026, 9, 13).millisecondsSinceEpoch, value: 5),
        (name: 'claude', day: DateTime(2026, 9, 14).millisecondsSinceEpoch, value: 1),
        (name: 'glm', day: DateTime(2026, 9, 12).millisecondsSinceEpoch, value: 1),
        (name: 'glm', day: DateTime(2026, 9, 13).millisecondsSinceEpoch, value: 2),
      ]);
    });
    test('topN 截断丢弃尾部维度', () {
      final cells = buildDimensionDayCells([
        series('a', [bucket('2026-09-12', 10)]),
        series('b', [bucket('2026-09-12', 5)]),
      ], 1);
      expect(cells, [
        (name: 'a', day: DateTime(2026, 9, 12).millisecondsSinceEpoch, value: 10),
      ]);
    });
    test('series 空 → 空格子（DimensionHeatmap 诚实空态）', () {
      expect(buildDimensionDayCells([]), <DimensionDayCell>[]);
    });
  });

  group('buildQuotaGauges', () {
    QuotaSnapshot snap(int platformId, double estBalanceRemaining, int createdAt) =>
        QuotaSnapshot(
          platformId: platformId,
          estBalanceRemaining: estBalanceRemaining,
          createdAt: createdAt,
        );
    const min = 60000;

    test('按平台分组：current=最新快照，peak=窗口峰值，trend fraction=余额/峰值（at 为 Unix 秒）', () {
      // p1 乱序喂入（纯函数不依赖调用侧升序约定）
      final snaps = [
        snap(1, 4, 3 * min), // p1 最新 → current=4，peak=10
        snap(1, 10, 1 * min),
        snap(2, 5, 2 * min), // p2 单点 → current=peak=5
      ];
      final gauges = buildQuotaGauges(snaps);
      // QuotaGauge 无值相等语义（trend 是 List），逐字段断言同一组期望值
      expect(gauges, hasLength(2));
      expect(gauges[0].platformId, 2);
      expect(gauges[0].current, 5);
      expect(gauges[0].peak, 5);
      expect(gauges[0].trend, [(at: 120, fraction: 1.0)]);
      expect(gauges[1].platformId, 1);
      expect(gauges[1].current, 4);
      expect(gauges[1].peak, 10);
      expect(gauges[1].trend, [(at: 60, fraction: 1.0), (at: 180, fraction: 0.4)]);
      // 按当前余额降序：p2(5) 在 p1(4) 前
      expect(gauges[0].platformId, 2);
    });

    test('峰值 ≤ 0 的平台剔除（喂仪表盘也是空态，不如不出卡）；空输入 → 空数组', () {
      expect(buildQuotaGauges([snap(3, 0, min)]), <QuotaGauge>[]);
      expect(buildQuotaGauges([]), <QuotaGauge>[]);
    });
  });
}
