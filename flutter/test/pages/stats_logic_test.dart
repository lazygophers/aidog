// 使用统计页纯逻辑单测。四个二次聚合函数的断言在 test/stats/aggregation_test.dart
// （票 I05 已逐条翻自 src/pages/Stats.test.ts），这里只测本票新增的那部分：
// 时间窗 / 环比 / 粒度降级 / 排序 / 分页 / 查询体 / 空名归一化 / 筛选搜索。
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/stats/models.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

StatsBucket bucket(String tb, int requests) =>
    StatsBucket(timeBucket: tb, totalRequests: requests);

DimensionEntry dim(
  String name, {
  int req = 0,
  int success = 0,
  int input = 0,
  int output = 0,
  int cache = 0,
  double cacheRate = 0,
  double avgMs = 0,
  double cost = 0,
}) => DimensionEntry(
  name: name,
  totalRequests: req,
  successCount: success,
  inputTokens: input,
  outputTokens: output,
  cacheTokens: cache,
  cacheRate: cacheRate,
  avgDurationMs: avgMs,
  totalCost: cost,
);

void main() {
  group('getTimeRange', () {
    final now = DateTime(2026, 9, 13, 14, 30, 15, 250);

    test('today = 本地当日零点 → 现在', () {
      final r = getTimeRange(TimePreset.today, now);
      expect(r.start, DateTime(2026, 9, 13).millisecondsSinceEpoch);
      expect(r.end, now.millisecondsSinceEpoch);
    });

    test('7d / 30d 往前推整天但保留时分秒（对齐 JS 的 setDate）', () {
      expect(
        getTimeRange(TimePreset.sevenDays, now).start,
        DateTime(2026, 9, 6, 14, 30, 15, 250).millisecondsSinceEpoch,
      );
      expect(
        getTimeRange(TimePreset.thirtyDays, now).start,
        DateTime(2026, 8, 14, 14, 30, 15, 250).millisecondsSinceEpoch,
      );
    });

    test('跨月 / 跨年自动归一', () {
      final r = getTimeRange(TimePreset.thirtyDays, DateTime(2026, 1, 5, 9));
      expect(r.start, DateTime(2025, 12, 6, 9).millisecondsSinceEpoch);
    });
  });

  test('previousRange 整体往前平移一个窗口长度', () {
    final r = previousRange(1000, 4000);
    expect(r.start, -2000);
    expect(r.end, 1000);
  });

  group('delta', () {
    test('prev > 0 → 百分比增减', () {
      expect(delta(150, 100), closeTo(50, 1e-9));
      expect(delta(50, 100), closeTo(-50, 1e-9));
      expect(delta(100, 100), 0);
    });
    test('prev 为 0 或负 → null（无对比基准）', () {
      expect(delta(10, 0), isNull);
      expect(delta(10, -1), isNull);
    });
  });

  test('granularityForPreset：today→hourly，7d/30d→daily', () {
    expect(granularityForPreset(TimePreset.today), 'hourly');
    expect(granularityForPreset(TimePreset.sevenDays), 'daily');
    expect(granularityForPreset(TimePreset.thirtyDays), 'daily');
  });

  test('granularityKey / isFineGranularity', () {
    expect(granularityKey('minute'), 'stats.granMinute');
    expect(granularityKey('5min'), 'stats.gran5min');
    expect(granularityKey('hourly'), 'stats.granHourly');
    expect(granularityKey('daily'), 'stats.granDaily');
    expect(granularityKey('whatever'), 'stats.granDaily');
    expect(isFineGranularity('minute'), isTrue);
    expect(isFineGranularity('5min'), isTrue);
    expect(isFineGranularity('hourly'), isFalse);
  });

  group('baseQuery', () {
    Map<String, Object?> q({
      String group = '',
      String model = '',
      String platform = '',
      bool coding = false,
    }) => baseQuery(
      granularity: 'hourly',
      groupBy: 'platform',
      filterGroup: group,
      filterModel: model,
      filterPlatform: platform,
      filterCodingPlan: coding,
    );

    test('空筛选项不进 map（不传 ≠ 传空串）', () {
      expect(q().keys.toList()..sort(), ['granularity', 'group_by']);
    });

    test('「无分组」sentinel → filter_group 空串', () {
      expect(q(group: kNoGroupSentinel)['filter_group'], '');
    });

    test('普通值原样透传；"0" 平台是 truthy，要进 map', () {
      final m = q(group: 'g1', model: 'gpt-5', platform: '0', coding: true);
      expect(m['filter_group'], 'g1');
      expect(m['filter_model'], 'gpt-5');
      expect(m['filter_platform'], '0');
      expect(m['filter_coding_plan'], true);
    });

    test('coding=false 不进 map', () {
      expect(q(coding: false).containsKey('filter_coding_plan'), isFalse);
    });
  });

  group('shouldDowngradeToMinute', () {
    const h24 = 24 * 60 * 60 * 1000;
    final sparse = [bucket('2026-09-13 10:00:00', 1)];
    final dense = [
      for (var h = 0; h < 4; h++) bucket('2026-09-13 0$h:00:00', 1),
    ];

    test('hourly + ≤24h + 非空桶 < 4 → 降级', () {
      expect(
        shouldDowngradeToMinute(
          granularity: 'hourly',
          spanMs: h24,
          buckets: sparse,
        ),
        isTrue,
      );
    });
    test('非空桶 ≥ 4 → 不降级', () {
      expect(
        shouldDowngradeToMinute(
          granularity: 'hourly',
          spanMs: h24,
          buckets: dense,
        ),
        isFalse,
      );
    });
    test('长范围绝不降到 minute（防桶爆炸）', () {
      expect(
        shouldDowngradeToMinute(
          granularity: 'hourly',
          spanMs: h24 + 1,
          buckets: sparse,
        ),
        isFalse,
      );
    });
    test('用户选 daily 时不降级', () {
      expect(
        shouldDowngradeToMinute(
          granularity: 'daily',
          spanMs: h24,
          buckets: sparse,
        ),
        isFalse,
      );
    });
    test('零请求的桶不算非空', () {
      expect(
        shouldDowngradeToMinute(
          granularity: 'hourly',
          spanMs: h24,
          buckets: [for (var h = 0; h < 8; h++) bucket('2026-09-13 0$h:00:00', 0)],
        ),
        isTrue,
      );
    });
  });

  group('sortDimensions', () {
    final dims = [
      dim('b', req: 5, cost: 3),
      dim('a', req: 9, cost: 1),
      dim('c', req: 5, cost: 2),
    ];

    test('数值列降序 / 升序', () {
      expect(
        [for (final d in sortDimensions(dims, SortKey.totalRequests, SortDir.desc)) d.name],
        ['a', 'b', 'c'],
      );
      expect(
        [for (final d in sortDimensions(dims, SortKey.totalRequests, SortDir.asc)) d.name],
        ['b', 'c', 'a'],
      );
    });

    test('并列保持首现序（稳定排序）', () {
      // b 与 c 的 total_requests 都是 5，降序里 b 必须仍在 c 前
      final r = sortDimensions(dims, SortKey.totalRequests, SortDir.desc);
      expect(r[1].name, 'b');
      expect(r[2].name, 'c');
    });

    test('name 列按串比较', () {
      expect(
        [for (final d in sortDimensions(dims, SortKey.name, SortDir.asc)) d.name],
        ['a', 'b', 'c'],
      );
    });

    test('空列表不抛', () => expect(sortDimensions(const [], SortKey.name, SortDir.asc), isEmpty));
  });

  group('nextSort', () {
    test('同列点第二下反向', () {
      final r = nextSort(SortKey.totalCost, SortDir.desc, SortKey.totalCost);
      expect(r.key, SortKey.totalCost);
      expect(r.dir, SortDir.asc);
    });
    test('换到数值列 → 降序', () {
      final r = nextSort(SortKey.name, SortDir.asc, SortKey.totalCost);
      expect(r.key, SortKey.totalCost);
      expect(r.dir, SortDir.desc);
    });
    test('换到 name 列 → 升序', () {
      final r = nextSort(SortKey.totalCost, SortDir.desc, SortKey.name);
      expect(r.key, SortKey.name);
      expect(r.dir, SortDir.asc);
    });
  });

  group('paginate', () {
    final rows = [for (var i = 0; i < 120; i++) dim('d$i')];

    test('每页 50，页数向上取整', () {
      final p = paginate(rows, 0);
      expect(p.pageCount, 3);
      expect(p.safePage, 0);
      expect(p.rows.length, 50);
      expect(p.rows.first.name, 'd0');
    });

    test('末页只剩余数条', () {
      final p = paginate(rows, 2);
      expect(p.rows.length, 20);
      expect(p.rows.first.name, 'd100');
    });

    test('页码越界夹到最后一页（筛选变窄后旧 page 还留着）', () {
      final p = paginate(rows, 99);
      expect(p.safePage, 2);
      expect(p.rows.length, 20);
    });

    test('空列表：页数仍是 1、行为空、不抛', () {
      final p = paginate(const [], 5);
      expect(p.pageCount, 1);
      expect(p.safePage, 0);
      expect(p.rows, isEmpty);
    });
  });

  test('空维度名归一化成「未知平台」，非空名不动', () {
    final r = normalizeDimNames([dim(''), dim('glm', req: 3)], '未知平台');
    expect([for (final d in r) d.name], ['未知平台', 'glm']);
    expect(r[1].totalRequests, 3);
  });

  test('空 series 名同样归一化，桶原样带过去', () {
    final r = normalizeSeriesNames([
      StatsSeries(name: '', buckets: [bucket('2026-09-13 10:00:00', 4)]),
    ], '未知平台');
    expect(r.single.name, '未知平台');
    expect(r.single.buckets.single.totalRequests, 4);
  });

  testWidgets('weekdayShort：0 = 周日（对齐 JS getDay 的下标约定）', (tester) async {
    late List<String> got;
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) {
            got = [for (var d = 0; d < 7; d++) weekdayShort(context, d)];
            return const SizedBox.shrink();
          },
        ),
      ),
    );
    expect(got, ['S', 'M', 'T', 'W', 'T', 'F', 'S']);
  });

  group('filterOptions', () {
    final opts = [
      const FilterOption(value: '1', label: '智谱', searchTerms: ['zhipu', 'zp']),
      const FilterOption(value: '2', label: 'DeepSeek'),
      const FilterOption(value: '3', label: '月之暗面', searchTerms: ['moonshot']),
    ];

    test('空查询（含全空格）返回全部', () {
      expect(filterOptions('', opts).length, 3);
      expect(filterOptions('   ', opts).length, 3);
    });
    test('label 子串不分大小写', () {
      expect([for (final o in filterOptions('deep', opts)) o.value], ['2']);
      expect([for (final o in filterOptions('DEEP', opts)) o.value], ['2']);
    });
    test('label 直接中文命中', () {
      expect([for (final o in filterOptions('智谱', opts)) o.value], ['1']);
    });
    test('searchTerms 命中（registry 的拼音 / 首字母是字面数据）', () {
      expect([for (final o in filterOptions('zhipu', opts)) o.value], ['1']);
      expect([for (final o in filterOptions('zp', opts)) o.value], ['1']);
      expect([for (final o in filterOptions('moon', opts)) o.value], ['3']);
    });
    test('无匹配 → 空（调用方据此显示 stats.noMatch）', () {
      expect(filterOptions('nope', opts), isEmpty);
    });
  });
}
